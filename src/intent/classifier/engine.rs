use std::path::Path;

use super::rules;
use crate::core::types::{ChangePattern, Evidence, StructuralChange};

#[derive(Debug, Clone)]
pub struct Reasoning {
    pub rule: String,
    pub evidence_summary: String,
    pub confidence: f64,
}

#[derive(Debug, Clone)]
pub enum ClassificationResult {
    Classified {
        pattern: ChangePattern,
        evidence: Vec<Evidence>,
        rule: &'static str,
        auto_resolved: bool,
        reasoning: Reasoning,
    },
    Ambiguous {
        candidates: Vec<(ChangePattern, &'static str)>,
        evidence: Vec<Evidence>,
        reasoning: Reasoning,
    },
    Unclassified {
        raw_changes: Vec<StructuralChange>,
        reason: String,
        reasoning: Reasoning,
    },
}

#[must_use]
pub fn classify(
    path: &Path,
    changes: &[StructuralChange],
    evidence: &[Evidence],
) -> ClassificationResult {
    let tier1_rules = rules::tier1_rules();
    let mut tier1_matched = Vec::new();

    for rule in &tier1_rules {
        if (rule.matches)(path, changes) {
            tier1_matched.push((rule.pattern.clone(), rule.name));
        }
    }

    if tier1_matched.len() == 1 {
        let (pattern, rule) = &tier1_matched[0];
        return ClassificationResult::Classified {
            pattern: pattern.clone(),
            evidence: evidence.to_vec(),
            rule,
            auto_resolved: false,
            reasoning: Reasoning {
                rule: rule.to_string(),
                evidence_summary: format!("{} evidence items", evidence.len()),
                confidence: 1.0,
            },
        };
    }

    if tier1_matched.len() >= 2 {
        let rules_str: Vec<&str> = tier1_matched.iter().map(|(_, r)| *r).collect();
        return ClassificationResult::Ambiguous {
            candidates: tier1_matched,
            evidence: evidence.to_vec(),
            reasoning: Reasoning {
                rule: format!("multiple tier1 rules: {}", rules_str.join(", ")),
                evidence_summary: format!("{} evidence items", evidence.len()),
                confidence: 0.5,
            },
        };
    }

    let tier2_rules = rules::tier2_rules();
    let mut tier2_matched: Vec<(ChangePattern, &str, f64)> = Vec::new();

    for rule in &tier2_rules {
        if (rule.matches)(evidence) {
            tier2_matched.push((rule.pattern.clone(), rule.name, rule.confidence));
        }
    }

    if tier2_matched.len() == 1 {
        let (pattern, rule, confidence) = &tier2_matched[0];
        return ClassificationResult::Classified {
            pattern: pattern.clone(),
            evidence: evidence.to_vec(),
            rule,
            auto_resolved: false,
            reasoning: Reasoning {
                rule: rule.to_string(),
                evidence_summary: format!("{} evidence items", evidence.len()),
                confidence: *confidence,
            },
        };
    }

    if tier2_matched.len() >= 2 {
        tier2_matched.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        let (pattern, rule, confidence) = &tier2_matched[0];
        return ClassificationResult::Classified {
            pattern: pattern.clone(),
            evidence: evidence.to_vec(),
            rule,
            auto_resolved: true,
            reasoning: Reasoning {
                rule: format!("{} (auto-resolved from {} candidates)", rule, tier2_matched.len()),
                evidence_summary: format!("{} evidence items", evidence.len()),
                confidence: *confidence,
            },
        };
    }

    // Tier 3: Infer pattern from structural changes when no rules match
    if !changes.is_empty() {
        let inferred = infer_pattern_from_changes(changes);
        return ClassificationResult::Classified {
            pattern: inferred.pattern,
            evidence: evidence.to_vec(),
            rule: inferred.rule,
            auto_resolved: true,
            reasoning: Reasoning {
                rule: format!("{} (structural heuristic)", inferred.rule),
                evidence_summary: format!("{} evidence items, {} structural changes", evidence.len(), changes.len()),
                confidence: inferred.confidence,
            },
        };
    }

    ClassificationResult::Unclassified {
        raw_changes: changes.to_vec(),
        reason: "no classification rule matched".into(),
        reasoning: Reasoning {
            rule: "none".to_string(),
            evidence_summary: format!("{} evidence items", evidence.len()),
            confidence: 0.0,
        },
    }
}

struct InferredPattern {
    pattern: ChangePattern,
    rule: &'static str,
    confidence: f64,
}

fn infer_pattern_from_changes(changes: &[StructuralChange]) -> InferredPattern {
    use crate::core::types::StructuralChangeKind;

    let mut has_removals = false;
    let mut has_additions = false;
    let mut has_test_changes = false;

    for change in changes {
        match change.kind {
            StructuralChangeKind::FunctionRemoved
            | StructuralChangeKind::ClassRemoved
            | StructuralChangeKind::MethodRemoved
            | StructuralChangeKind::InterfaceRemoved
            | StructuralChangeKind::TypeRemoved
            | StructuralChangeKind::ExportRemoved
            | StructuralChangeKind::FileRemoved => has_removals = true,
            StructuralChangeKind::FunctionAdded
            | StructuralChangeKind::ClassAdded
            | StructuralChangeKind::MethodAdded
            | StructuralChangeKind::InterfaceAdded
            | StructuralChangeKind::TypeAdded
            | StructuralChangeKind::ImportAdded
            | StructuralChangeKind::FileAdded => has_additions = true,
            StructuralChangeKind::TestCaseAdded
            | StructuralChangeKind::TestCaseModified
            | StructuralChangeKind::DescribeBlockAdded => has_test_changes = true,
            _ => {}
        }
    }

    if has_test_changes {
        return InferredPattern {
            pattern: ChangePattern::TestAddition,
            rule: "structural-test",
            confidence: 0.7,
        };
    }

    if has_removals && !has_additions {
        return InferredPattern {
            pattern: ChangePattern::BreakingChange,
            rule: "structural-removal",
            confidence: 0.6,
        };
    }

    if has_additions && !has_removals {
        return InferredPattern {
            pattern: ChangePattern::Feature,
            rule: "structural-addition",
            confidence: 0.5,
        };
    }

    InferredPattern {
        pattern: ChangePattern::Refactor,
        rule: "structural-mixed",
        confidence: 0.4,
    }
}

#[cfg(test)]
mod tests {
    use crate::core::types::{EvidenceKind, Location, StructuralChangeKind};
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn classifies_readme_as_documentation() -> anyhow::Result<()> {
        let result = classify(&PathBuf::from("README.md"), &[], &[]);
        if let ClassificationResult::Classified { pattern, rule, auto_resolved, reasoning, .. } = result {
            assert_eq!(pattern, ChangePattern::Documentation);
            assert_eq!(rule, "documentation-change");
            assert!(!auto_resolved);
            assert_eq!(reasoning.confidence, 1.0);
            Ok(())
        } else {
            anyhow::bail!("expected Classified")
        }
    }

    #[test]
    fn classifies_package_json_as_dependency() -> anyhow::Result<()> {
        let result = classify(&PathBuf::from("package.json"), &[], &[]);
        if let ClassificationResult::Classified { pattern, auto_resolved, reasoning, .. } = result {
            assert_eq!(pattern, ChangePattern::DependencyUpdate);
            assert!(!auto_resolved);
            assert_eq!(reasoning.confidence, 1.0);
            Ok(())
        } else {
            anyhow::bail!("expected Classified")
        }
    }

    #[test]
    fn unclassified_for_unknown_file() {
        let result = classify(&PathBuf::from("foo.xyz"), &[], &[]);
        if let ClassificationResult::Unclassified { reasoning, .. } = result {
            assert_eq!(reasoning.confidence, 0.0);
        } else {
            panic!("expected Unclassified");
        }
    }

    #[test]
    fn tier2_bugfix_classifies_when_tier1_fails() {
        let evidence = vec![Evidence {
            kind: EvidenceKind::NullGuardAdded,
            description: "null guard".into(),
            location: Location {
                file: PathBuf::from("src/auth.ts"),
                line: 42,
                column: 0,
            },
        }];
        let result = classify(&PathBuf::from("src/auth.ts"), &[], &evidence);
        if let ClassificationResult::Classified { pattern, auto_resolved, reasoning, .. } = result {
            assert_eq!(pattern, ChangePattern::BugFix);
            assert!(!auto_resolved);
            assert_eq!(reasoning.confidence, 0.8);
        } else {
            panic!("expected Classified");
        }
    }

    #[test]
    fn tier2_security_beats_bugfix_when_multiple_match() {
        let evidence = vec![
            Evidence {
                kind: EvidenceKind::AuthCheckAdded,
                description: "auth check".into(),
                location: Location {
                    file: PathBuf::from("src/auth.ts"),
                    line: 50,
                    column: 0,
                },
            },
            Evidence {
                kind: EvidenceKind::NullGuardAdded,
                description: "null guard".into(),
                location: Location {
                    file: PathBuf::from("src/auth.ts"),
                    line: 42,
                    column: 0,
                },
            },
        ];
        let result = classify(&PathBuf::from("src/auth.ts"), &[], &evidence);
        if let ClassificationResult::Classified { pattern, auto_resolved, reasoning, .. } = result {
            assert_eq!(pattern, ChangePattern::Security);
            assert!(auto_resolved);
            assert_eq!(reasoning.confidence, 0.9);
        } else {
            panic!("expected Classified");
        }
    }

    #[test]
    fn tier1_takes_precedence_over_tier2() {
        let evidence = vec![Evidence {
            kind: EvidenceKind::TestCaseAdded,
            description: "new test".into(),
            location: Location {
                file: PathBuf::from("package.json"),
                line: 10,
                column: 0,
            },
        }];
        let result = classify(&PathBuf::from("package.json"), &[], &evidence);
        if let ClassificationResult::Classified { pattern, auto_resolved, reasoning, .. } = result {
            assert_eq!(pattern, ChangePattern::DependencyUpdate);
            assert!(!auto_resolved);
            assert_eq!(reasoning.confidence, 1.0);
        } else {
            panic!("expected Classified");
        }
    }

    #[test]
    fn ambiguous_when_multiple_rules_match() -> anyhow::Result<()> {
        let result = classify(&PathBuf::from("package-lock.json"), &[], &[]);
        if let ClassificationResult::Ambiguous { candidates, reasoning, .. } = result {
            assert!(candidates.len() >= 2, "expected at least 2 candidates");
            let patterns: Vec<_> = candidates.iter().map(|(p, _)| p).collect();
            assert!(patterns.contains(&&ChangePattern::DependencyUpdate));
            assert_eq!(reasoning.confidence, 0.5);
            Ok(())
        } else {
            anyhow::bail!("expected Ambiguous")
        }
    }

    #[test]
    fn reasoning_contains_rule_name_and_evidence_summary() {
        let evidence = vec![Evidence {
            kind: EvidenceKind::ExportAdded,
            description: "new public API".into(),
            location: Location {
                file: PathBuf::from("src/lib.ts"),
                line: 10,
                column: 0,
            },
        }];
        let result = classify(&PathBuf::from("src/lib.ts"), &[], &evidence);
        if let ClassificationResult::Classified { reasoning, .. } = result {
            assert!(!reasoning.rule.is_empty(), "rule name should be set");
            assert!(
                !reasoning.evidence_summary.is_empty(),
                "evidence summary should be set"
            );
            assert!(
                reasoning.confidence > 0.0 && reasoning.confidence <= 1.0,
                "confidence should be in (0, 1]"
            );
        } else {
            panic!("expected Classified");
        }
    }

    #[test]
    fn tier3_infers_refactor_from_file_modified() -> anyhow::Result<()> {
        let changes = vec![StructuralChange {
            kind: StructuralChangeKind::FileModified,
            name: "lib.rs".into(),
            detail: "unsupported language".into(),
            location: Location { file: PathBuf::from("src/lib.rs"), line: 0, column: 0 },
        }];
        let result = classify(&PathBuf::from("src/lib.rs"), &changes, &[]);
        if let ClassificationResult::Classified { pattern, rule, auto_resolved, .. } = result {
            assert_eq!(pattern, ChangePattern::Refactor);
            assert_eq!(rule, "structural-mixed");
            assert!(auto_resolved);
            Ok(())
        } else {
            anyhow::bail!("expected Classified")
        }
    }

    #[test]
    fn tier3_infers_feature_from_file_added() -> anyhow::Result<()> {
        let changes = vec![StructuralChange {
            kind: StructuralChangeKind::FileAdded,
            name: "new.rs".into(),
            detail: "unsupported language".into(),
            location: Location { file: PathBuf::from("src/new.rs"), line: 0, column: 0 },
        }];
        let result = classify(&PathBuf::from("src/new.rs"), &changes, &[]);
        if let ClassificationResult::Classified { pattern, rule, auto_resolved, .. } = result {
            assert_eq!(pattern, ChangePattern::Feature);
            assert_eq!(rule, "structural-addition");
            assert!(auto_resolved);
            Ok(())
        } else {
            anyhow::bail!("expected Classified")
        }
    }

    #[test]
    fn tier3_infers_breaking_from_file_removed() -> anyhow::Result<()> {
        let changes = vec![StructuralChange {
            kind: StructuralChangeKind::FileRemoved,
            name: "old.rs".into(),
            detail: "unsupported language".into(),
            location: Location { file: PathBuf::from("src/old.rs"), line: 0, column: 0 },
        }];
        let result = classify(&PathBuf::from("src/old.rs"), &changes, &[]);
        if let ClassificationResult::Classified { pattern, rule, auto_resolved, .. } = result {
            assert_eq!(pattern, ChangePattern::BreakingChange);
            assert_eq!(rule, "structural-removal");
            assert!(auto_resolved);
            Ok(())
        } else {
            anyhow::bail!("expected Classified")
        }
    }

    #[test]
    fn tier1_beats_tier3_for_cargo_toml() -> anyhow::Result<()> {
        let changes = vec![StructuralChange {
            kind: StructuralChangeKind::FileModified,
            name: "Cargo.toml".into(),
            detail: "unsupported language".into(),
            location: Location { file: PathBuf::from("Cargo.toml"), line: 0, column: 0 },
        }];
        let result = classify(&PathBuf::from("Cargo.toml"), &changes, &[]);
        if let ClassificationResult::Classified { pattern, rule, auto_resolved, .. } = result {
            assert_eq!(pattern, ChangePattern::DependencyUpdate);
            assert_eq!(rule, "package-files");
            assert!(!auto_resolved, "Tier 1 should not be auto-resolved");
            Ok(())
        } else {
            anyhow::bail!("expected Classified")
        }
    }
}
