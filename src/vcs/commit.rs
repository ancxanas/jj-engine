use anyhow::Context;

use super::repo::GitRepo;

/// Stage specified file paths, create a tree, and commit.
/// Returns the commit SHA string.
pub fn apply_commit(
    git_repo: &GitRepo,
    message: &str,
    files: &[&std::path::Path],
) -> anyhow::Result<String> {
    let mut index = git_repo
        .inner
        .index()
        .context("failed to open git index")?;

    for path in files {
        index
            .add_path(path)
            .with_context(|| format!("failed to stage {}", path.display()))?;
    }

    index.write().context("failed to write git index")?;

    let tree_oid = index.write_tree().context("failed to write tree")?;
    let tree = git_repo
        .inner
        .find_tree(tree_oid)
        .context("failed to find tree")?;

    let signature = git_repo
        .inner
        .signature()
        .context("failed to create commit signature")?;

    let parent_commit = match git_repo.inner.head() {
        Ok(head) => Some(
            head.peel_to_commit()
                .context("HEAD is not a commit")?,
        ),
        Err(_) => None,
    };

    let parents: Vec<&git2::Commit> = parent_commit.as_ref().map_or_else(Vec::new, |c| vec![c]);

    let commit_oid = git_repo
        .inner
        .commit(Some("HEAD"), &signature, &signature, message, &tree, &parents)
        .context("failed to create commit")?;

    Ok(commit_oid.to_string())
}
