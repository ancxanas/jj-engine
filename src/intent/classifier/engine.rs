use std::path::Path;

use super::rules;
use crate::core::types::{ChangePattern, Evidence, StructuralChange};

#[derive(Debug, Clone)]
pub enum ClassificationResult {
    Classified {
        pattern: ChangePattern,
        evidence: Vec<Evidence>,
        rule: &'static str,
    },
    Unclassified {
        raw_changes: Vec<StructuralChange>,
        reason: String,
    },
}

#[must_use]
pub fn classify(
    path: &Path,
    changes: &[StructuralChange],
    evidence: &[Evidence],
) -> ClassificationResult {
    let rules = rules::tier1_rules();

    for rule in &rules {
        if (rule.matches)(path, changes) {
            return ClassificationResult::Classified {
                pattern: rule.pattern.clone(),
                evidence: evidence.to_vec(),
                rule: rule.name,
            };
        }
    }

    ClassificationResult::Unclassified {
        raw_changes: changes.to_vec(),
        reason: "no classification rule matched".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn classifies_readme_as_documentation() -> anyhow::Result<()> {
        let result = classify(&PathBuf::from("README.md"), &[], &[]);
        if let ClassificationResult::Classified { pattern, rule, .. } = result {
            assert_eq!(pattern, ChangePattern::Documentation);
            assert_eq!(rule, "documentation-change");
            Ok(())
        } else {
            anyhow::bail!("expected Classified")
        }
    }

    #[test]
    fn classifies_package_json_as_dependency() -> anyhow::Result<()> {
        let result = classify(&PathBuf::from("package.json"), &[], &[]);
        if let ClassificationResult::Classified { pattern, .. } = result {
            assert_eq!(pattern, ChangePattern::DependencyUpdate);
            Ok(())
        } else {
            anyhow::bail!("expected Classified")
        }
    }

    #[test]
    fn unclassified_for_unknown_file() {
        let result = classify(&PathBuf::from("src/utils/helpers.ts"), &[], &[]);
        assert!(matches!(result, ClassificationResult::Unclassified { .. }));
    }
}
