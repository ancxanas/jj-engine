use crate::core::types::{Evidence, EvidenceKind, StructuralChange, StructuralChangeKind};

#[must_use]
pub fn detect(changes: &[StructuralChange]) -> Vec<Evidence> {
    changes
        .iter()
        .filter_map(|c| match c.kind {
            StructuralChangeKind::ExportAdded => Some(Evidence {
                kind: EvidenceKind::ExportAdded,
                description: format!("new export: {}", c.name),
                location: c.location.clone(),
            }),
            StructuralChangeKind::FunctionAdded => Some(Evidence {
                kind: EvidenceKind::NewPublicApi,
                description: format!("new function: {}", c.name),
                location: c.location.clone(),
            }),
            StructuralChangeKind::ClassAdded => Some(Evidence {
                kind: EvidenceKind::NewPublicApi,
                description: format!("new class: {}", c.name),
                location: c.location.clone(),
            }),
            _ => None,
        })
        .collect()
}
