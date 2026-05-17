use super::rules;
use crate::core::types::{ChangePattern, PolicyDecision};

#[must_use]
pub fn evaluate(pattern: &ChangePattern) -> PolicyDecision {
    if rules::is_blocked(pattern) {
        return PolicyDecision::Blocked {
            reason: format!("{pattern:?} changes always require manual review"),
        };
    }

    if rules::is_auto_committable(pattern) {
        return PolicyDecision::AutoCommittable;
    }

    PolicyDecision::RequiresReview
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documentation_is_auto_committable() {
        assert_eq!(
            evaluate(&ChangePattern::Documentation),
            PolicyDecision::AutoCommittable
        );
    }

    #[test]
    fn bugfix_requires_review() {
        assert_eq!(
            evaluate(&ChangePattern::BugFix),
            PolicyDecision::RequiresReview
        );
    }

    #[test]
    fn security_is_blocked() {
        assert!(matches!(
            evaluate(&ChangePattern::Security),
            PolicyDecision::Blocked { .. }
        ));
    }

    #[test]
    fn unknown_requires_review() {
        assert_eq!(
            evaluate(&ChangePattern::Unknown),
            PolicyDecision::RequiresReview
        );
    }
}
