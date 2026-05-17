use anyhow::Context;
use jj_lib::object_id::ObjectId;
use jj_lib::repo::Repo;

use super::repo::RepoHandle;

pub fn apply_commit(
    repo_handle: &RepoHandle,
    message: &str,
    _files: &[&std::path::Path],
) -> anyhow::Result<String> {
    let repo = &repo_handle.repo;
    let wc_commit_id = repo
        .view()
        .get_wc_commit_id(&repo_handle.workspace_name)
        .context("no working copy commit found")?
        .clone();

    let wc_commit = repo.store().get_commit(&wc_commit_id)?;
    let parent_commit_id = wc_commit
        .parent_ids()
        .first()
        .context("working copy has no parent")?
        .clone();

    let mut tx = repo.start_transaction();
    let tx_repo = tx.repo_mut();
    let new_commit = repo_handle
        .runtime()
        .block_on(
            tx_repo
                .rewrite_commit(&wc_commit)
                .set_parents(vec![parent_commit_id])
                .set_description(message)
                .write(),
        )
        .context("failed to write commit")?;

    let workspace_name = repo_handle.workspace_name.clone();
    repo_handle
        .runtime()
        .block_on(tx_repo.edit(workspace_name, &new_commit))
        .context("failed to edit working copy")?;

    let change_id = new_commit.change_id().hex();
    let _ = repo_handle
        .runtime()
        .block_on(tx.write(message))
        .context("failed to write transaction")?;

    Ok(change_id)
}

pub fn create_new_change(repo_handle: &RepoHandle) -> anyhow::Result<()> {
    let repo = &repo_handle.repo;
    let wc_commit_id = repo
        .view()
        .get_wc_commit_id(&repo_handle.workspace_name)
        .context("no working copy commit found")?
        .clone();

    let wc_commit = repo.store().get_commit(&wc_commit_id)?;
    let parent_commit_id = wc_commit
        .parent_ids()
        .first()
        .context("working copy has no parent")?
        .clone();

    let parent_commit = repo.store().get_commit(&parent_commit_id)?;

    let mut tx = repo.start_transaction();
    let tx_repo = tx.repo_mut();
    let new_commit = repo_handle
        .runtime()
        .block_on(
            tx_repo
                .new_commit(vec![parent_commit_id], parent_commit.tree())
                .write(),
        )
        .context("failed to create new commit")?;

    let workspace_name = repo_handle.workspace_name.clone();
    repo_handle
        .runtime()
        .block_on(tx_repo.edit(workspace_name, &new_commit))
        .context("failed to edit working copy")?;

    let _ = repo_handle
        .runtime()
        .block_on(tx.write(""))
        .context("failed to write transaction")?;

    Ok(())
}
