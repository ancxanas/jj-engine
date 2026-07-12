use anyhow::Context;
use log::{debug, info};
use std::path::PathBuf;

use super::repo::GitRepo;

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

/// Collect working copy diffs by diffing the git index against the working directory.
pub fn get_working_copy_diff(git_repo: &GitRepo) -> anyhow::Result<Vec<FileDiff>> {
    let mut opts = git2::DiffOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true);

    let mut diff = git_repo
        .inner
        .diff_index_to_workdir(None, Some(&mut opts))
        .context("failed to diff index to workdir")?;

    let mut find_opts = git2::DiffFindOptions::new();
    find_opts.rename_limit(200);
    diff.find_similar(Some(&mut find_opts))?;

    let delta_count = diff.deltas().len();
    info!(
        "get_working_copy_diff: found {delta_count} deltas"
    );



    for delta_idx in 0..delta_count {
        let delta = diff.deltas().nth(delta_idx).context("failed to get delta")?;
        debug!(
            "delta[{delta_idx}]: status={:?} old={:?} new={:?}",
            delta.status(),
            delta.old_file().path(),
            delta.new_file().path()
        );
    }

    // Resolve relative paths against the git workdir to avoid CWD issues in daemon child processes.
    let workdir = git_repo
        .inner
        .workdir()
        .context("git repo has no workdir")?
        .to_path_buf();

    let mut diffs = Vec::new();

    for delta_idx in 0..diff.deltas().len() {
        let delta = diff.deltas().nth(delta_idx).context("failed to get delta")?;
        let file = delta.new_file();
        let path = file
            .path()
            .context("diff delta has no path")?
            .to_path_buf();

        // Resolve relative path against workdir for filesystem reads
        let abs_path = workdir.join(&path);

        match delta.status() {
            git2::Delta::Added | git2::Delta::Untracked => {
                let content = std::fs::read(&abs_path)
                    .with_context(|| format!("failed to read added file {}", abs_path.display()))?;
                let hash = ContentHash(Vec::from(file.id().as_bytes()));
                diffs.push(FileDiff::Added { path, content, hash });
            }
            git2::Delta::Deleted => {
                let blob_id = delta.old_file().id();
                let blob = git_repo
                    .inner
                    .find_blob(blob_id)
                    .with_context(|| format!("failed to find blob for deleted file {}", path.display()))?;
                let content = blob.content().to_vec();
                let hash = ContentHash(Vec::from(blob_id.as_bytes()));
                diffs.push(FileDiff::Removed { path, content, hash });
            }
            git2::Delta::Modified | git2::Delta::Renamed | git2::Delta::Copied => {
                let old_blob_id = delta.old_file().id();
                let new_blob_id = delta.new_file().id();

                let before = git_repo
                    .inner
                    .find_blob(old_blob_id)
                    .with_context(|| format!("failed to find old blob for {}", path.display()))?
                    .content()
                    .to_vec();

                let after = if new_blob_id.is_zero() {
                    std::fs::read(&abs_path)
                        .with_context(|| format!("failed to read modified file {}", abs_path.display()))?
                } else {
                    git_repo
                        .inner
                        .find_blob(new_blob_id)
                        .with_context(|| format!("failed to find new blob for {}", path.display()))?
                        .content()
                        .to_vec()
                };

                diffs.push(FileDiff::Modified {
                    path,
                    before,
                    after,
                    before_hash: ContentHash(Vec::from(old_blob_id.as_bytes())),
                    after_hash: ContentHash(Vec::from(new_blob_id.as_bytes())),
                });
            }
            _ => {}
        }
    }

    Ok(diffs)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_diff_index_to_workdir_sees_untracked_files() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();

        // Configure git user
        let mut cfg = repo.config().unwrap();
        cfg.set_str("user.name", "Test").unwrap();
        cfg.set_str("user.email", "test@test.com").unwrap();

        // Create initial commit with a file
        let initial_path = dir.path().join("existing.txt");
        std::fs::write(&initial_path, "existing content").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("existing.txt")).unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = repo.signature().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
            .unwrap();

        // Now create an UNTRACKED file (simulates what test harness does)
        let new_path = dir.path().join("src/features/new-feature.ts");
        std::fs::create_dir_all(new_path.parent().unwrap()).unwrap();
        std::fs::write(&new_path, "export function hello() {}").unwrap();

        // Verify untracked file exists on disk
        assert!(new_path.exists(), "new file should exist on disk");

        // Now try diff_index_to_workdir with include_untracked
        let mut opts = git2::DiffOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true);

        let diff = repo
            .diff_index_to_workdir(None, Some(&mut opts))
            .expect("diff should succeed");

        let delta_count = diff.deltas().len();
        eprintln!("Delta count: {delta_count}");
        for i in 0..delta_count {
            let delta = diff.deltas().nth(i).unwrap();
            eprintln!(
                "  delta[{i}]: status={:?} path={:?}",
                delta.status(),
                delta.new_file().path()
            );
        }

        assert_eq!(delta_count, 1, "should see exactly 1 untracked file");
        let delta = diff.deltas().nth(0).unwrap();
        assert!(
            matches!(delta.status(), git2::Delta::Added | git2::Delta::Untracked),
            "expected Added or Untracked, got {:?}",
            delta.status()
        );
        assert_eq!(
            delta.new_file().path().unwrap(),
            std::path::Path::new("src/features/new-feature.ts")
        );
    }

    #[test]
    fn test_diff_after_initial_commit_then_new_files() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();

        let mut cfg = repo.config().unwrap();
        cfg.set_str("user.name", "Test").unwrap();
        cfg.set_str("user.email", "test@test.com").unwrap();

        // Simulate what init_avcs does: commit just the config
        let config_dir = dir.path().join(".autonomous-vcs");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("config.toml"), "[daemon]\nenabled = true\n").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new(".autonomous-vcs/config.toml")).unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = repo.signature().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Initial commit with AVCS config", &tree, &[])
            .unwrap();

        // Now simulate test writing new files
        let new_file = dir.path().join("src/features/new-feature.ts");
        std::fs::create_dir_all(new_file.parent().unwrap()).unwrap();
        std::fs::write(&new_file, "export function hello() {}").unwrap();

        let test_file = dir.path().join("tests/features/new-feature.test.ts");
        std::fs::create_dir_all(test_file.parent().unwrap()).unwrap();
        std::fs::write(&test_file, "test('hello', () => {})").unwrap();

        // Verify files exist
        assert!(new_file.exists());
        assert!(test_file.exists());

        // Try diff
        let mut opts = git2::DiffOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true);

        let diff = repo
            .diff_index_to_workdir(None, Some(&mut opts))
            .expect("diff should succeed");

        let delta_count = diff.deltas().len();
        eprintln!("Delta count: {delta_count}");
        for i in 0..delta_count {
            let delta = diff.deltas().nth(i).unwrap();
            eprintln!(
                "  delta[{i}]: status={:?} path={:?}",
                delta.status(),
                delta.new_file().path()
            );
        }

        assert!(delta_count >= 2, "should see at least 2 untracked files, got {delta_count}");
    }

    #[test]
    fn test_diff_with_repository_discover() {
        let dir = tempfile::tempdir().unwrap();

        // Create repo, commit, and drop all borrows
        {
            let repo = git2::Repository::init(dir.path()).unwrap();

            let mut cfg = repo.config().unwrap();
            cfg.set_str("user.name", "Test").unwrap();
            cfg.set_str("user.email", "test@test.com").unwrap();

            // Initial commit
            let config_dir = dir.path().join(".autonomous-vcs");
            std::fs::create_dir_all(&config_dir).unwrap();
            std::fs::write(config_dir.join("config.toml"), "[daemon]\nenabled = true\n").unwrap();

            let mut index = repo.index().unwrap();
            index.add_path(std::path::Path::new(".autonomous-vcs/config.toml")).unwrap();
            index.write().unwrap();
            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            let sig = repo.signature().unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
                .unwrap();
        }

        // Now re-open via discover (what the daemon does)
        let repo = git2::Repository::discover(dir.path()).unwrap();

        // Create new files
        let new_file = dir.path().join("src/features/new-feature.ts");
        std::fs::create_dir_all(new_file.parent().unwrap()).unwrap();
        std::fs::write(&new_file, "export function hello() {}").unwrap();

        let mut opts = git2::DiffOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true);

        let diff = repo
            .diff_index_to_workdir(None, Some(&mut opts))
            .expect("diff should succeed");

        let delta_count = diff.deltas().len();
        eprintln!("Delta count via discover: {delta_count}");
        for i in 0..delta_count {
            let delta = diff.deltas().nth(i).unwrap();
            eprintln!(
                "  delta[{i}]: status={:?} path={:?}",
                delta.status(),
                delta.new_file().path()
            );
        }

        assert!(delta_count >= 1, "should see untracked file via discover");
    }
}
