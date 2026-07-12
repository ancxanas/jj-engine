use std::path::Path;

use crate::core::types::{ChangePattern, Evidence, EvidenceKind, StructuralChange};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleTier {
    Decisive,
    Strong,
    Weak,
}

pub struct Rule {
    pub name: &'static str,
    pub tier: RuleTier,
    pub pattern: ChangePattern,
    pub matches: fn(&Path, &[StructuralChange]) -> bool,
}

pub struct EvidenceRule {
    pub name: &'static str,
    pub pattern: ChangePattern,
    pub confidence: f64,
    pub matches: fn(&[Evidence]) -> bool,
}

#[must_use]
pub fn tier1_rules() -> Vec<Rule> {
    vec![
        Rule {
            name: "documentation-change",
            tier: RuleTier::Decisive,
            pattern: ChangePattern::Documentation,
            matches: |path, _| is_doc_file(path),
        },
        Rule {
            name: "package-files",
            tier: RuleTier::Decisive,
            pattern: ChangePattern::DependencyUpdate,
            matches: |path, _| is_package_file(path),
        },
        Rule {
            name: "lockfile-only",
            tier: RuleTier::Decisive,
            pattern: ChangePattern::DependencyUpdate,
            matches: |path, _| is_lockfile(path),
        },
        Rule {
            name: "test-only",
            tier: RuleTier::Decisive,
            pattern: ChangePattern::TestAddition,
            matches: |path, _| is_test_file(path),
        },
        Rule {
            name: "ci-config",
            tier: RuleTier::Decisive,
            pattern: ChangePattern::Refactor,
            matches: |path, _| is_ci_config(path),
        },
        Rule {
            name: "docker-file",
            tier: RuleTier::Decisive,
            pattern: ChangePattern::Refactor,
            matches: |path, _| is_docker_file(path),
        },
        Rule {
            name: "migration-file",
            tier: RuleTier::Decisive,
            pattern: ChangePattern::Feature,
            matches: |path, _| is_migration_file(path),
        },
    ]
}

#[must_use]
pub fn tier2_rules() -> Vec<EvidenceRule> {
    vec![
        EvidenceRule {
            name: "bugfix-evidence",
            pattern: ChangePattern::BugFix,
            confidence: 0.8,
            matches: |evidence| {
                evidence.iter().any(|e| {
                    matches!(
                        e.kind,
                        EvidenceKind::NullGuardAdded
                            | EvidenceKind::ErrorHandlerAdded
                            | EvidenceKind::OptionalChainingAdded
                    )
                })
            },
        },
        EvidenceRule {
            name: "feature-evidence",
            pattern: ChangePattern::Feature,
            confidence: 0.8,
            matches: |evidence| {
                evidence.iter().any(|e| {
                    matches!(
                        e.kind,
                        EvidenceKind::ExportAdded | EvidenceKind::NewPublicApi
                    )
                })
            },
        },
        EvidenceRule {
            name: "security-evidence",
            pattern: ChangePattern::Security,
            confidence: 0.9,
            matches: |evidence| {
                evidence.iter().any(|e| {
                    matches!(
                        e.kind,
                        EvidenceKind::AuthCheckAdded
                            | EvidenceKind::InputValidationAdded
                            | EvidenceKind::SanitizationAdded
                    )
                })
            },
        },
        EvidenceRule {
            name: "refactor-evidence",
            pattern: ChangePattern::Refactor,
            confidence: 0.7,
            matches: |evidence| {
                evidence.iter().any(|e| {
                    matches!(
                        e.kind,
                        EvidenceKind::SymbolRenamed | EvidenceKind::CodeMoved
                    )
                })
            },
        },
        EvidenceRule {
            name: "test-evidence",
            pattern: ChangePattern::TestAddition,
            confidence: 0.8,
            matches: |evidence| {
                evidence.iter().any(|e| {
                    matches!(
                        e.kind,
                        EvidenceKind::TestCaseAdded | EvidenceKind::TestCaseModified
                    )
                })
            },
        },
    ]
}

fn is_doc_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("md" | "mdx" | "txt" | "rst")
    )
}

fn is_package_file(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|f| f.to_str()),
        Some("package.json" | "package-lock.json" | "Cargo.toml" | "go.mod" | "pyproject.toml")
    )
}

fn is_lockfile(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|f| f.to_str()),
        Some("package-lock.json" | "yarn.lock" | "pnpm-lock.yaml" | "bun.lockb" | "Cargo.lock")
    )
}

#[allow(clippy::case_sensitive_file_extension_comparisons)]
fn is_test_file(path: &Path) -> bool {
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    name.ends_with(".test")
        || name.ends_with(".spec")
        || name.ends_with("_test")
        || name.ends_with("_spec")
}

fn is_ci_config(path: &Path) -> bool {
    let s = path.to_string_lossy();
    s.contains(".github/workflows/")
        || s.contains(".gitlab-ci")
        || s.contains(".circleci/")
        || s.contains("Jenkinsfile")
        || s.contains(".travis")
}

fn is_docker_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().to_string());
    name == "Dockerfile"
        || name == "docker-compose.yml"
        || name == "docker-compose.yaml"
        || name.ends_with(".dockerfile")
}

fn is_migration_file(path: &Path) -> bool {
    let s = path.to_string_lossy();
    (s.contains("migrations/") || s.contains("migrate"))
        && (s.ends_with(".sql") || s.ends_with(".ts") || s.ends_with(".js"))
}

#[cfg(test)]
fn is_config_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
    matches!(
        name,
        "tsconfig.json"
            | "jest.config.ts"
            | "jest.config.js"
            | "vite.config.ts"
            | "vite.config.js"
            | "next.config.js"
            | "next.config.mjs"
            | ".eslintrc"
            | ".eslintrc.js"
            | ".eslintrc.json"
            | ".prettierrc"
            | ".prettierrc.json"
            | ".env"
            | ".env.local"
            | ".env.production"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn detects_doc_files() {
        assert!(is_doc_file(Path::new("README.md")));
        assert!(is_doc_file(Path::new("docs/guide.mdx")));
        assert!(!is_doc_file(Path::new("src/main.ts")));
    }

    #[test]
    fn detects_package_files() {
        assert!(is_package_file(Path::new("package.json")));
        assert!(is_package_file(Path::new("package-lock.json")));
        assert!(is_package_file(Path::new("Cargo.toml")));
        assert!(is_package_file(Path::new("go.mod")));
        assert!(!is_package_file(Path::new("src/config.json")));
    }

    #[test]
    fn detects_lockfiles() {
        assert!(is_lockfile(Path::new("Cargo.lock")));
        assert!(is_lockfile(Path::new("yarn.lock")));
        assert!(!is_lockfile(Path::new("src/lib.rs")));
    }

    #[test]
    fn detects_test_files() {
        assert!(is_test_file(Path::new("session.test.ts")));
        assert!(is_test_file(Path::new("auth.spec.tsx")));
        assert!(!is_test_file(Path::new("session.ts")));
    }

    #[test]
    fn detects_config_files() {
        assert!(is_config_file(Path::new("tsconfig.json")));
        assert!(is_config_file(Path::new(".env")));
        assert!(is_config_file(Path::new(".env.local")));
        assert!(!is_config_file(Path::new("src/config.ts")));
    }

    #[test]
    fn tier1_rules_classify_correctly() {
        let rules = tier1_rules();
        let empty: Vec<StructuralChange> = vec![];

        let readme = PathBuf::from("README.md");
        let matched = rules.iter().find(|r| (r.matches)(&readme, &empty));
        assert_eq!(
            matched.map(|r| &r.pattern),
            Some(&ChangePattern::Documentation)
        );

        let pkg = PathBuf::from("package.json");
        let matched = rules.iter().find(|r| (r.matches)(&pkg, &empty));
        assert_eq!(
            matched.map(|r| &r.pattern),
            Some(&ChangePattern::DependencyUpdate)
        );

        let test = PathBuf::from("auth.test.ts");
        let matched = rules.iter().find(|r| (r.matches)(&test, &empty));
        assert_eq!(
            matched.map(|r| &r.pattern),
            Some(&ChangePattern::TestAddition)
        );
    }

    #[test]
    fn tier2_bugfix_rule_matches_evidence() {
        let rules = tier2_rules();
        let bugfix_rule = rules.iter().find(|r| r.name == "bugfix-evidence").unwrap();
        
        let evidence = vec![Evidence {
            kind: EvidenceKind::NullGuardAdded,
            description: "null guard".into(),
            location: crate::core::types::Location {
                file: PathBuf::from("src/auth.ts"),
                line: 42,
                column: 0,
            },
        }];
        assert!((bugfix_rule.matches)(&evidence));
        assert_eq!(bugfix_rule.pattern, ChangePattern::BugFix);
        assert_eq!(bugfix_rule.confidence, 0.8);
    }

    #[test]
    fn tier2_feature_rule_matches_evidence() {
        let rules = tier2_rules();
        let feature_rule = rules.iter().find(|r| r.name == "feature-evidence").unwrap();
        
        let evidence = vec![Evidence {
            kind: EvidenceKind::ExportAdded,
            description: "export function".into(),
            location: crate::core::types::Location {
                file: PathBuf::from("src/api.ts"),
                line: 10,
                column: 0,
            },
        }];
        assert!((feature_rule.matches)(&evidence));
        assert_eq!(feature_rule.pattern, ChangePattern::Feature);
    }

    #[test]
    fn tier2_security_rule_matches_evidence() {
        let rules = tier2_rules();
        let security_rule = rules.iter().find(|r| r.name == "security-evidence").unwrap();
        
        let evidence = vec![Evidence {
            kind: EvidenceKind::AuthCheckAdded,
            description: "auth check".into(),
            location: crate::core::types::Location {
                file: PathBuf::from("src/auth.ts"),
                line: 50,
                column: 0,
            },
        }];
        assert!((security_rule.matches)(&evidence));
        assert_eq!(security_rule.pattern, ChangePattern::Security);
        assert_eq!(security_rule.confidence, 0.9);
    }

    #[test]
    fn tier2_refactor_rule_matches_evidence() {
        let rules = tier2_rules();
        let refactor_rule = rules.iter().find(|r| r.name == "refactor-evidence").unwrap();
        
        let evidence = vec![Evidence {
            kind: EvidenceKind::SymbolRenamed,
            description: "renamed function".into(),
            location: crate::core::types::Location {
                file: PathBuf::from("src/utils.ts"),
                line: 25,
                column: 0,
            },
        }];
        assert!((refactor_rule.matches)(&evidence));
        assert_eq!(refactor_rule.pattern, ChangePattern::Refactor);
    }

    #[test]
    fn tier2_test_rule_matches_evidence() {
        let rules = tier2_rules();
        let test_rule = rules.iter().find(|r| r.name == "test-evidence").unwrap();
        
        let evidence = vec![Evidence {
            kind: EvidenceKind::TestCaseAdded,
            description: "new test case".into(),
            location: crate::core::types::Location {
                file: PathBuf::from("src/auth.test.ts"),
                line: 30,
                column: 0,
            },
        }];
        assert!((test_rule.matches)(&evidence));
        assert_eq!(test_rule.pattern, ChangePattern::TestAddition);
    }

    #[test]
    fn tier2_rules_no_match_without_evidence() {
        let rules = tier2_rules();
        let empty_evidence: Vec<Evidence> = vec![];
        
        for rule in &rules {
            assert!(!(rule.matches)(&empty_evidence), "Rule {} should not match empty evidence", rule.name);
        }
    }

    #[test]
    fn detects_ci_config_files() {
        assert!(is_ci_config(Path::new(".github/workflows/ci.yml")));
        assert!(is_ci_config(Path::new(".gitlab-ci.yml")));
        assert!(is_ci_config(Path::new(".circleci/config.yml")));
        assert!(is_ci_config(Path::new("Jenkinsfile")));
        assert!(!is_ci_config(Path::new("src/main.ts")));
    }

    #[test]
    fn detects_docker_files() {
        assert!(is_docker_file(Path::new("Dockerfile")));
        assert!(is_docker_file(Path::new("docker-compose.yml")));
        assert!(is_docker_file(Path::new("docker-compose.yaml")));
        assert!(is_docker_file(Path::new("app.dockerfile")));
        assert!(!is_docker_file(Path::new("src/main.ts")));
    }

    #[test]
    fn detects_migration_files() {
        assert!(is_migration_file(Path::new("migrations/001_create_users.sql")));
        assert!(is_migration_file(Path::new("src/migrate/add_index.ts")));
        assert!(!is_migration_file(Path::new("src/main.ts")));
    }

    #[test]
    fn tier1_ci_config_classifies_correctly() {
        let rules = tier1_rules();
        let empty: Vec<StructuralChange> = vec![];
        let ci = PathBuf::from(".github/workflows/ci.yml");
        let matched = rules.iter().find(|r| (r.matches)(&ci, &empty));
        assert_eq!(
            matched.map(|r| &r.pattern),
            Some(&ChangePattern::Refactor)
        );
    }

    #[test]
    fn tier1_docker_classifies_correctly() {
        let rules = tier1_rules();
        let empty: Vec<StructuralChange> = vec![];
        let docker = PathBuf::from("Dockerfile");
        let matched = rules.iter().find(|r| (r.matches)(&docker, &empty));
        assert_eq!(
            matched.map(|r| &r.pattern),
            Some(&ChangePattern::Refactor)
        );
    }

    #[test]
    fn tier1_migration_classifies_correctly() {
        let rules = tier1_rules();
        let empty: Vec<StructuralChange> = vec![];
        let migration = PathBuf::from("migrations/001_create_users.sql");
        let matched = rules.iter().find(|r| (r.matches)(&migration, &empty));
        assert_eq!(
            matched.map(|r| &r.pattern),
            Some(&ChangePattern::Feature)
        );
    }
}
