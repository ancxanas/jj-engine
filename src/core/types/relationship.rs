use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileRelationship {
    pub from: PathBuf,
    pub to: PathBuf,
    pub kind: RelationshipKind,
    pub strength: RelationshipStrength,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum RelationshipKind {
    TestOf,
    Imports,
    SharedSymbol,
    DirectoryPeer,
    SharedPattern,
    CoChangeHistory,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum RelationshipStrength {
    Strong,
    Medium,
    Weak,
}
