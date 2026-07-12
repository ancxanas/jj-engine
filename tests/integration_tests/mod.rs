//! Integration tests for AVCS daemon mode

// Common test utilities
pub mod common {
    pub mod fixture;
    pub mod daemon;
    pub mod git;
    pub mod assertions;
}

// Toy project integration tests
pub mod toy_project {
    pub mod fixture;
    pub mod tests;
}

// Real-world repository integration tests
pub mod real_world {
    pub mod sources;
    pub mod tests;
}

// Re-export commonly used types
pub use common::daemon::DaemonHarness;
pub use common::git::{GitRepo, Commit};
pub use common::assertions::CommitAssertions;
pub use common::fixture::{create_toy_project, avcs_config_toml};
pub use real_world::sources::RealWorldRepo;