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

    if base != impl_stem {
        return false;
    }

    // Same directory (e.g., src/auth.test.ts → src/auth.ts)
    if test_path.parent() == impl_path.parent() {
        return true;
    }

    // Cross-directory: mirror the relative structure (e.g., tests/features/X.test.ts → src/features/X.ts)
    // Strip the "tests" segment from the test path, then strip "src" from the impl path,
    // and compare the directory portions (filenames already matched via stem check above).
    let test_str = test_path.to_string_lossy();
    let impl_str = impl_path.to_string_lossy();

    let test_after = test_str
        .strip_prefix("tests/")
        .or_else(|| test_str.strip_prefix("tests\\"));
    let impl_after = impl_str
        .strip_prefix("src/")
        .or_else(|| impl_str.strip_prefix("src\\"))
        .or_else(|| impl_str.strip_prefix("lib/"))
        .or_else(|| impl_str.strip_prefix("lib\\"));

    if let (Some(t), Some(i)) = (test_after, impl_after) {
        let test_dir = Path::new(t).parent().unwrap_or_else(|| Path::new(""));
        let impl_dir = Path::new(i).parent().unwrap_or_else(|| Path::new(""));
        if test_dir == impl_dir {
            return true;
        }
    }

    false
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
    fn detects_cross_directory_test_of() {
        assert!(is_test_of(
            Path::new("tests/features/new-feature.test.ts"),
            Path::new("src/features/new-feature.ts"),
        ));
        assert!(is_test_of(
            Path::new("tests/auth.test.ts"),
            Path::new("src/auth.ts"),
        ));
    }

    #[test]
    fn cross_directory_requires_tests_prefix() {
        assert!(!is_test_of(
            Path::new("lib/auth.test.ts"),
            Path::new("src/auth.ts"),
        ));
    }
}
