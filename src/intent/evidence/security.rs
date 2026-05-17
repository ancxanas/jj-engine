use crate::core::types::{Evidence, StructuralChange};

#[must_use]
pub const fn detect(_changes: &[StructuralChange]) -> Vec<Evidence> {
    Vec::new()
}
