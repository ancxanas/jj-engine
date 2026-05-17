use owo_colors::OwoColorize;

use crate::intent::message::parser;

pub fn run(change_id: &str) -> anyhow::Result<()> {
    let project_root = std::env::current_dir()?;

    let output = std::process::Command::new("jj")
        .current_dir(&project_root)
        .arg("log")
        .arg("-r")
        .arg(change_id)
        .arg("--no-graph")
        .arg("-T")
        .arg("description")
        .output()?;

    if !output.status.success() {
        anyhow::bail!("could not find commit {change_id}");
    }

    let message = String::from_utf8_lossy(&output.stdout);
    let Some(block) = parser::extract_evidence_block(&message) else {
        println!("No AVCS evidence found in commit {change_id}.");
        return Ok(());
    };

    println!("\n{} | commit {change_id}\n", "AVCS Explain".bold());

    if let Some(pattern) = parser::extract_pattern(block) {
        println!("  Pattern: {pattern}");
    }
    if let Some(rule) = parser::extract_rule(block) {
        println!("  Rule:    {rule}");
    }

    println!();
    Ok(())
}
