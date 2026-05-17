use petgraph::stable_graph::{NodeIndex, StableGraph};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::core::types::{ChangePattern, RelationshipKind, RelationshipStrength};

pub struct FileNode {
    pub path: PathBuf,
    pub pattern: ChangePattern,
}

pub struct Edge {
    pub kind: RelationshipKind,
    pub strength: RelationshipStrength,
}

pub struct RelationshipGraph {
    pub graph: StableGraph<FileNode, Edge>,
    pub index_map: HashMap<PathBuf, NodeIndex>,
}

impl Default for RelationshipGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl RelationshipGraph {
    #[must_use]
    pub fn new() -> Self {
        Self {
            graph: StableGraph::new(),
            index_map: HashMap::new(),
        }
    }

    pub fn add_file(&mut self, path: PathBuf, pattern: ChangePattern) -> NodeIndex {
        if let Some(&idx) = self.index_map.get(&path) {
            return idx;
        }
        let idx = self.graph.add_node(FileNode {
            path: path.clone(),
            pattern,
        });
        self.index_map.insert(path, idx);
        idx
    }

    pub fn add_edge(
        &mut self,
        from: &Path,
        to: &Path,
        kind: RelationshipKind,
        strength: RelationshipStrength,
    ) {
        let Some(&from_idx) = self.index_map.get(from) else {
            return;
        };
        let Some(&to_idx) = self.index_map.get(to) else {
            return;
        };
        self.graph
            .add_edge(from_idx, to_idx, Edge { kind, strength });
    }

    #[must_use]
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_nodes_and_edges() {
        let mut graph = RelationshipGraph::new();
        graph.add_file("src/auth.ts".into(), ChangePattern::BugFix);
        graph.add_file("src/auth.test.ts".into(), ChangePattern::TestAddition);
        graph.add_edge(
            Path::new("src/auth.ts"),
            Path::new("src/auth.test.ts"),
            RelationshipKind::TestOf,
            RelationshipStrength::Strong,
        );
        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.graph.edge_count(), 1);
    }

    #[test]
    fn deduplicates_nodes() {
        let mut graph = RelationshipGraph::new();
        graph.add_file("a.ts".into(), ChangePattern::BugFix);
        graph.add_file("a.ts".into(), ChangePattern::BugFix);
        assert_eq!(graph.node_count(), 1);
    }
}
