use anyhow::Context;

pub struct GitRepo {
    pub inner: git2::Repository,
}

impl GitRepo {
    /// Open a git repository at the given project root.
    pub fn open(project_root: &std::path::Path) -> anyhow::Result<Self> {
        let inner = git2::Repository::discover(project_root)
            .context("failed to open git repository")?;
        Ok(Self { inner })
    }

    /// Get the current HEAD commit SHA, or an empty string if no commits exist.
    pub fn head_sha(&self) -> anyhow::Result<String> {
        match self.inner.head() {
            Ok(head) => {
                let commit = head
                    .peel_to_commit()
                    .context("HEAD is not a commit")?;
                Ok(commit.id().to_string())
            }
            Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(String::new()),
            Err(e) => Err(e).context("failed to read HEAD"),
        }
    }

    /// Find a commit by its SHA string and return its full message.
    pub fn find_commit_message(&self, sha: &str) -> anyhow::Result<String> {
        let oid = git2::Oid::from_str(sha).context("invalid commit SHA")?;
        let commit = self
            .inner
            .find_commit(oid)
            .with_context(|| format!("commit not found: {sha}"))?;
        Ok(commit.message().unwrap_or("").to_string())
    }
}
