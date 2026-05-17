use jj_lib::object_id::ObjectId;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::core::output::renderer::Renderer;
use crate::core::types::{
    AnalysisMeta, AnalysisResult, AnalysisStats, ChangePattern, Evidence, FileChange,
    FileChangeType, ImpactAssessment, ImpactLevel, Intent, IntentStatus, PolicyDecision,
    StructuralChange, UnclassifiedChange,
};
use crate::intent::classifier::engine::{self, ClassificationResult};
use crate::intent::cluster::{edges, graph::RelationshipGraph, partition};
use crate::intent::message::generator;
use crate::intent::policy;
use crate::jj::diff::{self, FileDiff};
use crate::semantic::languages;

struct AnalyzedFile {
    path: PathBuf,
    change_type: FileChangeType,
    structural_changes: Vec<StructuralChange>,
    evidence: Vec<Evidence>,
    classification: ClassificationResult,
}

pub fn run(json: bool) -> anyhow::Result<()> {
    let start = Instant::now();
    let project_root = std::env::current_dir()?;
    let handle = crate::jj::repo::open(&project_root)?;
    let diffs = diff::get_working_copy_diff(&handle)?;
    let (jj_change_id, jj_commit_id) = current_ids(&handle)?;

    let mut graph = RelationshipGraph::new();
    let mut analyzed_files = Vec::new();

    for diff in &diffs {
        let (path, changes, evidence) = analyze_file(diff)?;
        let classification = engine::classify(&path, &changes, &evidence);
        let pattern = match &classification {
            ClassificationResult::Classified { pattern, .. } => pattern.clone(),
            ClassificationResult::Unclassified { .. } => ChangePattern::Unknown,
        };

        graph.add_file(path.clone(), pattern);
        analyzed_files.push(AnalyzedFile {
            path,
            change_type: file_change_type(diff),
            structural_changes: changes,
            evidence,
            classification,
        });
    }

    edges::detect_all_edges(&mut graph);
    let groups = partition::partition(&graph);
    let evidence_data = build_evidence_index(&analyzed_files);

    let mut intents = Vec::new();
    for (index, group) in groups.iter().enumerate() {
        let (evidence, rule) = merge_evidence(&group.files, &evidence_data);
        let message = generator::generate(group, &evidence, rule);
        let policy_decision = policy::engine::evaluate(&group.pattern);

        intents.push(Intent {
            id: format!("int-{:03}", index + 1),
            pattern: group.pattern.clone(),
            suggested_message: message,
            files: build_intent_files(group, &analyzed_files),
            evidence,
            clustering_reason: group.reason.clone(),
            relationships: group.relationships.clone(),
            impact: ImpactAssessment {
                level: ImpactLevel::Low,
                affected_files: group.files.len(),
                is_public_api_change: false,
                is_breaking: false,
                notes: vec![],
            },
            order: index + 1,
            status: IntentStatus::Pending,
            policy: policy_decision,
        });
    }

    let unclassified = build_unclassified(&analyzed_files);
    let stats = build_stats(&intents, &unclassified, start.elapsed());
    let result = AnalysisResult {
        meta: AnalysisMeta {
            timestamp: chrono::Utc::now(),
            project_root,
            jj_change_id,
            jj_commit_id,
            total_files_changed: diffs.len(),
            analysis_duration_ms: duration_ms(start.elapsed()),
            analyzer_version: env!("CARGO_PKG_VERSION").into(),
        },
        intents,
        ambiguous: vec![],
        unclassified,
        stats,
    };

    let output = if json {
        crate::cli::output::json::JsonRenderer.render(&result)
    } else {
        crate::cli::output::human::HumanRenderer.render(&result)
    };
    println!("{output}");
    Ok(())
}

pub fn analyze_file(
    diff: &FileDiff,
) -> anyhow::Result<(PathBuf, Vec<StructuralChange>, Vec<Evidence>)> {
    match diff {
        FileDiff::Added { path, content, .. } => {
            let Some(analyzer) = languages::analyzer_for(path) else {
                return Ok((path.clone(), vec![], vec![]));
            };
            if analyzer.language_id() == "docs" {
                return Ok((path.clone(), vec![], analyzer.detect_evidence(&[])));
            }
            let before = b"".as_slice();
            let before_tree = analyzer.parse(before)?;
            let after_tree = analyzer.parse(content)?;
            let changes = analyzer.extract_changes(&before_tree, &after_tree, before, content)?;
            let evidence = analyzer.detect_evidence(&changes);
            Ok((path.clone(), changes, evidence))
        }
        FileDiff::Removed { path, content, .. } => {
            let Some(analyzer) = languages::analyzer_for(path) else {
                return Ok((path.clone(), vec![], vec![]));
            };
            if analyzer.language_id() == "docs" {
                return Ok((path.clone(), vec![], analyzer.detect_evidence(&[])));
            }
            let after = b"".as_slice();
            let before_tree = analyzer.parse(content)?;
            let after_tree = analyzer.parse(after)?;
            let changes = analyzer.extract_changes(&before_tree, &after_tree, content, after)?;
            let evidence = analyzer.detect_evidence(&changes);
            Ok((path.clone(), changes, evidence))
        }
        FileDiff::Modified {
            path,
            before,
            after,
            ..
        } => {
            let Some(analyzer) = languages::analyzer_for(path) else {
                return Ok((path.clone(), vec![], vec![]));
            };
            if analyzer.language_id() == "docs" {
                return Ok((path.clone(), vec![], analyzer.detect_evidence(&[])));
            }
            let before_tree = analyzer.parse(before)?;
            let after_tree = analyzer.parse(after)?;
            let changes = analyzer.extract_changes(&before_tree, &after_tree, before, after)?;
            let evidence = analyzer.detect_evidence(&changes);
            Ok((path.clone(), changes, evidence))
        }
    }
}

#[must_use]
pub fn merge_evidence(
    files: &[PathBuf],
    all: &[(PathBuf, Vec<Evidence>, ClassificationResult)],
) -> (Vec<Evidence>, &'static str) {
    let mut evidence = Vec::new();
    let mut rule = "unknown";
    for (path, items, classification) in all {
        if files.contains(path) {
            evidence.extend(items.clone());
            if let ClassificationResult::Classified {
                rule: matched_rule, ..
            } = classification
            {
                rule = matched_rule;
            }
        }
    }
    (evidence, rule)
}

fn current_ids(repo_handle: &crate::jj::repo::RepoHandle) -> anyhow::Result<(String, String)> {
    use jj_lib::repo::Repo;

    let repo = &repo_handle.repo;
    let wc_commit_id = repo
        .view()
        .get_wc_commit_id(&repo_handle.workspace_name)
        .ok_or_else(|| anyhow::anyhow!("no working copy commit found"))?
        .clone();
    let wc_commit = repo.store().get_commit(&wc_commit_id)?;
    Ok((wc_commit.change_id().hex(), wc_commit.id().hex()))
}

#[must_use]
const fn file_change_type(diff: &FileDiff) -> FileChangeType {
    match diff {
        FileDiff::Added { .. } => FileChangeType::Added,
        FileDiff::Removed { .. } => FileChangeType::Removed,
        FileDiff::Modified { .. } => FileChangeType::Modified,
    }
}

#[must_use]
fn build_evidence_index(
    analyzed_files: &[AnalyzedFile],
) -> Vec<(PathBuf, Vec<Evidence>, ClassificationResult)> {
    analyzed_files
        .iter()
        .map(|file| {
            (
                file.path.clone(),
                file.evidence.clone(),
                file.classification.clone(),
            )
        })
        .collect()
}

#[must_use]
fn build_intent_files(
    group: &partition::IntentGroup,
    analyzed_files: &[AnalyzedFile],
) -> Vec<FileChange> {
    let mut files: Vec<_> = analyzed_files
        .iter()
        .filter(|file| group.files.contains(&file.path))
        .map(|file| FileChange {
            path: file.path.clone(),
            change_type: file.change_type.clone(),
            structural_changes: file.structural_changes.clone(),
        })
        .collect();
    files.sort_by(|left, right| left.path.cmp(&right.path));
    files
}

#[must_use]
fn build_unclassified(analyzed_files: &[AnalyzedFile]) -> Vec<UnclassifiedChange> {
    analyzed_files
        .iter()
        .filter_map(|file| {
            if let ClassificationResult::Unclassified {
                raw_changes,
                reason,
            } = &file.classification
            {
                Some(UnclassifiedChange {
                    files: vec![file.path.clone()],
                    raw_changes: raw_changes.clone(),
                    reason: reason.clone(),
                })
            } else {
                None
            }
        })
        .collect()
}

#[must_use]
fn build_stats(
    intents: &[Intent],
    unclassified: &[UnclassifiedChange],
    elapsed: Duration,
) -> AnalysisStats {
    AnalysisStats {
        total_intents: intents.len(),
        auto_committable: intents
            .iter()
            .filter(|intent| intent.policy == PolicyDecision::AutoCommittable)
            .count(),
        requires_review: intents
            .iter()
            .filter(|intent| intent.policy == PolicyDecision::RequiresReview)
            .count(),
        blocked: intents
            .iter()
            .filter(|intent| matches!(intent.policy, PolicyDecision::Blocked { .. }))
            .count(),
        ambiguous: 0,
        unclassified: unclassified.len(),
        parse_duration_ms: 0,
        diff_duration_ms: 0,
        cluster_duration_ms: 0,
        classify_duration_ms: duration_ms(elapsed),
    }
}

#[allow(clippy::cast_possible_truncation)]
#[must_use]
const fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis() as u64
}
