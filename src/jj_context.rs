//! Shared JJ workspace context.
//! Centralizes settings, store factories, and workspace loading
//! so every module uses the same setup without duplication.

use std::path::Path;

use anyhow::Result;
use jj_lib::config::StackedConfig;
use jj_lib::repo::StoreFactories;
use jj_lib::settings::UserSettings;
use jj_lib::workspace::WorkingCopyFactories;
use jj_lib::workspace::Workspace;
use jj_lib::workspace::default_working_copy_factories;

/// Shared context for opening JJ workspaces.
/// Build once and reuse across functions that need workspace access.
pub struct JjContext {
    settings: UserSettings,
    store_factories: StoreFactories,
    working_copy_factories: WorkingCopyFactories,
}

impl JjContext {
    /// Creates a new context using default JJ configuration.
    pub fn new() -> Result<Self> {
        let config = StackedConfig::with_defaults();
        let settings = UserSettings::from_config(config)?;
        let store_factories = StoreFactories::default();
        let working_copy_factories = default_working_copy_factories();

        Ok(Self {
            settings,
            store_factories,
            working_copy_factories,
        })
    }

    /// Opens a JJ workspace at the given path.
    pub fn load_workspace(&self, path: &Path) -> Result<Workspace> {
        Ok(Workspace::load(
            &self.settings,
            path,
            &self.store_factories,
            &self.working_copy_factories,
        )?)
    }
}
