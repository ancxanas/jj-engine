//! Work unit inference and change grouping.
//! Builds a dependency graph from semantic data and finds connected components.
//! Classifies each component into a meaningful work unit.

use std::collections::HashMap;
use std::collections::HashSet;

use crate::semantic::EntityKind;
use crate::semantic::EntityPath;
use crate::semantic::SemanticChange;
use crate::semantic::SemanticChangeType;
use crate::semantic::SemanticDiff;
use crate::semantic::SemanticSnapshot;

/// The kind of work a unit represents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkKind {
    /// New public capability added.
    Feature,

    /// Existing behavior corrected. No structural changes.
    BugFix,

    /// Code reorganized. Includes renames, signature changes,
    /// private additions, dependency changes, removals.
    Refactor,

    /// Test code added or modified.
    Test,
}

/// A logical unit of work inferred from a set of related changes.
#[derive(Debug, Clone)]
pub struct WorkUnit {
    /// Unique id within this planning cycle.
    pub id: usize,

    /// What kind of work this represents.
    pub kind: WorkKind,

    /// The entities involved in this work unit.
    pub entities: Vec<EntityPath>,

    /// The semantic changes that make up this work unit.
    pub changes: Vec<SemanticChange>,

    /// If this is a Test unit, links to the id of the
    /// implementation unit it verifies.
    pub related_to: Option<usize>,
}

/// A graph of dependencies between code entities.
#[derive(Debug, Clone)]
pub struct DependencyGraph {
    /// Forward edges: entity → entities it depends on.
    pub forward: HashMap<EntityPath, Vec<EntityPath>>,

    /// Reverse edges: entity → entities that depend on it.
    pub reverse: HashMap<EntityPath, Vec<EntityPath>>,
}

impl DependencyGraph {
    /// Creates an empty graph.
    pub fn empty() -> Self {
        Self {
            forward: HashMap::new(),
            reverse: HashMap::new(),
        }
    }

    /// Adds a directed edge from `from` to `to`.
    fn add_edge(&mut self, from: EntityPath, to: EntityPath) {
        self.forward
            .entry(from.clone())
            .or_default()
            .push(to.clone());
        self.reverse.entry(to).or_default().push(from);
    }

    /// Returns all entities this entity depends on.
    pub fn dependencies_of(&self, entity: &EntityPath) -> &[EntityPath] {
        match self.forward.get(entity) {
            Some(deps) => deps,
            None => &[],
        }
    }

    /// Returns all entities that depend on this entity.
    pub fn dependents_of(&self, entity: &EntityPath) -> &[EntityPath] {
        match self.reverse.get(entity) {
            Some(deps) => deps,
            None => &[],
        }
    }

    /// Returns total number of edges.
    pub fn edge_count(&self) -> usize {
        self.forward.values().map(|v| v.len()).sum()
    }
}

/// Builds a dependency graph from a semantic snapshot.
///
/// For each entity, resolves its `calls` and `uses_types` against
/// all entities in the snapshot. An edge is created only if:
/// - the target entity is in the same file, OR
/// - the caller's file has an import matching the target's file
///
/// This prevents false connections between unrelated entities
/// with the same name in different modules.pub
pub fn build_graph(snapshot: &SemanticSnapshot) -> DependencyGraph {
    let mut graph = DependencyGraph::empty();

    // Build name lookup: name → list of EntityPaths with that name
    let mut name_lookup: HashMap<&str, Vec<&EntityPath>> = HashMap::new();
    for (path, _entity) in &snapshot.entities {
        name_lookup.entry(&path.name).or_default().push(path);
    }

    // Build file → imports lookup
    let mut file_imports: HashMap<&std::path::PathBuf, Vec<&String>> = HashMap::new();
    for (path, entity) in &snapshot.entities {
        if path.kind == crate::semantic::EntityKind::Import {
            for imp in &entity.imports {
                file_imports.entry(&path.file).or_default().push(imp);
            }
        }
    }

    // For each entity resolve calls and type references into edges
    for (path, entity) in &snapshot.entities {
        let file_imp_list = file_imports
            .get(&path.file)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);

        // Resolve both calls and type references with same logic
        for name in entity.calls.iter().chain(entity.uses_types.iter()) {
            if let Some(targets) = name_lookup.get(name.as_str()) {
                for target in targets {
                    if *target == path {
                        continue;
                    }

                    let is_same_file = target.file == path.file;

                    let is_imported = file_imp_list.iter().any(|imp| {
                        let target_file_stem = target
                            .file
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("");
                        imp.contains(target_file_stem)
                    });

                    // Only connect if same file or explicitly imported
                    if is_same_file || is_imported {
                        graph.add_edge(path.clone(), (*target).clone());
                    }
                }
            }
        }
    }

    for edges in graph.forward.values_mut() {
        let mut seen = HashSet::new();
        edges.retain(|e| seen.insert(e.clone()));
    }

    for edges in graph.reverse.values_mut() {
        let mut seen = HashSet::new();
        edges.retain(|e| seen.insert(e.clone()));
    }

    graph
}

/// Finds connected components among the given changed entities.
pub fn find_connected_components(
    changed: &[EntityPath],
    graph: &DependencyGraph,
) -> Vec<Vec<EntityPath>> {
    if changed.is_empty() {
        return Vec::new();
    }

    let changed_set: HashSet<&EntityPath> = changed.iter().collect();
    let mut visited: HashSet<&EntityPath> = HashSet::new();
    let mut components: Vec<Vec<EntityPath>> = Vec::new();

    for entity in changed {
        if visited.contains(entity) {
            continue;
        }

        let mut component: Vec<EntityPath> = Vec::new();
        let mut queue: Vec<&EntityPath> = vec![entity];
        let mut seen: HashSet<&EntityPath> = HashSet::new();
        seen.insert(entity);

        while let Some(current) = queue.pop() {
            if changed_set.contains(current) {
                if !visited.contains(current) {
                    component.push(current.clone());
                    visited.insert(current);
                }
            }

            for dep in graph.dependencies_of(current) {
                if !seen.contains(dep) {
                    seen.insert(dep);
                    queue.push(dep);
                }
            }

            for dep in graph.dependents_of(current) {
                if !seen.contains(dep) {
                    seen.insert(dep);
                    queue.push(dep);
                }
            }
        }

        if !component.is_empty() {
            components.push(component);
        }
    }

    components
}

/// Classifies connected components into work units.
///
/// Each component is split into implementation and test groups.
/// Each group becomes its own WorkUnit.
/// Test units link back to their implementation unit via `related_to`.
pub fn classify_work_units(
    components: Vec<Vec<EntityPath>>,
    diff: &SemanticDiff,
    snapshot: &SemanticSnapshot,
) -> Vec<WorkUnit> {
    let mut units: Vec<WorkUnit> = Vec::new();
    let mut next_id: usize = 0;

    // Build lookup: EntityPath → SemanticChange
    let change_lookup: HashMap<&EntityPath, &SemanticChange> =
        diff.changes.iter().map(|c| (&c.entity, c)).collect();

    for component in components {
        // Split into implementation and test entities
        let mut impl_entities: Vec<EntityPath> = Vec::new();
        let mut test_entities: Vec<EntityPath> = Vec::new();

        for entity in &component {
            let is_test = snapshot
                .entities
                .get(entity)
                .map(|e| e.is_test)
                .unwrap_or(false);

            if is_test || entity.kind == EntityKind::Test {
                test_entities.push(entity.clone());
            } else {
                impl_entities.push(entity.clone());
            }
        }

        // Classify and create implementation work unit
        let impl_unit_id = if !impl_entities.is_empty() {
            let impl_changes: Vec<SemanticChange> = impl_entities
                .iter()
                .filter_map(|e| change_lookup.get(e).map(|c| (*c).clone()))
                .collect();

            let kind = classify_impl_group(&impl_changes);

            let id = next_id;
            next_id += 1;

            units.push(WorkUnit {
                id,
                kind,
                entities: impl_entities,
                changes: impl_changes,
                related_to: None,
            });

            Some(id)
        } else {
            None
        };

        // Create test work unit if test entities exist
        if !test_entities.is_empty() {
            let test_changes: Vec<SemanticChange> = test_entities
                .iter()
                .filter_map(|e| change_lookup.get(e).map(|c| (*c).clone()))
                .collect();

            let id = next_id;
            next_id += 1;

            units.push(WorkUnit {
                id,
                kind: WorkKind::Test,
                entities: test_entities,
                changes: test_changes,
                related_to: impl_unit_id,
            });
        }
    }

    units
}

/// Classifies an implementation group into Feature, BugFix, or Refactor.
///
/// Priority: Feature > BugFix > Refactor
fn classify_impl_group(changes: &[SemanticChange]) -> WorkKind {
    // Check for public additions → Feature
    let has_public_addition = changes
        .iter()
        .any(|c| c.change_type == SemanticChangeType::Added && c.is_public);

    if has_public_addition {
        return WorkKind::Feature;
    }

    // Check for only implementation changes → BugFix
    let all_impl_changed = !changes.is_empty()
        && changes
            .iter()
            .all(|c| c.change_type == SemanticChangeType::ImplementationChanged);

    if all_impl_changed {
        return WorkKind::BugFix;
    }

    // Everything else → Refactor
    // Covers:
    // - private additions only
    // - signature changes
    // - removals
    // - mixed changes without new public surface
    // - import/dependency changes
    WorkKind::Refactor
}

/// Detects features that have no corresponding test unit.
/// Returns ids of work units that are Features without a linked Test.
pub fn find_untested_features(units: &[WorkUnit]) -> Vec<usize> {
    let tested_impl_ids: HashSet<usize> = units
        .iter()
        .filter(|u| u.kind == WorkKind::Test)
        .filter_map(|u| u.related_to)
        .collect();

    units
        .iter()
        .filter(|u| u.kind == WorkKind::Feature)
        .filter(|u| !tested_impl_ids.contains(&u.id))
        .map(|u| u.id)
        .collect()
}
