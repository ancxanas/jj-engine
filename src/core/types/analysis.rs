use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::{AmbiguousChange, Intent, UnclassifiedChange};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AnalysisResult {
    pub meta: AnalysisMeta,
    pub intents: Vec<Intent>,
    pub ambiguous: Vec<AmbiguousChange>,
    pub unclassified: Vec<UnclassifiedChange>,
    pub stats: AnalysisStats,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AnalysisMeta {
    pub timestamp: DateTime<Utc>,
    pub project_root: PathBuf,
    pub jj_change_id: String,
    pub jj_commit_id: String,
    pub total_files_changed: usize,
    pub analysis_duration_ms: u64,
    pub analyzer_version: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AnalysisStats {
    pub total_intents: usize,
    pub auto_committable: usize,
    pub requires_review: usize,
    pub blocked: usize,
    pub ambiguous: usize,
    pub unclassified: usize,
    pub parse_duration_ms: u64,
    pub diff_duration_ms: u64,
    pub cluster_duration_ms: u64,
    pub classify_duration_ms: u64,
}
