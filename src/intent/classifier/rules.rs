use std::path::Path;

use crate::core::types::{ChangePattern, StructuralChange};

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
            name: "config-change",
            tier: RuleTier::Decisive,
            pattern: ChangePattern::Configuration,
            matches: |path, _| is_config_file(path),
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
        Some("package.json" | "package-lock.json")
    )
}

fn is_lockfile(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|f| f.to_str()),
        Some("package-lock.json" | "yarn.lock" | "pnpm-lock.yaml" | "bun.lockb")
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
        assert!(!is_package_file(Path::new("src/config.json")));
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
}
