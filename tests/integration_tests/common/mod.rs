//! Common test utilities for AVCS integration tests

pub mod daemon;
pub mod fixture;
pub mod git;
pub mod assertions;

// Re-export commonly used types
pub use daemon::DaemonHarness;
pub use assertions::CommitAssertions;

use std::path::Path;

/// Creates a temporary directory for testing
pub fn temp_dir() -> anyhow::Result<tempfile::TempDir> {
    tempfile::tempdir()
}

/// Writes a file to the given path, creating parent directories if needed
pub fn write_file(path: &Path, content: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(())
}