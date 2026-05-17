use crate::core::types::ChangePattern;

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
}
