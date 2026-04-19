//! Repository state observation via jj-lib.

use crate::session::RepoSession;

use anyhow::Result;
use futures::StreamExt as _;
use jj_lib::backend::TreeValue;
use jj_lib::matchers::EverythingMatcher;
use jj_lib::object_id::ObjectId;
use jj_lib::repo::Repo;
use std::path::PathBuf;
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

    /// The commit this bookmark points to, if any.
    pub commit_id: Option<String>,

    /// Whether this bookmark tracks a remote.
    pub is_tracked: bool,

    /// Whether local is ahead of remote.
    pub is_ahead_of_remote: bool,

    /// Whether local is behind remote.
    pub is_behind_remote: bool,
}

pub fn inspect_from_session(session: &RepoSession) -> Result<RepoState> {
    let path = session.repo_path.as_path();
    let repo = &session.repo;
    let store = repo.store();
    let commit = store.get_commit(&session.wc_commit_id)?;

    let current_commit_id = commit.id().hex();
    let current_change_id = commit.change_id().hex();

    let description = commit.description().to_string();
    let current_description = if description.is_empty() {
        None
    } else {
        Some(description)
    };

    let is_empty_commit = if let Some(parent_id) = commit.parent_ids().first() {
        let parent = store.get_commit(parent_id)?;
        commit.tree_ids() == parent.tree_ids()
    } else {
        true
    };

    let commit_tree = commit.tree();

    let mut diff_stream = commit_tree.diff_stream(&session.wc_tree, &EverythingMatcher);

    let mut modified_files: Vec<PathBuf> = Vec::new();
    let mut conflicted_files: Vec<PathBuf> = Vec::new();

    while let Some(entry) = futures::executor::block_on(diff_stream.next()) {
        let fs_path = path.join(entry.path.as_internal_file_string());
        if let Ok(values) = entry.values {
            if values.after.iter().any(|v| v.is_none()) {
                conflicted_files.push(fs_path);
            } else {
                modified_files.push(fs_path);
            }
        }
    }

    let untracked_files: Vec<PathBuf> = session
        .wc_stats
        .untracked_paths
        .keys()
        .map(|p| path.join(p.as_internal_file_string()))
        .collect();

    let has_changes = !modified_files.is_empty() || !untracked_files.is_empty();
    let has_conflicts = !conflicted_files.is_empty();

    let bookmarks = collect_bookmarks(repo.as_ref());
    let has_remote = bookmarks.iter().any(|b| b.is_tracked);

    let is_safe_to_rewrite = !bookmarks.iter().any(|b| {
        b.is_tracked
            && !b.is_ahead_of_remote
            && !b.is_behind_remote
            && b.commit_id.as_deref() == Some(&current_commit_id)
    });

    Ok(RepoState {
        root: session.workspace.workspace_root().to_path_buf(),
        modified_files,
        untracked_files,
        conflicted_files,
        has_changes,
        has_conflicts,
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

        let commit_id = local_target.as_normal().map(|id| id.hex());

        result.push(BookmarkState {
            name: name.as_str().to_string(),
            commit_id,
            is_tracked,
            is_ahead_of_remote,
            is_behind_remote,
        });
    }

    result
}

pub async fn committed_snapshot_from_session(
    session: &RepoSession,
) -> Result<crate::semantic::SemanticSnapshot> {
    let path = session.repo_path.as_path();
    let store = session.repo.store();
    let commit = store.get_commit(&session.wc_commit_id)?;

    // Use PARENT commit tree as the "before" state
    let parent_id = commit
        .parent_ids()
        .first()
        .ok_or_else(|| anyhow::anyhow!("no parent commit"))?;
    let parent_commit = store.get_commit(parent_id)?;
    let committed_tree = parent_commit.tree();

    let mut entities = std::collections::HashMap::new();
    let mut files_scanned = 0;

    for (repo_path, value_result) in committed_tree.entries() {
        let merged_value = value_result?;

        let tree_value = match merged_value.as_normal() {
            Some(v) => v,
            None => continue,
        };

        let file_id = match tree_value {
            TreeValue::File { id, .. } => id.clone(),
            _ => continue,
        };

        let file_name = repo_path.as_internal_file_string().to_string();
        if !file_name.ends_with(".rs") {
            continue;
        }

        let content = {
            let mut reader = store.read_file(&repo_path, &file_id).await?;
            let mut buf = Vec::new();
            reader.read_to_end(&mut buf).await?;
            buf
        };

        let source = match String::from_utf8(content) {
            Ok(s) => s,
            Err(_) => continue,
        };

        files_scanned += 1;

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
