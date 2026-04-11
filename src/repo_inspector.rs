//! Repository state observation via jj-lib.

use std::path::Path;
use std::path::PathBuf;

use anyhow::Result;
use jj_lib::config::StackedConfig;
use jj_lib::object_id::ObjectId;
use jj_lib::repo::Repo;
use jj_lib::repo::StoreFactories;
use jj_lib::settings::UserSettings;
use jj_lib::workspace::Workspace;
use jj_lib::workspace::default_working_copy_factories;
use pollster::FutureExt as _;

/// The state of a JJ repository at a point in time.
/// Purely descriptive. No decisions made here.
#[derive(Debug, Clone)]
pub struct RepoState {
    /// Root path of the repository.
    pub root: PathBuf,

    /// Files modified in the working copy.
    pub modified_files: Vec<PathBuf>,

    /// Files not yet tracked by JJ.
    pub untracked_files: Vec<PathBuf>,

    /// Files with conflicts.
    pub conflicted_files: Vec<PathBuf>,

    /// Whether the working copy has any changes.
    pub has_changes: bool,

    /// Whether there are conflicts.
    pub has_conflicts: bool,

    /// The current change ID (JJ concept).
    pub current_change_id: String,

    /// The current commit ID.
    pub current_commit_id: String,

    /// Description of the current commit, if any.
    pub current_description: Option<String>,

    /// Whether the current commit is empty.
    pub is_empty_commit: bool,

    /// Whether the current commit is safe to rewrite.
    pub is_safe_to_rewrite: bool,

    /// Bookmarks in the repository.
    pub bookmarks: Vec<BookmarkState>,

    /// Whether a remote is configured.
    pub has_remote: bool,
}

/// The state of a single bookmark.
#[derive(Debug, Clone)]
pub struct BookmarkState {
    /// Name of the bookmark.
    pub name: String,

    /// Whether this bookmark tracks a remote.
    pub is_tracked: bool,

    /// Whether local is ahead of remote.
    pub is_ahead_of_remote: bool,

    /// Whether local is behind remote.
    pub is_behind_remote: bool,
}

/// Opens a JJ workspace at the given path and reads basic repo state.
pub fn inspect(path: &Path) -> Result<RepoState> {
    // Build config and settings
    let config = StackedConfig::with_defaults();
    let settings = UserSettings::from_config(config)?;
    let store_factories = StoreFactories::default();
    let working_copy_factories = default_working_copy_factories();

    // Open workspace
    let workspace = Workspace::load(
        &settings,
        path,
        &store_factories,
        &working_copy_factories,
    )?;

    // Load repo at latest operation
    let repo = workspace.repo_loader().load_at_head().block_on()?;

    // Get workspace name
    let workspace_name = workspace.workspace_name();

    // Get working copy commit id
    let wc_commit_id = repo
        .view()
        .get_wc_commit_id(workspace_name)
        .ok_or_else(|| anyhow::anyhow!("no working copy commit found"))?;

    // Load the commit
    let store = repo.store();
    let commit = store.get_commit(wc_commit_id)?;

    // Read commit info
    let current_commit_id = commit.id().hex();
    let current_change_id = commit.change_id().hex();

    let description = commit.description().to_string();
    let current_description = if description.is_empty() {
        None
    } else {
        Some(description)
    };

    // Check if commit is empty by comparing tree to parent tree
    let is_empty_commit = if let Some(parent_id) = commit.parent_ids().first() {
        let parent = store.get_commit(parent_id)?;
        commit.tree_ids() == parent.tree_ids()
    } else {
        true
    };

    let root = workspace.workspace_root().to_path_buf();

    Ok(RepoState {
        root,
        modified_files: Vec::new(),
        untracked_files: Vec::new(),
        conflicted_files: Vec::new(),
        has_changes: false,
        has_conflicts: false,
        current_change_id,
        current_commit_id,
        current_description,
        is_empty_commit,
        is_safe_to_rewrite: true,
        bookmarks: Vec::new(),
        has_remote: false,
    })
}
