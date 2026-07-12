//! Real-world repository sources for integration testing

use std::path::Path;
use std::process::Command;
use anyhow::Result;

/// A real-world repository to test against
pub struct RealWorldRepo {
    pub name: &'static str,
    pub url: &'static str,
    pub clone_dir: &'static str,
}

impl RealWorldRepo {
    /// axios - popular HTTP client
    pub fn axios() -> Self {
        Self {
            name: "axios",
            url: "https://github.com/axios/axios.git",
            clone_dir: "axios",
        }
    }
    
    /// fastify - fast web framework
    pub fn fastify() -> Self {
        Self {
            name: "fastify",
            url: "https://github.com/fastify/fastify.git",
            clone_dir: "fastify",
        }
    }
    
    /// trpc - end-to-end typesafe APIs
    pub fn trpc() -> Self {
        Self {
            name: "trpc",
            url: "https://github.com/trpc/trpc.git",
            clone_dir: "trpc",
        }
    }
    
    /// All available repos
    pub fn all() -> Vec<Self> {
        vec![
            Self::axios(),
            Self::fastify(),
            Self::trpc(),
        ]
    }
    
    /// Clones the repository into the given base directory (depth=1)
    pub async fn clone_into(&self, base_dir: &Path) -> Result<()> {
        let dest = base_dir.join(self.clone_dir);
        
        // Remove if exists
        if dest.exists() {
            std::fs::remove_dir_all(&dest)?;
        }
        
        // Clone with depth=1
        let output = Command::new("git")
            .args(["clone", "--depth", "1", self.url, &dest.to_string_lossy()])
            .output()?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to clone {}: {}", self.url, stderr);
        }
        
        // Strip .git to make it a fresh repo
        let git_dir = dest.join(".git");
        if git_dir.exists() {
            std::fs::remove_dir_all(&git_dir)?;
        }
        
        Ok(())
    }
    
    /// Initializes a fresh git repo in the cloned directory
    pub fn init_fresh_git(&self, base_dir: &Path) -> Result<()> {
        let dest = base_dir.join(self.clone_dir);
        
        let output = Command::new("git")
            .current_dir(&dest)
            .args(["init"])
            .output()?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to init git: {}", stderr);
        }
        
        // Configure user
        Command::new("git")
            .current_dir(&dest)
            .args(["config", "user.name", "Test User"])
            .output()?;
        
        Command::new("git")
            .current_dir(&dest)
            .args(["config", "user.email", "test@example.com"])
            .output()?;
        
        // Initial commit
        Command::new("git")
            .current_dir(&dest)
            .args(["add", "-A"])
            .output()?;
        
        Command::new("git")
            .current_dir(&dest)
            .args(["commit", "-m", "Initial commit after clone"])
            .output()?;
        
        Ok(())
    }
}