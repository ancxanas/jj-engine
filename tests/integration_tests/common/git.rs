//! Git repository helpers for integration tests

use std::path::{Path, PathBuf};
use std::process::Command;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub email: String,
    pub date: String,
    pub message: String,
    pub files: Vec<String>,
}

pub struct GitRepo {
    path: PathBuf,
}

impl GitRepo {
    pub fn init<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let path = path.as_ref().to_path_buf();
        
        // Initialize git repo
        let output = Command::new("git")
            .current_dir(&path)
            .args(["init"])
            .output()?;
        
        if !output.status.success() {
            anyhow::bail!("git init failed: {}", String::from_utf8_lossy(&output.stderr));
        }

        // Configure user
        Command::new("git")
            .current_dir(&path)
            .args(["config", "user.name", "Test User"])
            .output()?;
        
        Command::new("git")
            .current_dir(&path)
            .args(["config", "user.email", "test@example.com"])
            .output()?;

        Ok(Self { path })
    }

    pub fn add_all(&self) -> anyhow::Result<()> {
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(["add", "-A"])
            .output()?;
        
        if !output.status.success() {
            anyhow::bail!("git add failed: {}", String::from_utf8_lossy(&output.stderr));
        }
        Ok(())
    }

    pub fn commit(&self, message: &str) -> anyhow::Result<()> {
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(["commit", "-m", message])
            .output()?;
        
        if !output.status.success() {
            anyhow::bail!("git commit failed: {}", String::from_utf8_lossy(&output.stderr));
        }
        Ok(())
    }

    pub fn log(&self, limit: usize) -> anyhow::Result<Vec<Commit>> {
        let output = Command::new("git")
            .current_dir(&self.path)
            .args([
                "log",
                &format!("-{}", limit),
                "--pretty=format:%H|%h|%an|%ae|%ai|%s",
                "--name-only",
            ])
            .output()?;

        if !output.status.success() {
            anyhow::bail!("git log failed: {}", String::from_utf8_lossy(&output.stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        self.parse_log(&stdout)
    }

    fn parse_log(&self, log: &str) -> anyhow::Result<Vec<Commit>> {
        let mut commits = Vec::new();
        let mut current_commit: Option<Commit> = None;
        let mut in_files = false;

        for line in log.lines() {
            if line.is_empty() {
                if in_files {
                    in_files = false;
                    if let Some(commit) = current_commit.take() {
                        commits.push(commit);
                    }
                }
                continue;
            }

            if !in_files && line.contains('|') {
                // Commit header line
                if let Some(commit) = current_commit.take() {
                    commits.push(commit);
                }
                
                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() >= 6 {
                    current_commit = Some(Commit {
                        hash: parts[0].to_string(),
                        short_hash: parts[1].to_string(),
                        author: parts[2].to_string(),
                        email: parts[3].to_string(),
                        date: parts[4].to_string(),
                        message: parts[5].to_string(),
                        files: Vec::new(),
                    });
                    in_files = true;
                }
            } else if in_files && current_commit.is_some() {
                // File name
                if let Some(ref mut commit) = current_commit {
                    commit.files.push(line.trim().to_string());
                }
            }
        }

        // Don't forget the last commit
        if let Some(commit) = current_commit {
            commits.push(commit);
        }

        Ok(commits)
    }

    pub fn head_hash(&self) -> anyhow::Result<String> {
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(["rev-parse", "HEAD"])
            .output()?;

        if !output.status.success() {
            anyhow::bail!("git rev-parse HEAD failed: {}", String::from_utf8_lossy(&output.stderr));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub fn status(&self) -> anyhow::Result<String> {
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(["status", "--porcelain"])
            .output()?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    pub fn diff(&self, cached: bool) -> anyhow::Result<String> {
        let mut args = vec!["diff"];
        if cached {
            args.push("--cached");
        }
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(&args)
            .output()?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}