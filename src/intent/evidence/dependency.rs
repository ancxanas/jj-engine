use crate::core::types::{Evidence, EvidenceKind, StructuralChange};

#[must_use]
pub fn detect(changes: &[StructuralChange]) -> Vec<Evidence> {
    let mut evidence = Vec::new();

    for change in changes {
        let file_name = change
            .location
            .file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        let file_name_lower = file_name.to_ascii_lowercase();

        // Package files
        if matches!(
            file_name_lower.as_str(),
            "package.json" | "cargo.toml" | "pyproject.toml" | "go.mod" | "requirements.txt"
        ) {
            evidence.push(Evidence {
                kind: EvidenceKind::PackageFileChanged,
                description: format!("package file modified: {file_name}"),
                location: change.location.clone(),
            });
        }
        // Lock files
        else if matches!(
            file_name_lower.as_str(),
            "package-lock.json"
                | "yarn.lock"
                | "pnpm-lock.yaml"
                | "bun.lockb"
                | "cargo.lock"
                | "poetry.lock"
                | "pipfile.lock"
                | "composer.lock"
        ) {
            evidence.push(Evidence {
                kind: EvidenceKind::LockFileChanged,
                description: format!("lock file modified: {file_name}"),
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
    fn detects_package_file() {
        let changes = vec![StructuralChange {
            kind: StructuralChangeKind::ImportAdded,
            name: "lodash".to_string(),
            detail: "added".to_string(),
            location: Location {
                file: std::path::PathBuf::from("package.json"),
                line: 1,
                column: 0,
            },
        }];
        let evidence = detect(&changes);
        assert_eq!(evidence.len(), 1);
        assert!(matches!(evidence[0].kind, EvidenceKind::PackageFileChanged));
    }

    #[test]
    fn detects_lock_file() {
        let changes = vec![StructuralChange {
            kind: StructuralChangeKind::ImportAdded,
            name: "react".to_string(),
            detail: "modified".to_string(),
            location: Location {
                file: std::path::PathBuf::from("package-lock.json"),
                line: 1,
                column: 0,
            },
        }];
        let evidence = detect(&changes);
        assert_eq!(evidence.len(), 1);
        assert!(matches!(evidence[0].kind, EvidenceKind::LockFileChanged));
    }

    #[test]
    fn no_evidence_for_other_files() {
        let changes = vec![StructuralChange {
            kind: StructuralChangeKind::FunctionModified,
            name: "greet".to_string(),
            detail: "modified".to_string(),
            location: Location {
                file: std::path::PathBuf::from("src/utils.ts"),
                line: 10,
                column: 0,
            },
        }];
        let evidence = detect(&changes);
        assert!(evidence.is_empty());
    }
}
