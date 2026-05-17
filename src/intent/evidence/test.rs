use crate::core::types::{Evidence, EvidenceKind, StructuralChange, StructuralChangeKind};

#[must_use]
pub fn detect(changes: &[StructuralChange]) -> Vec<Evidence> {
    changes
        .iter()
        .filter_map(|c| match c.kind {
            StructuralChangeKind::TestCaseAdded => Some(Evidence {
                kind: EvidenceKind::TestCaseAdded,
                description: format!("test case added: {}", c.name),
                location: c.location.clone(),
            }),
            StructuralChangeKind::TestCaseModified => Some(Evidence {
                kind: EvidenceKind::TestCaseModified,
                description: format!("test case modified: {}", c.name),
                location: c.location.clone(),
            }),
            StructuralChangeKind::DescribeBlockAdded => Some(Evidence {
                kind: EvidenceKind::TestCaseAdded,
                description: format!("test suite added: {}", c.name),
                location: c.location.clone(),
            }),
            _ => None,
        })
        .collect()
}
