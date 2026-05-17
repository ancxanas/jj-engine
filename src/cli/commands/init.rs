use anyhow::Context;
use std::fs;
use std::path::Path;

const DEFAULT_CONFIG: &str = r#"[project]
name = ""
language = "typescript"

[analysis]
include_patterns = ["src/**/*", "*.md"]
exclude_patterns = ["node_modules/**", "dist/**", ".next/**", "coverage/**"]

[autonomy]
auto_commit_patterns = ["Documentation"]

[commit]
message_style = "conventional"
include_evidence_footer = true
sign_commits = false
max_subject_length = 72

[output]
default_format = "human"
color = true
"#;

pub fn run() -> anyhow::Result<()> {
    let avcs_dir = Path::new(".autonomous-vcs");

    if avcs_dir.exists() {
        println!("AVCS already initialized in this project.");
        return Ok(());
    }

    fs::create_dir_all(avcs_dir).context("failed to create .autonomous-vcs directory")?;
    fs::write(avcs_dir.join("config.toml"), DEFAULT_CONFIG)
        .context("failed to write config.toml")?;
    fs::write(avcs_dir.join(".gitignore"), "ast-cache/\n").context("failed to write .gitignore")?;

    println!("AVCS initialized. Config at .autonomous-vcs/config.toml");
    Ok(())
}
