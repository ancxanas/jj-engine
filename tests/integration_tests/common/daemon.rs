//! Daemon test harness for AVCS integration tests

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::time::sleep;

use crate::common::git::{GitRepo, Commit};
use avcs_lib::daemon::lifecycle;

/// Harness for managing an AVCS daemon process during tests
pub struct DaemonHarness {
    pub temp_dir: TempDir,
    daemon_child: Option<Child>,
    pub git: GitRepo,
    pub avcs_binary: PathBuf,
}

impl DaemonHarness {
    /// Creates a new daemon harness with a temporary directory and initialized git repo
    pub async fn new() -> anyhow::Result<Self> {
        let temp_dir = tempfile::tempdir()?;
        let git = GitRepo::init(temp_dir.path())?;
        let avcs_binary = find_avcs_binary()?;

        Ok(Self {
            temp_dir,
            daemon_child: None,
            git,
            avcs_binary,
        })
    }
    
    /// Creates a daemon harness from an existing directory path
    pub async fn new_from_path(path: PathBuf) -> anyhow::Result<Self> {
        let git = GitRepo::init(&path)?;
        let avcs_binary = find_avcs_binary()?;
        
        // Create a tempdir wrapper for the existing path
        let temp_dir = tempfile::tempdir()?;
        std::fs::remove_dir(temp_dir.path())?;
        std::fs::rename(&path, temp_dir.path())?;
        
        Ok(Self {
            temp_dir,
            daemon_child: None,
            git,
            avcs_binary,
        })
    }

    /// Initializes AVCS in the test directory with optional custom config
    pub async fn init_avcs(&mut self, config: Option<&str>) -> anyhow::Result<()> {
        let config_content: String = match config {
            Some(c) => c.to_string(),
            None => crate::common::fixture::avcs_config_toml(true),
        };
        eprintln!("DEBUG: Writing config:\n{}", config_content);
        
        // Write config file
        let config_path = self.temp_dir.path().join(".autonomous-vcs").join("config.toml");
        std::fs::create_dir_all(config_path.parent().unwrap())?;
        std::fs::write(&config_path, config_content)?;

        // Run avcs init
        let output = Command::new(&self.avcs_binary)
            .current_dir(self.temp_dir.path())
            .arg("init")
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("avcs init failed: {}", stderr);
        }

        // Commit the initial config
        self.git.add_all()?;
        self.git.commit("Initial commit with AVCS config")?;

        Ok(())
    }

    /// Starts the AVCS daemon in the background
    pub async fn start_daemon(&mut self) -> anyhow::Result<()> {
        let binary = find_avcs_binary()?;
        eprintln!("DEBUG: Using avcs binary: {:?}", binary);
        
        let mut cmd = Command::new(&binary);
        cmd.current_dir(self.temp_dir.path())
            .arg("daemon")
            .arg("start");
        // Don't pipe stdout/stderr - let them go to terminal for debugging

        let child = cmd.spawn()?;
        
        // Wait for the parent process to exit (it prints the daemon PID and exits)
        let output = child.wait_with_output()?;
        
        if !output.status.success() {
            anyhow::bail!("avcs daemon start failed with status {:?}", output.status);
        }
        
        // The parent process exits successfully, but the daemon child continues running
        // Wait a bit for the daemon to fully start and write its PID file
        sleep(Duration::from_millis(5000)).await;
        
        // Verify the daemon is running by checking the PID file
        let pid_file = self.temp_dir.path().join(".autonomous-vcs/daemon.pid");
        eprintln!("DEBUG: Looking for PID file at: {:?}", pid_file);
        eprintln!("DEBUG: PID file exists: {}", pid_file.exists());
        
        if !pid_file.exists() {
            // List files in the directory for debugging
            if let Ok(entries) = std::fs::read_dir(self.temp_dir.path()) {
                for entry in entries.flatten() {
                    eprintln!("DEBUG: File: {:?}", entry.path());
                }
            }
            if let Ok(entries) = std::fs::read_dir(self.temp_dir.path().join(".autonomous-vcs")) {
                for entry in entries.flatten() {
                    eprintln!("DEBUG: .autonomous-vcs file: {:?}", entry.path());
                }
            }
            anyhow::bail!("Daemon PID file not found after start");
        }
        
        let pid_str = std::fs::read_to_string(&pid_file)?;
        let pid: u32 = pid_str.trim().parse()?;
        
        // Verify the process is actually running
        if !lifecycle::is_running(pid) {
            // Try to read the daemon log file to see what happened
            let log_file = self.temp_dir.path().join(".autonomous-vcs/daemon_rCURRENT.log");
            eprintln!("DEBUG: Checking for log file at: {:?}, exists: {}", log_file, log_file.exists());
            if log_file.exists() {
                if let Ok(log_content) = std::fs::read_to_string(&log_file) {
                    eprintln!("DEBUG: Daemon log content:");
                    for line in log_content.lines() {
                        eprintln!("LOG: {}", line);
                    }
                }
            } else {
                eprintln!("DEBUG: Log file does not exist, checking directory:");
                if let Ok(entries) = std::fs::read_dir(self.temp_dir.path().join(".autonomous-vcs")) {
                    for entry in entries.flatten() {
                        eprintln!("DEBUG: File in .autonomous-vcs: {:?}", entry.path());
                    }
                }
            }
            anyhow::bail!("Daemon process (PID {}) is not running", pid);
        }
        
        println!("Daemon started successfully (PID {})", pid);
        
        Ok(())
    }

    /// Stops the AVCS daemon gracefully
    pub async fn stop_daemon(&mut self) -> anyhow::Result<()> {
        // Use lifecycle stop_daemon which handles PID file and signal properly
        lifecycle::stop_daemon(self.temp_dir.path())?;

        Ok(())
    }

    /// Returns the current number of commits in the git log
    pub fn commit_count(&self) -> anyhow::Result<usize> {
        Ok(self.git.log(100)?.len())
    }

    /// Waits for a new commit to appear in the git history
    pub async fn wait_for_commit(&self, timeout_duration: Duration) -> anyhow::Result<Commit> {
        let start_count = self.commit_count()?;
        self.wait_for_commit_from(start_count, timeout_duration).await
    }

    /// Waits for a commit to appear after the given baseline count.
    /// Use this to avoid race conditions where the daemon commits during startup.
    pub async fn wait_for_commit_from(
        &self,
        baseline_count: usize,
        timeout_duration: Duration,
    ) -> anyhow::Result<Commit> {
        let deadline = Instant::now() + timeout_duration;

        while Instant::now() < deadline {
            let commits = self.git.log(100)?;
            if commits.len() > baseline_count {
                // Return the newest commit (first in log)
                return Ok(commits[0].clone());
            }
            sleep(Duration::from_millis(500)).await;
        }

        anyhow::bail!("Timeout waiting for new commit after {:?}", timeout_duration)
    }

    /// Gets all commits in the repo
    pub fn get_commits(&self) -> anyhow::Result<Vec<Commit>> {
        self.git.log(50)
    }

    /// Writes a file to the test directory
    pub fn write_file(&self, path: &str, content: &str) -> anyhow::Result<()> {
        let full_path = self.temp_dir.path().join(path);
        crate::common::fixture::write_file(&full_path, content)?;
        Ok(())
    }

    /// Deletes a file from the test directory
    pub fn delete_file(&self, path: &str) -> anyhow::Result<()> {
        let full_path = self.temp_dir.path().join(path);
        if full_path.exists() {
            std::fs::remove_file(full_path)?;
        }
        Ok(())
    }

    /// Returns the path to the test directory
    pub fn path(&self) -> &Path {
        self.temp_dir.path()
    }
}

impl Drop for DaemonHarness {
    fn drop(&mut self) {
        if let Some(mut child) = self.daemon_child.take() {
            let _ = child.kill();
        }
    }
}

/// Finds the avcs binary path
fn find_avcs_binary() -> anyhow::Result<PathBuf> {
    // Try target/debug/avcs
    let debug_path = std::env::current_dir()?.join("target/debug/avcs");
    if debug_path.exists() {
        return Ok(debug_path);
    }

    // Try target/release/avcs first (release build)
    let release_path = std::env::current_dir()?.join("target/release/avcs");
    if release_path.exists() {
        return Ok(release_path);
    }

    // Try cargo build --release
    anyhow::bail!("avcs binary not found. Run 'cargo build --release' first.")
}