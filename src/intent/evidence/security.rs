use crate::core::types::{Evidence, EvidenceKind, StructuralChange};

#[must_use]
pub fn detect(changes: &[StructuralChange]) -> Vec<Evidence> {
    let mut evidence = Vec::new();

    for change in changes {
        let name_lower = change.name.to_ascii_lowercase();

        // Check for security-related patterns
        if name_lower.contains("sanitize") {
            evidence.push(Evidence {
                kind: EvidenceKind::SanitizationAdded,
                description: format!("sanitization added: {}", change.name),
                location: change.location.clone(),
            });
        } else if name_lower.contains("validate") || name_lower.contains("verify") {
            evidence.push(Evidence {
                kind: EvidenceKind::InputValidationAdded,
                description: format!("validation added: {}", change.name),
                location: change.location.clone(),
            });
        } else if name_lower.contains("auth")
            || name_lower.contains("login")
            || name_lower.contains("jwt")
            || name_lower.contains("token")
            || name_lower.contains("crypto")
            || name_lower.contains("hash")
            || name_lower.contains("password")
            || name_lower.contains("permission")
            || name_lower.contains("access")
            || name_lower.contains("role")
        {
            evidence.push(Evidence {
                kind: EvidenceKind::AuthCheckAdded,
                description: format!("authentication/authorization: {}", change.name),
                location: change.location.clone(),
            });
        }
    }

    evidence
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{Location, StructuralChange, StructuralChangeKind};

    #[test]
    fn detects_sanitization() {
        let changes = vec![StructuralChange {
            kind: StructuralChangeKind::FunctionAdded,
            name: "sanitizeInput".to_string(),
            detail: "added".to_string(),
            location: Location {
                file: std::path::PathBuf::from("src/security.ts"),
                line: 10,
                column: 0,
            },
        }];
        let evidence = detect(&changes);
        assert_eq!(evidence.len(), 1);
        assert!(matches!(evidence[0].kind, EvidenceKind::SanitizationAdded));
    }

    #[test]
    fn detects_validation() {
        let changes = vec![StructuralChange {
            kind: StructuralChangeKind::FunctionAdded,
            name: "validateEmail".to_string(),
            detail: "added".to_string(),
            location: Location {
                file: std::path::PathBuf::from("src/utils.ts"),
                line: 5,
                column: 0,
            },
        }];
        let evidence = detect(&changes);
        assert_eq!(evidence.len(), 1);
        assert!(matches!(
            evidence[0].kind,
            EvidenceKind::InputValidationAdded
        ));
    }

    #[test]
    fn detects_auth() {
        let changes = vec![StructuralChange {
            kind: StructuralChangeKind::FunctionAdded,
            name: "checkPermission".to_string(),
            detail: "added".to_string(),
            location: Location {
                file: std::path::PathBuf::from("src/auth.ts"),
                line: 1,
                column: 0,
            },
        }];
        let evidence = detect(&changes);
        assert_eq!(evidence.len(), 1);
        assert!(matches!(evidence[0].kind, EvidenceKind::AuthCheckAdded));
    }

    #[test]
    fn no_evidence_for_irrelevant_names() {
        let changes = vec![StructuralChange {
            kind: StructuralChangeKind::FunctionAdded,
            name: "calculateTotal".to_string(),
            detail: "added".to_string(),
            location: Location {
                file: std::path::PathBuf::from("src/math.ts"),
                line: 1,
                column: 0,
            },
        }];
        let evidence = detect(&changes);
        assert!(evidence.is_empty());
    }
}
