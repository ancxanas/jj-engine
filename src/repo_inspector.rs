//! Repository state observation via jj-lib.

use std::path::Path;
use std::path::PathBuf;

use crate::jj_context::JjContext;
use anyhow::Result;
use futures::StreamExt as _;
use jj_lib::backend::TreeValue;
use jj_lib::gitignore::GitIgnoreFile;
use jj_lib::matchers::EverythingMatcher;
use jj_lib::object_id::ObjectId;
use jj_lib::repo::Repo;
use jj_lib::working_copy::SnapshotOptions;
use pollster::FutureExt as _;
use tokio::io::AsyncReadExt;

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

/// Opens a JJ workspace at the given path and reads repo state.
pub fn inspect(path: &Path) -> Result<RepoState> {
    // Canonicalize path so all output uses absolute paths
    let path = path.canonicalize()?;

    // Build config and settings
    let ctx = JjContext::new()?;

    // Open workspace
    let mut workspace = ctx.load_workspace(&path)?;

    // Load repo at latest operation
    let repo = workspace.repo_loader().load_at_head().block_on()?;

    // Get workspace name
    let workspace_name = workspace.workspace_name().to_owned();

    // Get working copy commit id
    let wc_commit_id = repo
        .view()
        .get_wc_commit_id(&workspace_name)
        .ok_or_else(|| anyhow::anyhow!("no working copy commit found"))?
        .clone();

    // Load the commit
    let store = repo.store();
    let commit = store.get_commit(&wc_commit_id)?;

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

    // Get commit tree
    let commit_tree = commit.tree();

    // Snapshot the working copy
    let snapshot_options = SnapshotOptions {
        base_ignores: GitIgnoreFile::empty(),
        progress: None,
        start_tracking_matcher: &EverythingMatcher,
        force_tracking_matcher: &EverythingMatcher,
        max_new_file_size: u64::MAX,
    };

    let mut locked_ws = workspace.start_working_copy_mutation()?;
    let (wc_tree, stats) = locked_ws
        .locked_wc()
        .snapshot(&snapshot_options)
        .block_on()?;

    // Diff commit tree against working copy tree
    let mut diff_stream = commit_tree.diff_stream(&wc_tree, &EverythingMatcher);

    let mut modified_files: Vec<PathBuf> = Vec::new();

    while let Some(entry) = diff_stream.next().block_on() {
        let fs_path = path.join(entry.path.as_internal_file_string());
        modified_files.push(fs_path);
    }

    // Collect untracked files
    let untracked_files: Vec<PathBuf> = stats
        .untracked_paths
        .keys()
        .map(|p| path.join(p.as_internal_file_string()))
        .collect();

    let has_changes = !modified_files.is_empty();

    // Collect bookmarks
    let bookmarks = collect_bookmarks(repo.as_ref());

    // Determine if remote exists
    let has_remote = bookmarks.iter().any(|b| b.is_tracked);

    // Determine if safe to rewrite
    // Not safe if any bookmark tracking a remote points to current commit
    let is_safe_to_rewrite = !bookmarks
        .iter()
        .any(|b| b.is_tracked && !b.is_ahead_of_remote && !b.is_behind_remote);

    let root = workspace.workspace_root().to_path_buf();

    Ok(RepoState {
        root,
        modified_files,
        untracked_files,
        conflicted_files: Vec::new(),
        has_changes,
        has_conflicts: false,
        current_change_id,
        current_commit_id,
        current_description,
        is_empty_commit,
        is_safe_to_rewrite,
        bookmarks,
        has_remote,
    })
}

/// Reads all bookmarks and determines their tracking state.
fn collect_bookmarks(repo: &dyn Repo) -> Vec<BookmarkState> {
    let view = repo.view();
    let mut result = Vec::new();

    for (name, target) in view.bookmarks() {
        let local_target = target.local_target;
        let remote_refs = target.remote_refs;

        // Check if this bookmark has any remote tracking
        let is_tracked = !remote_refs.is_empty();

        // Check if local is ahead of remote
        // Local is ahead if local target differs from any remote target
        let is_ahead_of_remote = is_tracked
            && remote_refs
                .iter()
                .any(|(_remote_name, remote_ref)| local_target != &remote_ref.target);

        // Check if local is behind remote
        // Local is behind if local is absent but remote has a target
        let is_behind_remote = is_tracked
            && local_target.is_absent()
            && remote_refs
                .iter()
                .any(|(_remote_name, remote_ref)| remote_ref.target.is_present());

        result.push(BookmarkState {
            name: name.into(),
            is_tracked,
            is_ahead_of_remote,
            is_behind_remote,
        });
    }

    result
}

/// Builds a SemanticSnapshot from the committed tree of the current commit.
/// This represents the "before" state — what was last committed.
/// Builds a SemanticSnapshot from the committed tree of the current commit.
/// This represents the "before" state — what was last committed.
pub fn committed_snapshot(path: &Path) -> Result<crate::semantic::SemanticSnapshot> {
    // Canonicalize path
    let path = path.canonicalize()?;
    let path = path.as_path();

    let ctx = JjContext::new()?;
    let workspace = ctx.load_workspace(path)?;
    let repo = workspace.repo_loader().load_at_head().block_on()?;
    let workspace_name = workspace.workspace_name().to_owned();

    // Get working copy commit
    let wc_commit_id = repo
        .view()
        .get_wc_commit_id(&workspace_name)
        .ok_or_else(|| anyhow::anyhow!("no working copy commit found"))?
        .clone();

    let store = repo.store();
    let commit = store.get_commit(&wc_commit_id)?;

    // Get PARENT commit tree (the "before" state)
    // The working copy commit IS the current state
    let parent_id = commit
        .parent_ids()
        .first()
        .ok_or_else(|| anyhow::anyhow!("no parent commit"))?;
    let parent_commit = store.get_commit(parent_id)?;
    let committed_tree = parent_commit.tree();
    // Walk all entries in committed tree
    let mut entities = std::collections::HashMap::new();
    let mut files_scanned = 0;

    // Create a tokio runtime to run async file reads
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    for (repo_path, value_result) in committed_tree.entries() {
        let merged_value = value_result?;

        // Get resolved value, skip conflicts
        let tree_value = match merged_value.as_normal() {
            Some(v) => v,
            None => continue,
        };

        // Only process regular files
        let file_id = match tree_value {
            TreeValue::File { id, .. } => id.clone(),
            _ => continue,
        };

        // Only process Rust files for now
        let file_name = repo_path.as_internal_file_string().to_string();
        if !file_name.ends_with(".rs") {
            continue;
        }

        // Read file content from committed tree using tokio runtime
        let content = rt.block_on(async {
            let mut reader = store.read_file(&repo_path, &file_id).await?;

            let mut buf = Vec::new();
            reader.read_to_end(&mut buf).await?;
            Ok::<Vec<u8>, anyhow::Error>(buf)
        })?;

        // Convert to string, skip non-utf8 files
        let source = match String::from_utf8(content) {
            Ok(s) => s,
            Err(_) => continue,
        };

        files_scanned += 1;

        // Parse file into entities
        let fs_path = path.join(&file_name);
        let file_entities = crate::semantic::parse_file(&fs_path, &source)?;

        for entity in file_entities {
            entities.insert(entity.path.clone(), entity);
        }
    }

    Ok(crate::semantic::SemanticSnapshot {
        entities,
        files_scanned,
    })
}
