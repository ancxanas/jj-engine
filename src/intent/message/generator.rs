use std::path::PathBuf;

use crate::core::types::{ChangePattern, Evidence};
use crate::intent::cluster::partition::IntentGroup;

#[must_use]
#[allow(clippy::format_push_string)]
pub fn generate(group: &IntentGroup, evidence: &[Evidence], rule: &str) -> String {
    let commit_type = pattern_to_type(&group.pattern);
    let scope = infer_scope(&group.files);
    let description = infer_description(group, evidence);

    let subject = scope.map_or_else(
        || format!("{commit_type}: {description}"),
        |scope| format!("{commit_type}({scope}): {description}"),
    );

    let mut message = subject;
    message.push_str("\n\nAVCS-EVIDENCE:\n");
    message.push_str(&format!("  Pattern: {:?}\n", group.pattern));
    message.push_str("  Files:\n");
    for file in &group.files {
        message.push_str(&format!("    - {}\n", file.display()));
    }
    if !evidence.is_empty() {
        message.push_str("  Structural-Changes:\n");
        for item in evidence {
            message.push_str(&format!("    - {:?}: {}\n", item.kind, item.description));
        }
    }
    message.push_str(&format!("  Rule: {rule}\n"));
    message.push_str(&format!(
        "  Engine-Version: {}\n",
        env!("CARGO_PKG_VERSION")
    ));
    message
}

#[must_use]
#[allow(clippy::match_same_arms)]
const fn pattern_to_type(pattern: &ChangePattern) -> &'static str {
    match pattern {
        ChangePattern::BugFix | ChangePattern::Security => "fix",
        ChangePattern::Refactor | ChangePattern::DeadCodeRemoval => "refactor",
        ChangePattern::Feature => "feat",
        ChangePattern::Performance => "perf",
        ChangePattern::DependencyUpdate | ChangePattern::Configuration | ChangePattern::Unknown => {
            "chore"
        }
        ChangePattern::TestAddition => "test",
        ChangePattern::Documentation => "docs",
        ChangePattern::BreakingChange => "feat!",
    }
}

#[must_use]
fn infer_scope(files: &[PathBuf]) -> Option<String> {
    if files.len() == 1 {
        return files[0]
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(String::from);
    }

    let parents: Vec<_> = files
        .iter()
        .filter_map(|file| file.parent())
        .filter_map(|parent| parent.file_name())
        .filter_map(|name| name.to_str())
        .collect();

    if !parents.is_empty() && parents.iter().all(|parent| *parent == parents[0]) {
        return Some(parents[0].to_string());
    }

    None
}

#[must_use]
fn infer_description(group: &IntentGroup, evidence: &[Evidence]) -> String {
    if let Some(first) = evidence.first() {
        return first.description.clone();
    }

    match group.pattern {
        ChangePattern::Documentation => String::from("update documentation"),
        ChangePattern::DependencyUpdate => String::from("update dependencies"),
        ChangePattern::TestAddition => String::from("update tests"),
        ChangePattern::Configuration => String::from("update configuration"),
        _ => format!(
            "update {}",
            group.files.first().map_or("files", |file| {
                file.file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or("files")
            })
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{EvidenceKind, Location};

    #[test]
    fn generates_docs_message() {
        let group = IntentGroup {
            files: vec![PathBuf::from("README.md")],
            pattern: ChangePattern::Documentation,
            relationships: vec![],
            reason: String::from("documentation file"),
        };
        let message = generate(&group, &[], "documentation-change");
        assert!(message.starts_with("docs(README): update documentation"));
        assert!(message.contains("AVCS-EVIDENCE:"));
        assert!(message.contains("Pattern: Documentation"));
        assert!(message.contains("Rule: documentation-change"));
    }

    #[test]
    fn generates_dependency_message() {
        let group = IntentGroup {
            files: vec![
                PathBuf::from("package.json"),
                PathBuf::from("package-lock.json"),
            ],
            pattern: ChangePattern::DependencyUpdate,
            relationships: vec![],
            reason: String::from("package files"),
        };
        let message = generate(&group, &[], "package-files");
        assert!(message.starts_with("chore: update dependencies"));
    }

    #[test]
    fn includes_evidence_in_footer() {
        let group = IntentGroup {
            files: vec![PathBuf::from("src/auth.ts")],
            pattern: ChangePattern::BugFix,
            relationships: vec![],
            reason: String::from("bug fix"),
        };
        let evidence = vec![crate::core::types::Evidence {
            kind: EvidenceKind::NullGuardAdded,
            description: String::from("null guard in getUser"),
            location: Location {
                file: PathBuf::from("src/auth.ts"),
                line: 42,
                column: 0,
            },
        }];
        let message = generate(&group, &evidence, "null-guard");
        assert!(message.contains("NullGuardAdded: null guard in getUser"));
    }
}
