use anyhow::Context;
use jj_lib::config::StackedConfig;
use jj_lib::ref_name::WorkspaceNameBuf;
use jj_lib::repo::ReadonlyRepo;
use jj_lib::repo::StoreFactories;
use jj_lib::settings::UserSettings;
use jj_lib::workspace::default_working_copy_factories;
use jj_lib::workspace::Workspace;
use std::path::Path;
use std::sync::Arc;

pub struct RepoHandle {
    pub workspace_name: WorkspaceNameBuf,
    pub repo: Arc<ReadonlyRepo>,
    pub workspace: Workspace,
    runtime: tokio::runtime::Runtime,
}

impl RepoHandle {
    pub fn repo_mut(&self) -> Arc<ReadonlyRepo> {
        self.repo.clone()
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn workspace_mut(&self) -> &Workspace {
        &self.workspace
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn runtime(&self) -> &tokio::runtime::Runtime {
        &self.runtime
    }

    pub fn get_wc_commit_id(&self) -> Option<jj_lib::backend::CommitId> {
        self.repo
            .view()
            .get_wc_commit_id(&self.workspace_name)
            .cloned()
    }

    pub fn start_transaction(&self) -> anyhow::Result<jj_lib::transaction::Transaction> {
        Ok(self.repo.start_transaction())
    }
}

pub fn open(project_root: &Path) -> anyhow::Result<RepoHandle> {
    let settings = UserSettings::from_config(StackedConfig::with_defaults())
        .context("failed to load jj settings")?;
    let working_copy_factories = default_working_copy_factories();
    let workspace = Workspace::load(
        &settings,
        project_root,
        &StoreFactories::default(),
        &working_copy_factories,
    )
    .context("failed to open jj workspace")?;

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to create tokio runtime")?;

    let repo = runtime
        .block_on(workspace.repo_loader().load_at_head())
        .context("failed to load jj repo")?;

    let workspace_name = WorkspaceNameBuf::from(workspace.workspace_name().as_str());

    Ok(RepoHandle {
        workspace_name,
        repo,
        workspace,
        runtime,
    })
}
