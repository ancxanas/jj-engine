use owo_colors::OwoColorize;
use std::path::{Path, PathBuf};

use crate::core::config::Config;
use crate::core::types::{ChangePattern, Evidence, PolicyDecision};
use crate::intent::classifier::engine::{self, ClassificationResult};
use crate::intent::cluster::{edges, graph::RelationshipGraph, partition};
use crate::intent::message::generator;
use crate::intent::policy;
use crate::vcs::diff::{self, FileDiff};

pub fn run(auto: bool, _json: bool) -> anyhow::Result<()> {
    let project_root = std::env::current_dir()?;
    let config = Config::load(&project_root)?;
    let git_repo = crate::vcs::repo::GitRepo::open(&project_root)?;
    let all_diffs = diff::get_working_copy_diff(&git_repo)?;

    if all_diffs.is_empty() {
        println!("No changes in working copy.");
        return Ok(());
    }

    // Apply include/exclude filters
    let diffs: Vec<_> = all_diffs
        .into_iter()
        .filter(|diff| {
            let path = match diff {
                FileDiff::Added { path, .. }
                | FileDiff::Removed { path, .. }
                | FileDiff::Modified { path, .. } => path,
            };
            let path_str = path.to_string_lossy().to_ascii_lowercase();
            let included = config.analysis.include_patterns.iter().any(|pat| {
                let pat = pat.to_ascii_lowercase();
                if pat.contains('*') || pat.contains("**") {
                    let clean = pat.replace("**/", "").replace("/*", "");
                    path_str.contains(&clean)
                } else if let Some(stripped) = pat.strip_prefix('.') {
                    path_str.ends_with(stripped)
                } else {
                    path_str.contains(&pat)
                }
            });
            if !included {
                return false;
            }
            let excluded = config.analysis.exclude_patterns.iter().any(|pat| {
                let pat = pat.to_ascii_lowercase();
                if pat.contains('*') || pat.contains("**") {
                    let clean = pat.replace("**/", "").replace("/*", "");
                    path_str.contains(&clean)
                } else if let Some(stripped) = pat.strip_prefix('.') {
                    path_str.ends_with(stripped)
                } else {
                    path_str.contains(&pat)
                }
            });
            !excluded
        })
        .collect();

    let mut graph = RelationshipGraph::new();
    let mut all_data: Vec<(PathBuf, Vec<Evidence>, ClassificationResult)> = Vec::new();

    for diff in &diffs {
        let (path, changes, evidence) = crate::cli::commands::analyze::analyze_file(diff)?;
        let classification = engine::classify(&path, &changes, &evidence);
        let pattern = match &classification {
            ClassificationResult::Classified { pattern, .. } => pattern.clone(),
            ClassificationResult::Ambiguous { .. }
            | ClassificationResult::Unclassified { .. } => ChangePattern::Unknown,
        };
        graph.add_file(path.clone(), pattern);
        all_data.push((path, evidence, classification));
    }

    edges::detect_all_edges(&mut graph);
    let groups = partition::partition(&graph);

    for group in &groups {
        let (evidence, rule) =
            crate::cli::commands::analyze::merge_evidence(&group.files, &all_data);
        let message = generator::generate(group, &evidence, rule);
        let auto_patterns: Vec<ChangePattern> = config
            .autonomy
            .auto_commit_patterns
            .iter()
            .filter_map(|s| ChangePattern::parse(s))
            .collect();
        let decision = policy::engine::evaluate_with_config(&group.pattern, &auto_patterns);

        match decision {
            PolicyDecision::AutoCommittable if auto => {
                let file_refs: Vec<&Path> = group.files.iter().map(PathBuf::as_path).collect();
                let sha = crate::vcs::commit::apply_commit(&git_repo, &message, &file_refs)?;
                println!(
                    "  {} Auto-committed {} ({sha})",
                    "[OK]".green(),
                    first_line(&message),
                );
            }
            PolicyDecision::AutoCommittable => {
                println!(
                    "  {} Ready to auto-commit: {}",
                    "[AUTO]".blue(),
                    first_line(&message),
                );
            }
            PolicyDecision::RequiresReview => {
                println!(
                    "  {} Requires review: {}",
                    "[REVIEW]".yellow(),
                    first_line(&message),
                );
            }
            PolicyDecision::Blocked { reason } => {
                println!(
                    "  {} Blocked: {} ({reason})",
                    "[BLOCKED]".red(),
                    first_line(&message),
                );
            }
        }
    }

    if !auto {
        println!("\n  Run 'avcs commit --auto' to apply auto-committable intents.\n");
    }

    Ok(())
}

#[must_use]
fn first_line(message: &str) -> &str {
    message.lines().next().unwrap_or(message)
}
