use crate::core::output::renderer::Renderer;
use crate::core::types::AnalysisResult;

pub struct JsonRenderer;

impl Renderer for JsonRenderer {
    fn render(&self, result: &AnalysisResult) -> String {
        serde_json::to_string_pretty(result)
            .unwrap_or_else(|error| format!("{{\"error\": \"{error}\"}}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{AnalysisMeta, AnalysisStats};
    use chrono::Utc;

    #[test]
    fn renders_valid_json() -> anyhow::Result<()> {
        let result = AnalysisResult {
            meta: AnalysisMeta {
                timestamp: Utc::now(),
                project_root: "/tmp/test".into(),
                commit_sha: String::from("abc123"),
                total_files_changed: 0,
                analysis_duration_ms: 0,
                analyzer_version: String::from("0.1.0"),
            },
            intents: vec![],
            ambiguous: vec![],
            unclassified: vec![],
            stats: AnalysisStats {
                total_intents: 0,
                auto_committable: 0,
                requires_review: 0,
                blocked: 0,
                ambiguous: 0,
                unclassified: 0,
                parse_duration_ms: 0,
                diff_duration_ms: 0,
                cluster_duration_ms: 0,
                classify_duration_ms: 0,
            },
        };
        let rendered = JsonRenderer.render(&result);
        let parsed: serde_json::Value = serde_json::from_str(&rendered)?;
        assert!(parsed.get("meta").is_some());
        assert!(parsed.get("intents").is_some());
        Ok(())
    }
}
