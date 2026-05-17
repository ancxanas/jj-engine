use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::StructuralChange;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileChange {
    pub path: PathBuf,
    pub change_type: FileChangeType,
    pub structural_changes: Vec<StructuralChange>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum FileChangeType {
    Added,
    Removed,
    Modified,
    Renamed { from: PathBuf },
}
