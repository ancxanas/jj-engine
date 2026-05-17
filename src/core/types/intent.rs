use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::{
    ChangePattern, Evidence, FileChange, FileRelationship, ImpactAssessment, PolicyDecision,
    StructuralChange,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Intent {
    pub id: String,
    pub pattern: ChangePattern,
    pub suggested_message: String,
    pub files: Vec<FileChange>,
    pub evidence: Vec<Evidence>,
    pub clustering_reason: String,
    pub relationships: Vec<FileRelationship>,
    pub impact: ImpactAssessment,
    pub order: usize,
    pub status: IntentStatus,
    pub policy: PolicyDecision,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum IntentStatus {
    Pending,
    Approved,
    Applied,
    Held,
    Rejected,
    Failed,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AmbiguousChange {
    pub files: Vec<PathBuf>,
    pub candidates: Vec<(ChangePattern, Vec<Evidence>)>,
    pub raw_changes: Vec<StructuralChange>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UnclassifiedChange {
    pub files: Vec<PathBuf>,
    pub raw_changes: Vec<StructuralChange>,
    pub reason: String,
}
