use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ImpactAssessment {
    pub level: ImpactLevel,
    pub affected_files: usize,
    pub is_public_api_change: bool,
    pub is_breaking: bool,
    pub notes: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ImpactLevel {
    Low,
    Medium,
    High,
    Critical,
}
