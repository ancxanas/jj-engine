use anyhow::{anyhow, Context};
use futures::io::AsyncReadExt;
use jj_lib::backend::FileId;
use jj_lib::backend::TreeValue;
use jj_lib::matchers::EverythingMatcher;
use jj_lib::merge::Diff;
use jj_lib::merged_tree::TreeDiffIterator;
use jj_lib::object_id::ObjectId;
use jj_lib::repo::ReadonlyRepo;
use jj_lib::repo::Repo;
use jj_lib::repo_path::RepoPath;
use std::path::PathBuf;
use std::sync::Arc;

use super::repo::RepoHandle;

#[derive(Debug, Clone)]
pub struct ContentHash(pub Vec<u8>);

#[derive(Debug, Clone)]
pub enum FileDiff {
    Added {
        path: PathBuf,
        content: Vec<u8>,
        hash: ContentHash,
    },
    Removed {
        path: PathBuf,
        content: Vec<u8>,
        hash: ContentHash,
    },
    Modified {
        path: PathBuf,
        before: Vec<u8>,
        after: Vec<u8>,
        before_hash: ContentHash,
        after_hash: ContentHash,
    },
}

pub fn get_working_copy_diff(repo_handle: &RepoHandle) -> anyhow::Result<Vec<FileDiff>> {
    let repo = &repo_handle.repo;
    let wc_commit_id = repo
        .view()
        .get_wc_commit_id(&repo_handle.workspace_name)
        .context("no working copy commit found")?
        .clone();

    let wc_commit = repo.store().get_commit(&wc_commit_id)?;
    let parent = wc_commit
        .parent_ids()
        .first()
        .context("working copy has no parent")?;
    let parent_commit = repo.store().get_commit(parent)?;

    let parent_tree = parent_commit.tree();
    let wc_tree = wc_commit.tree();

    let mut diffs = Vec::new();
    let runtime = repo_handle.runtime();

    for entry in TreeDiffIterator::new(&parent_tree, &wc_tree, &EverythingMatcher) {
        let Diff { before, after } = entry.values?;
        let before = before.into_resolved().map_err(|_| {
            anyhow!(
                "unresolved tree diff at {}",
                entry.path.as_internal_file_string()
            )
        })?;
        let after = after.into_resolved().map_err(|_| {
            anyhow!(
                "unresolved tree diff at {}",
                entry.path.as_internal_file_string()
            )
        })?;
        let file_path = PathBuf::from(entry.path.as_internal_file_string());

        match (before, after) {
            (None, Some(TreeValue::File { id, .. })) => {
                let content = read_file(repo, &entry.path, &id, runtime)?;
                diffs.push(FileDiff::Added {
                    path: file_path,
                    content,
                    hash: ContentHash(id.to_bytes()),
                });
            }
            (Some(TreeValue::File { id, .. }), None) => {
                let content = read_file(repo, &entry.path, &id, runtime)?;
                diffs.push(FileDiff::Removed {
                    path: file_path,
                    content,
                    hash: ContentHash(id.to_bytes()),
                });
            }
            (
                Some(TreeValue::File { id: before_id, .. }),
                Some(TreeValue::File { id: after_id, .. }),
            ) if before_id != after_id => {
                let before = read_file(repo, &entry.path, &before_id, runtime)?;
                let after = read_file(repo, &entry.path, &after_id, runtime)?;
                diffs.push(FileDiff::Modified {
                    path: file_path,
                    before,
                    after,
                    before_hash: ContentHash(before_id.to_bytes()),
                    after_hash: ContentHash(after_id.to_bytes()),
                });
            }
            _ => {}
        }
    }

    Ok(diffs)
}

fn read_file(
    repo: &Arc<ReadonlyRepo>,
    path: &RepoPath,
    id: &FileId,
    runtime: &tokio::runtime::Runtime,
) -> anyhow::Result<Vec<u8>> {
    let mut reader = runtime
        .block_on(repo.store().read_file(path, id))
        .with_context(|| format!("failed to read file {}", path.as_internal_file_string()))?;
    let mut content = Vec::new();
    runtime
        .block_on(reader.read_to_end(&mut content))
        .with_context(|| {
            format!(
                "failed to read file bytes {}",
                path.as_internal_file_string()
            )
        })?;
    Ok(content)
}
