use crate::core::types::{Evidence, EvidenceKind, StructuralChange, StructuralChangeKind};

#[must_use]
pub fn detect(changes: &[StructuralChange]) -> Vec<Evidence> {
    changes
        .iter()
        .filter_map(|c| match c.kind {
            StructuralChangeKind::NullCheckAdded => Some(Evidence {
                kind: EvidenceKind::NullGuardAdded,
                description: format!("null guard added in {}", c.name),
                location: c.location.clone(),
            }),
            StructuralChangeKind::IfStatementAdded => Some(Evidence {
                kind: EvidenceKind::NullGuardAdded,
                description: format!("conditional guard added in {}", c.name),
                location: c.location.clone(),
            }),
            StructuralChangeKind::TryCatchAdded => Some(Evidence {
                kind: EvidenceKind::ErrorHandlerAdded,
                description: format!("error handler added in {}", c.name),
                location: c.location.clone(),
            }),
            StructuralChangeKind::OptionalChainAdded => Some(Evidence {
                kind: EvidenceKind::OptionalChainingAdded,
                description: format!("optional chaining added in {}", c.name),
                location: c.location.clone(),
            }),
            _ => None,
        })
        .collect()
}
