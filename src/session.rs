//! RepoSession: single consistent connection to a JJ repository.
//!
//! Created once per command. Holds workspace, repo, and working
//! copy state so they are never loaded more than once.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use jj_lib::backend::CommitId;
use jj_lib::gitignore::GitIgnoreFile;
use jj_lib::matchers::EverythingMatcher;
use jj_lib::merged_tree::MergedTree;
use jj_lib::repo::ReadonlyRepo;
use jj_lib::repo::Repo;
use jj_lib::working_copy::SnapshotOptions;
use jj_lib::working_copy::SnapshotStats;
use jj_lib::workspace::Workspace;

use crate::jj_context::JjContext;

/// A single consistent connection to a JJ repository.
/// Created once per command execution.
pub struct RepoSession {
    /// Absolute path to the repository root.
    pub repo_path: PathBuf,

    /// The open workspace (private — use methods for access).
    workspace: Workspace,

    /// The repository at the latest operation head.
    pub repo: Arc<ReadonlyRepo>,

    /// The working copy commit ID.
    pub wc_commit_id: CommitId,

    /// Stats from the snapshot (untracked files etc).
    pub wc_stats: SnapshotStats,
}

impl RepoSession {
    /// Opens a JJ workspace, snapshots the working copy, and
    /// loads the repo. All in one call. Reuse this session
    /// for the entire command.
    pub async fn load(path: &Path) -> Result<Self> {
        let repo_path = path.canonicalize()?;
        let ctx = JjContext::new()?;
        let mut workspace = ctx.load_workspace(&repo_path)?;

        // Snapshot working copy to pick up filesystem changes
        let snapshot_options = SnapshotOptions {
            base_ignores: GitIgnoreFile::empty(),
            progress: None,
            start_tracking_matcher: &EverythingMatcher,
            force_tracking_matcher: &EverythingMatcher,
            max_new_file_size: u64::MAX,
        };

        let mut locked_ws = workspace.start_working_copy_mutation()?;
        let (_wc_tree, wc_stats) = locked_ws.locked_wc().snapshot(&snapshot_options).await?;
        let op_id = locked_ws.locked_wc().old_operation_id().clone();
        locked_ws.finish(op_id).await?;

        // Load repo from SAME workspace after snapshot
        let repo = workspace.repo_loader().load_at_head().await?;
        let workspace_name = workspace.workspace_name().to_owned();

        let wc_commit_id = repo
            .view()
            .get_wc_commit_id(&workspace_name)
            .ok_or_else(|| anyhow::anyhow!("no working copy commit found"))?
            .clone();

        Ok(Self {
            repo_path,
            workspace,
            repo,
            wc_commit_id,
            wc_stats,
        })
    }

    /// Returns the working copy tree from repo store.
    /// Always consistent with repo store — no store mismatch risk.
    pub fn wc_tree(&self) -> Result<MergedTree> {
        let commit = self.repo.store().get_commit(&self.wc_commit_id)?;
        Ok(commit.tree())
    }

    /// Returns the workspace root path.
    pub fn workspace_root(&self) -> &Path {
        self.workspace.workspace_root()
    }
}
