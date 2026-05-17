pub mod bugfix;
pub mod dependency;
pub mod docs;
pub mod feature;
pub mod refactor;
pub mod security;
pub mod test;

use crate::core::types::{Evidence, StructuralChange};

#[must_use]
pub fn detect_all(changes: &[StructuralChange]) -> Vec<Evidence> {
    let mut evidence = Vec::new();
    evidence.extend(bugfix::detect(changes));
    evidence.extend(refactor::detect(changes));
    evidence.extend(feature::detect(changes));
    evidence.extend(security::detect(changes));
    evidence.extend(test::detect(changes));
    evidence.extend(dependency::detect(changes));
    evidence.extend(docs::detect(changes));
    evidence
}
