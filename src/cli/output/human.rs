use owo_colors::OwoColorize;

use crate::core::output::renderer::Renderer;
use crate::core::types::{AnalysisResult, FileChange, FileChangeType, ImpactLevel, Intent, PolicyDecision};

pub struct HumanRenderer {
    pub verbose: bool,
}

impl Renderer for HumanRenderer {
    #[allow(clippy::format_push_string)]
    fn render(&self, result: &AnalysisResult) -> String {
        let mut output = String::new();

        output.push_str(&format!(
            "\n{}\n  {} files | {}ms\n  commit:{} {}\n",
            "AVCS Analysis".bold(),
            result.meta.total_files_changed,
            result.meta.analysis_duration_ms,
            result.meta.commit_sha,
            result.meta.timestamp.format("%Y-%m-%d %H:%M:%S"),
        ));

        for (index, intent) in result.intents.iter().enumerate() {
            output.push_str(&render_intent(index + 1, intent));
        }

        for ambiguous in &result.ambiguous {
            output.push_str(&format!(
                "\n  {} | {} file(s)\n",
                "Ambiguous".yellow().bold(),
                ambiguous.files.len(),
            ));
            for (pattern, _) in &ambiguous.candidates {
                output.push_str(&format!("    Candidate: {pattern:?}\n"));
            }
            for file in &ambiguous.files {
                output.push_str(&format!("    {}\n", file.display()));
            }
            if self.verbose {
                output.push_str("    Reason: multiple rules matched with equal confidence\n");
            }
        }

        for unclassified in &result.unclassified {
            output.push_str(&format!(
                "\n  {} | {} file(s)\n  {}\n",
                "Unclassified".yellow(),
                unclassified.files.len(),
                unclassified.reason,
            ));
            for file in &unclassified.files {
                output.push_str(&format!("    {}\n", file.display()));
            }
            if self.verbose {
                output.push_str("    Hint: check if file has detectable structural changes\n");
            }
        }

        output.push_str(&format!(
            "\n  {}\n    {} intents found\n    {} auto-committable\n    {} requires review\n    {} blocked\n    {} ambiguous\n    {} unclassified\n",
            "SUMMARY".bold(),
            result.stats.total_intents,
            result.stats.auto_committable,
            result.stats.requires_review,
            result.stats.blocked,
            result.stats.ambiguous,
            result.stats.unclassified,
        ));

        output.push_str(&format!(
            "\n  {}\n    avcs commit        interactive review\n    avcs commit --auto auto-commit safe intents\n\n",
            "NEXT STEPS".bold(),
        ));

        output
    }
}

#[must_use]
#[allow(clippy::format_push_string)]
fn render_intent(index: usize, intent: &Intent) -> String {
    let policy_label = match &intent.policy {
        PolicyDecision::AutoCommittable => "AUTO-COMMITTABLE".green().to_string(),
        PolicyDecision::RequiresReview => "REQUIRES REVIEW".yellow().to_string(),
        PolicyDecision::Blocked { .. } => "BLOCKED".red().to_string(),
    };

    let impact_label = match intent.impact.level {
        ImpactLevel::Low => "Low".dimmed().to_string(),
        ImpactLevel::Medium => "Medium".yellow().to_string(),
        ImpactLevel::High => "High".red().to_string(),
        ImpactLevel::Critical => "Critical".red().bold().to_string(),
    };

    let mut output = format!(
        "\n{}\n\n  {} | {:?} | {} | Impact: {}\n  {}\n",
        "-".repeat(50),
        format!("Intent {index}").bold(),
        intent.pattern,
        policy_label,
        impact_label,
        intent.suggested_message,
    );

    for file in &intent.files {
        output.push_str(&format!(
            "\n    {} {}",
            file.change_type_symbol(),
            file.path.display()
        ));
        for structural_change in &file.structural_changes {
            output.push_str(&format!(
                "\n      {:?}: {}",
                structural_change.kind, structural_change.name
            ));
        }
    }

    if !intent.evidence.is_empty() {
        output.push_str("\n\n  Evidence:\n");
        for evidence in &intent.evidence {
            output.push_str(&format!(
                "    {} {:?}  {}:{}\n",
                "+".green(),
                evidence.kind,
                evidence.location.file.display(),
                evidence.location.line,
            ));
        }
    }

    output.push('\n');
    output
}

trait FileChangeSymbol {
    fn change_type_symbol(&self) -> &'static str;
}

impl FileChangeSymbol for FileChange {
    fn change_type_symbol(&self) -> &'static str {
        match &self.change_type {
            FileChangeType::Added => "+",
            FileChangeType::Removed => "-",
            FileChangeType::Modified => "~",
            FileChangeType::Renamed { .. } => "R",
        }
    }
}
