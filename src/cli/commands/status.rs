use crate::core::output::renderer::Renderer;
use crate::core::types::{AnalysisResult, ImpactLevel};

pub fn run(json: bool) -> anyhow::Result<()> {
    let result = super::analyze::analyze()?;

    if json {
        let output = crate::cli::output::json::JsonRenderer.render(&result);
        println!("{output}");
    } else {
        println!("{}", render_compact(&result));
    }

    Ok(())
}

fn render_compact(result: &AnalysisResult) -> String {
    let mut lines: Vec<String> = Vec::new();

    lines.push(format!(
        "AVCS Status: {} files changed",
        result.meta.total_files_changed
    ));

    // Intent counts by policy
    lines.push(String::new());
    lines.push("Intents:".into());
    lines.push(format!("  {} auto-committable", result.stats.auto_committable));
    lines.push(format!("  {} requires review", result.stats.requires_review));
    lines.push(format!("  {} blocked", result.stats.blocked));

    // Unclassified and ambiguous counts
    if result.stats.unclassified > 0 {
        lines.push(format!("  {} unclassified", result.stats.unclassified));
    }
    if result.stats.ambiguous > 0 {
        lines.push(format!("  {} ambiguous", result.stats.ambiguous));
    }

    // Impact distribution
    let mut impact_counts = [0; 4]; // Low, Medium, High, Critical
    for intent in &result.intents {
        match intent.impact.level {
            ImpactLevel::Low => impact_counts[0] += 1,
            ImpactLevel::Medium => impact_counts[1] += 1,
            ImpactLevel::High => impact_counts[2] += 1,
            ImpactLevel::Critical => impact_counts[3] += 1,
        }
    }

    lines.push(String::new());
    lines.push("Impact distribution:".into());
    if impact_counts[3] > 0 {
        lines.push(format!("  {} critical", impact_counts[3]));
    }
    if impact_counts[2] > 0 {
        lines.push(format!("  {} high", impact_counts[2]));
    }
    if impact_counts[1] > 0 {
        lines.push(format!("  {} medium", impact_counts[1]));
    }
    if impact_counts[0] > 0 {
        lines.push(format!("  {} low", impact_counts[0]));
    }

    // Footer
    lines.push(String::new());
    lines.push("Run `avcs analyze` for details".into());

    lines.join("\n")
}
