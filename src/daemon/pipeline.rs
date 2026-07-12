use crate::core::config::Config;
use crate::core::types::{ChangePattern, Evidence};
use crate::intent::classifier::engine::{self, ClassificationResult};
use crate::intent::cluster::{edges, graph::RelationshipGraph, partition};
use crate::intent::message::generator;
use crate::intent::policy;
use crate::vcs::commit::apply_commit;
use crate::vcs::diff::{self, FileDiff};
use crate::vcs::repo::GitRepo;
use globset::{Glob, GlobSetBuilder};
use log::{debug, info, warn};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[allow(clippy::too_many_lines)]
pub fn run_pipeline(project_root: &Path, is_running: &Arc<AtomicBool>) {
    let config = match Config::load(project_root) {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to load config: {e}");
            return;
        }
    };

    let git_repo = match GitRepo::open(project_root) {
        Ok(r) => r,
        Err(e) => {
            warn!("Failed to open git repo: {e}");
            return;
        }
    };

    let all_diffs = match diff::get_working_copy_diff(&git_repo) {
        Ok(d) => d,
        Err(e) => {
            warn!("Failed to get diff: {e}");
            return;
        }
    };

    if all_diffs.is_empty() {
        debug!("No changes in working copy");
        return;
    }

    let diffs = apply_include_exclude(&all_diffs, &config);
    if diffs.is_empty() {
        debug!("No changes after include/exclude filtering");
        return;
    }

    info!("Processing {} files through pipeline", diffs.len());

    let mut graph = RelationshipGraph::new();
    let mut all_data: Vec<(PathBuf, Vec<Evidence>, ClassificationResult)> = Vec::new();

    for file_diff in &diffs {
        if !is_running.load(Ordering::Relaxed) {
            return;
        }

        let (path, changes, evidence) = match crate::cli::commands::analyze::analyze_file(file_diff)
        {
            Ok(data) => data,
            Err(e) => {
                warn!("Failed to analyze {}: {e}", file_diff_path(file_diff).display());
                continue;
            }
        };

        let classification = engine::classify(&path, &changes, &evidence);
        let pattern = match &classification {
            ClassificationResult::Classified { pattern, .. } => pattern.clone(),
            ClassificationResult::Ambiguous { .. } | ClassificationResult::Unclassified { .. } => {
                ChangePattern::Unknown
            }
        };
        graph.add_file(path.clone(), pattern);
        all_data.push((path, evidence, classification));
    }

    if !is_running.load(Ordering::Relaxed) {
        return;
    }

    edges::detect_all_edges(&mut graph);
    let groups = partition::partition(&graph);

    for group in &groups {
        if !is_running.load(Ordering::Relaxed) {
            return;
        }

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
            crate::core::types::PolicyDecision::AutoCommittable => {
                let file_refs: Vec<&Path> = group.files.iter().map(PathBuf::as_path).collect();
                match apply_commit(&git_repo, &message, &file_refs) {
                    Ok(sha) => {
                        let first_line = message.lines().next().unwrap_or(&message);
                        info!(
                            "Auto-committed {:?} ({} files): {} [{}]",
                            group.pattern,
                            group.files.len(),
                            first_line,
                            &sha[..8]
                        );
                    }
                    Err(e) => {
                        warn!("Auto-commit failed for {:?}: {e}", group.pattern);
                    }
                }
            }
            crate::core::types::PolicyDecision::RequiresReview => {
                debug!("Requires review: {:?}", group.pattern);
            }
            crate::core::types::PolicyDecision::Blocked { reason } => {
                debug!("Blocked {:?}: {reason}", group.pattern);
            }
        }
    }
}

const fn file_diff_path(diff: &FileDiff) -> &PathBuf {
    match diff {
        FileDiff::Added { path, .. }
        | FileDiff::Removed { path, .. }
        | FileDiff::Modified { path, .. } => path,
    }
}

fn apply_include_exclude(diffs: &[FileDiff], config: &Config) -> Vec<FileDiff> {
    let include_matcher = build_glob_matcher(&config.analysis.include_patterns);
    let exclude_matcher = build_glob_matcher(&config.analysis.exclude_patterns);

    diffs
        .iter()
        .filter(|diff| {
            let path = file_diff_path(diff);
            let path_str = path.to_string_lossy();

            let included = if include_matcher.is_empty() {
                true
            } else {
                include_matcher.is_match(&*path_str)
            };
            if !included {
                return false;
            }

            let excluded = if exclude_matcher.is_empty() {
                false
            } else {
                exclude_matcher.is_match(&*path_str)
            };
            !excluded
        })
        .cloned()
        .collect()
}

fn build_glob_matcher(patterns: &[String]) -> globset::GlobSet {
    let mut builder = GlobSetBuilder::new();
    for pat in patterns {
        match Glob::new(pat) {
            Ok(glob) => {
                builder.add(glob);
            }
            Err(e) => {
                warn!("Invalid glob pattern '{pat}': {e}");
            }
        }
    }
    match builder.build() {
        Ok(set) => set,
        Err(e) => {
            warn!("Failed to build glob set: {e}");
            GlobSetBuilder::new().build().unwrap_or_default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_run_pipeline_empty_diff() {
        let dir = tempdir().unwrap();
        let is_running = Arc::new(AtomicBool::new(true));
        // Should not panic with empty project
        run_pipeline(dir.path(), &is_running);
    }
}
