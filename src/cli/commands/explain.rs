use anyhow::Context;
use owo_colors::OwoColorize;

use crate::intent::message::parser;
use crate::vcs::repo::GitRepo;

pub fn run(repo: &GitRepo, commit_sha: &str) -> anyhow::Result<()> {
    let message = repo
        .find_commit_message(commit_sha)
        .with_context(|| format!("could not find commit {commit_sha}"))?;

    let Some(block) = parser::extract_evidence_block(&message) else {
        println!("No AVCS evidence found in commit {commit_sha}.");
        return Ok(());
    };

    println!("\n{} | commit {commit_sha}\n", "AVCS Explain".bold());

    if let Some(pattern) = parser::extract_pattern(block) {
        println!("  Pattern: {pattern}");
    }
    if let Some(rule) = parser::extract_rule(block) {
        println!("  Rule:    {rule}");
    }

    println!();
    Ok(())
}
