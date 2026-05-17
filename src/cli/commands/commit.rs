use owo_colors::OwoColorize;
use std::path::{Path, PathBuf};

use crate::core::types::{ChangePattern, Evidence, PolicyDecision};
use crate::intent::classifier::engine::{self, ClassificationResult};
use crate::intent::cluster::{edges, graph::RelationshipGraph, partition};
use crate::intent::message::generator;
use crate::intent::policy;
use crate::jj::diff;

pub fn run(auto: bool, _json: bool) -> anyhow::Result<()> {
    let project_root = std::env::current_dir()?;
    let handle = crate::jj::repo::open(&project_root)?;
    let diffs = diff::get_working_copy_diff(&handle)?;

    if diffs.is_empty() {
        println!("No changes in working copy.");
        return Ok(());
    }

    let mut graph = RelationshipGraph::new();
    let mut all_data: Vec<(PathBuf, Vec<Evidence>, ClassificationResult)> = Vec::new();

    for diff in &diffs {
        let (path, changes, evidence) = crate::cli::commands::analyze::analyze_file(diff)?;
        let classification = engine::classify(&path, &changes, &evidence);
        let pattern = match &classification {
            ClassificationResult::Classified { pattern, .. } => pattern.clone(),
            ClassificationResult::Unclassified { .. } => ChangePattern::Unknown,
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
        let decision = policy::engine::evaluate(&group.pattern);

        match decision {
            PolicyDecision::AutoCommittable if auto => {
                let file_refs: Vec<&Path> = group.files.iter().map(PathBuf::as_path).collect();
                let change_id = crate::jj::executor::apply_commit(&handle, &message, &file_refs)?;
                println!(
                    "  {} Auto-committed {} ({change_id})",
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
