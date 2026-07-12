use crate::core::output::renderer::Renderer;
use crate::core::types::{AnalysisResult, PolicyDecision};

pub fn run(json: bool) -> anyhow::Result<()> {
    let result = super::analyze::analyze()?;

    if json {
        let output = crate::cli::output::json::JsonRenderer.render(&result);
        println!("{output}");
    } else {
        println!("{}", render_preview(&result));
    }

    Ok(())
}

#[allow(clippy::format_push_string)]
fn render_preview(result: &AnalysisResult) -> String {
    let mut output = String::new();

    if result.intents.is_empty() && result.ambiguous.is_empty() && result.unclassified.is_empty() {
        output.push_str("No changes to preview.\n");
        return output;
    }

    for intent in &result.intents {
        let prefix = match &intent.policy {
            PolicyDecision::AutoCommittable => String::new(),
            PolicyDecision::RequiresReview => "[REVIEW] ".into(),
            PolicyDecision::Blocked { reason } => format!("[BLOCKED: {reason}] "),
        };

        let policy_tag = match &intent.policy {
            PolicyDecision::AutoCommittable => "auto",
            PolicyDecision::RequiresReview => "review",
            PolicyDecision::Blocked { .. } => "blocked",
        };

        output.push_str(&format!(
            "{prefix}{}\n({:?} | {})\n\n",
            intent.suggested_message, intent.pattern, policy_tag,
        ));
    }

    if !result.ambiguous.is_empty() {
        output.push_str("-- Ambiguous (cannot auto-commit) --\n\n");
        for ambiguous in &result.ambiguous {
            let candidates: Vec<_> = ambiguous
                .candidates
                .iter()
                .map(|(p, _)| format!("{p:?}"))
                .collect();
            output.push_str(&format!(
                "  {} | candidates: {}\n",
                ambiguous
                    .files
                    .iter()
                    .map(|f| f.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
                candidates.join(", "),
            ));
        }
        output.push('\n');
    }

    if !result.unclassified.is_empty() {
        output.push_str("-- Unclassified --\n\n");
        for unclassified in &result.unclassified {
            output.push_str(&format!(
                "  {} | {}\n",
                unclassified
                    .files
                    .iter()
                    .map(|f| f.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
                unclassified.reason,
            ));
        }
        output.push('\n');
    }

    output
}
