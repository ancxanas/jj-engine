use std::path::Path;

use super::graph::RelationshipGraph;
use crate::core::types::{RelationshipKind, RelationshipStrength};

pub fn detect_all_edges(graph: &mut RelationshipGraph) {
    let paths: Vec<_> = graph.index_map.keys().cloned().collect();
    for i in 0..paths.len() {
        for j in (i + 1)..paths.len() {
            detect_pair(graph, &paths[i], &paths[j]);
        }
    }
}

fn detect_pair(graph: &mut RelationshipGraph, a: &Path, b: &Path) {
    if is_test_of(a, b) {
        graph.add_edge(a, b, RelationshipKind::TestOf, RelationshipStrength::Strong);
    } else if is_test_of(b, a) {
        graph.add_edge(b, a, RelationshipKind::TestOf, RelationshipStrength::Strong);
    }

    if is_directory_peer(a, b) {
        graph.add_edge(
            a,
            b,
            RelationshipKind::DirectoryPeer,
            RelationshipStrength::Weak,
        );
        graph.add_edge(
            b,
            a,
            RelationshipKind::DirectoryPeer,
            RelationshipStrength::Weak,
        );
    }

    if has_shared_pattern(graph, a, b) {
        graph.add_edge(
            a,
            b,
            RelationshipKind::SharedPattern,
            RelationshipStrength::Weak,
        );
        graph.add_edge(
            b,
            a,
            RelationshipKind::SharedPattern,
            RelationshipStrength::Weak,
        );
    }
}

fn is_test_of(test_path: &Path, impl_path: &Path) -> bool {
    let test_stem = test_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("");
    let impl_stem = impl_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("");

    if test_stem.is_empty() || impl_stem.is_empty() {
        return false;
    }

    let base = test_stem
        .strip_suffix(".test")
        .or_else(|| test_stem.strip_suffix(".spec"))
        .or_else(|| test_stem.strip_suffix("_test"))
        .or_else(|| test_stem.strip_suffix("_spec"));

    let Some(base) = base else {
        return false;
    };

    base == impl_stem && test_path.parent() == impl_path.parent()
}

fn is_directory_peer(a: &Path, b: &Path) -> bool {
    match (a.parent(), b.parent()) {
        (Some(left), Some(right)) => left == right && left.components().count() > 1,
        _ => false,
    }
}

fn has_shared_pattern(graph: &RelationshipGraph, a: &Path, b: &Path) -> bool {
    let Some(&left_idx) = graph.index_map.get(a) else {
        return false;
    };
    let Some(&right_idx) = graph.index_map.get(b) else {
        return false;
    };
    graph.graph[left_idx].pattern == graph.graph[right_idx].pattern
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_test_of_relationship() {
        assert!(is_test_of(
            Path::new("src/auth.test.ts"),
            Path::new("src/auth.ts"),
        ));
        assert!(is_test_of(
            Path::new("src/auth.spec.ts"),
            Path::new("src/auth.ts"),
        ));
        assert!(!is_test_of(
            Path::new("src/auth.ts"),
            Path::new("src/auth.test.ts"),
        ));
    }

    #[test]
    fn detects_directory_peer() {
        assert!(is_directory_peer(
            Path::new("src/auth/login.ts"),
            Path::new("src/auth/session.ts"),
        ));
        assert!(!is_directory_peer(
            Path::new("src/auth/login.ts"),
            Path::new("src/db/client.ts"),
        ));
    }

    #[test]
    fn test_of_requires_same_directory() {
        assert!(!is_test_of(
            Path::new("tests/auth.test.ts"),
            Path::new("src/auth.ts"),
        ));
    }
}
