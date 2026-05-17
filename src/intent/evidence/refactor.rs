use crate::core::types::{Evidence, EvidenceKind, StructuralChange, StructuralChangeKind};

#[must_use]
pub fn detect(changes: &[StructuralChange]) -> Vec<Evidence> {
    changes
        .iter()
        .filter_map(|c| match c.kind {
            StructuralChangeKind::FunctionRenamed => Some(Evidence {
                kind: EvidenceKind::SymbolRenamed,
                description: format!("function renamed: {}", c.name),
                location: c.location.clone(),
            }),
            _ => None,
        })
        .collect()
}
