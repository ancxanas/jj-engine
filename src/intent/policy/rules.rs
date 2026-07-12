use crate::core::types::{ChangePattern, ImpactAssessment, ImpactLevel, StructuralChange, StructuralChangeKind};

const AUTO_COMMIT_PATTERNS: &[ChangePattern] = &[ChangePattern::Documentation];

const BLOCKED_PATTERNS: &[ChangePattern] =
    &[ChangePattern::Security, ChangePattern::BreakingChange];

#[must_use]
pub fn is_auto_committable(pattern: &ChangePattern) -> bool {
    AUTO_COMMIT_PATTERNS.contains(pattern)
}

#[must_use]
pub fn is_blocked(pattern: &ChangePattern) -> bool {
    BLOCKED_PATTERNS.contains(pattern)
}

#[must_use]
pub fn infer_impact(changes: &[StructuralChange], affected_files: usize) -> ImpactAssessment {
    let mut level = ImpactLevel::Low;
    let mut is_public_api_change = false;
    let mut is_breaking = false;
    let mut notes = Vec::new();

    for change in changes {
        match &change.kind {
            StructuralChangeKind::FunctionRemoved
            | StructuralChangeKind::ClassRemoved
            | StructuralChangeKind::InterfaceRemoved => {
                level = ImpactLevel::Critical;
                is_breaking = true;
                is_public_api_change = true;
                notes.push(format!("removed {}", change.name));
            }
            StructuralChangeKind::ExportRemoved | StructuralChangeKind::ExportModified => {
                if level != ImpactLevel::Critical {
                    level = ImpactLevel::High;
                }
                is_public_api_change = true;
                notes.push(format!("modified export {}", change.name));
            }
            StructuralChangeKind::FunctionModified
            | StructuralChangeKind::ClassModified
            | StructuralChangeKind::InterfaceModified => {
                if level != ImpactLevel::Critical && level != ImpactLevel::High {
                    level = ImpactLevel::High;
                }
                is_public_api_change = true;
                notes.push(format!("modified {}", change.name));
            }
            _ => {}
        }
    }

    if affected_files >= 5 && level == ImpactLevel::Low {
        level = ImpactLevel::Medium;
        notes.push(format!("{affected_files} files affected"));
    }

    ImpactAssessment {
        level,
        affected_files,
        is_public_api_change,
        is_breaking,
        notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documentation_is_auto_committable() {
        assert!(is_auto_committable(&ChangePattern::Documentation));
    }

    #[test]
    fn bugfix_is_not_auto_committable() {
        assert!(!is_auto_committable(&ChangePattern::BugFix));
    }

    #[test]
    fn security_is_blocked() {
        assert!(is_blocked(&ChangePattern::Security));
    }

    #[test]
    fn breaking_change_is_blocked() {
        assert!(is_blocked(&ChangePattern::BreakingChange));
    }

    #[test]
    fn feature_is_not_blocked() {
        assert!(!is_blocked(&ChangePattern::Feature));
    }

    #[test]
    fn infer_impact_low_for_single_addition() {
        let changes = vec![StructuralChange {
            kind: StructuralChangeKind::FunctionAdded,
            name: "helper".into(),
            detail: String::new(),
            location: crate::core::types::Location {
                file: "src/lib.rs".into(),
                line: 10,
                column: 0,
            },
        }];
        let impact = infer_impact(&changes, 1);
        assert_eq!(impact.level, ImpactLevel::Low);
        assert!(!impact.is_breaking);
        assert!(!impact.is_public_api_change);
    }

    #[test]
    fn infer_impact_critical_for_removal() {
        let changes = vec![StructuralChange {
            kind: StructuralChangeKind::FunctionRemoved,
            name: "old_api".into(),
            detail: String::new(),
            location: crate::core::types::Location {
                file: "src/lib.rs".into(),
                line: 10,
                column: 0,
            },
        }];
        let impact = infer_impact(&changes, 1);
        assert_eq!(impact.level, ImpactLevel::Critical);
        assert!(impact.is_breaking);
        assert!(impact.is_public_api_change);
    }

    #[test]
    fn infer_impact_medium_for_many_files() {
        let impact = infer_impact(&[], 5);
        assert_eq!(impact.level, ImpactLevel::Medium);
    }
}
