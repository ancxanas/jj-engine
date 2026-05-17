use petgraph::stable_graph::NodeIndex;
use petgraph::visit::EdgeRef;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;

use super::graph::RelationshipGraph;
use crate::core::types::{ChangePattern, FileRelationship};

pub struct IntentGroup {
    pub files: Vec<PathBuf>,
    pub pattern: ChangePattern,
    pub relationships: Vec<FileRelationship>,
    pub reason: String,
}

#[must_use]
pub fn partition(graph: &RelationshipGraph) -> Vec<IntentGroup> {
    if graph.node_count() == 0 {
        return vec![];
    }

    let mut seen = HashSet::new();
    let mut groups = Vec::new();

    for idx in graph.graph.node_indices() {
        if seen.contains(&idx) {
            continue;
        }

        let component = collect_component(graph, idx, &mut seen);
        let mut files: Vec<_> = component
            .iter()
            .map(|&node_idx| graph.graph[node_idx].path.clone())
            .collect();
        files.sort();

        let pattern = dominant_pattern(
            component
                .iter()
                .map(|&node_idx| &graph.graph[node_idx].pattern),
        );
        let relationships = collect_relationships(graph, &component);
        let reason = if relationships.is_empty() {
            String::from("isolated file change")
        } else {
            relationships
                .iter()
                .map(|relationship| format!("{:?}", relationship.kind))
                .collect::<Vec<_>>()
                .join(" + ")
        };

        groups.push(IntentGroup {
            files,
            pattern,
            relationships,
            reason,
        });
    }

    groups.sort_by(|left, right| left.files.cmp(&right.files));
    groups
}

fn collect_component(
    graph: &RelationshipGraph,
    start: NodeIndex,
    seen: &mut HashSet<NodeIndex>,
) -> Vec<NodeIndex> {
    let mut queue = VecDeque::from([start]);
    let mut component = Vec::new();
    seen.insert(start);

    while let Some(node_idx) = queue.pop_front() {
        component.push(node_idx);
        for neighbor in graph.graph.neighbors_undirected(node_idx) {
            if seen.insert(neighbor) {
                queue.push_back(neighbor);
            }
        }
    }

    component
}

fn dominant_pattern<'a>(patterns: impl Iterator<Item = &'a ChangePattern>) -> ChangePattern {
    let patterns: Vec<_> = patterns.cloned().collect();
    let mut counts = HashMap::new();
    for pattern in &patterns {
        *counts.entry(pattern.clone()).or_insert(0usize) += 1;
    }

    let mut best_pattern = ChangePattern::Unknown;
    let mut best_count = 0usize;
    for pattern in patterns {
        let count = counts.get(&pattern).copied().unwrap_or(0);
        if count > best_count {
            best_pattern = pattern;
            best_count = count;
        }
    }

    best_pattern
}

fn collect_relationships(
    graph: &RelationshipGraph,
    component: &[NodeIndex],
) -> Vec<FileRelationship> {
    let component_set: HashSet<_> = component.iter().copied().collect();
    let mut relationships = Vec::new();

    for &node_idx in component {
        for edge in graph.graph.edges(node_idx) {
            if !component_set.contains(&edge.target()) {
                continue;
            }

            let from = &graph.graph[edge.source()].path;
            let to = &graph.graph[edge.target()].path;
            let weight = edge.weight();
            relationships.push(FileRelationship {
                from: from.clone(),
                to: to.clone(),
                kind: weight.kind.clone(),
                strength: weight.strength.clone(),
            });
        }
    }

    relationships.sort_by(|left, right| {
        (&left.from, &left.to, &left.kind, &left.strength).cmp(&(
            &right.from,
            &right.to,
            &right.kind,
            &right.strength,
        ))
    });
    relationships.dedup_by(|left, right| {
        left.from == right.from
            && left.to == right.to
            && left.kind == right.kind
            && left.strength == right.strength
    });
    relationships
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent::cluster::edges;
    use crate::intent::cluster::graph::RelationshipGraph;

    #[test]
    fn clusters_test_with_implementation() -> anyhow::Result<()> {
        let mut graph = RelationshipGraph::new();
        graph.add_file("src/auth.ts".into(), ChangePattern::BugFix);
        graph.add_file("src/auth.test.ts".into(), ChangePattern::TestAddition);
        graph.add_file("src/db.ts".into(), ChangePattern::Refactor);
        edges::detect_all_edges(&mut graph);
        let groups = partition(&graph);

        let auth_group = groups
            .iter()
            .find(|group| group.files.contains(&PathBuf::from("src/auth.ts")))
            .ok_or_else(|| anyhow::anyhow!("missing auth group"))?;
        assert!(auth_group
            .files
            .contains(&PathBuf::from("src/auth.test.ts")));
        assert!(!auth_group.files.contains(&PathBuf::from("src/db.ts")));
        Ok(())
    }

    #[test]
    fn isolated_files_get_own_group() {
        let mut graph = RelationshipGraph::new();
        graph.add_file("README.md".into(), ChangePattern::Documentation);
        graph.add_file("src/db.ts".into(), ChangePattern::Refactor);
        edges::detect_all_edges(&mut graph);
        let groups = partition(&graph);

        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn empty_graph_returns_no_groups() {
        let graph = RelationshipGraph::new();
        let groups = partition(&graph);
        assert!(groups.is_empty());
    }
}
