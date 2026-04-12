//! Semantic/AST project understanding layer.
//! Uses tree-sitter to parse source files and extract code entities.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Result;
use walkdir::WalkDir;

/// The kind of code entity.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EntityKind {
    Function,
    Struct,
    Enum,
    Trait,
    Impl,
    Module,
    Import,
    Constant,
    Test,
}

/// Uniquely identifies a code entity within a project.
/// Used as the key in SemanticSnapshot.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntityPath {
    /// The file this entity lives in.
    pub file: PathBuf,

    /// The name of the entity.
    pub name: String,

    /// The kind of entity.
    pub kind: EntityKind,
}

/// A single code entity extracted from a source file.
#[derive(Debug, Clone)]
pub struct EntityInfo {
    /// Unique path identifying this entity.
    pub path: EntityPath,

    /// Whether this entity is public.
    pub is_public: bool,

    /// Whether this is a test function.
    pub is_test: bool,

    /// Start line in the source file (zero-indexed).
    pub line_start: usize,

    /// End line in the source file (zero-indexed).
    pub line_end: usize,

    /// Hash of the entity signature (name + params + return type).
    /// Used to detect signature changes.
    pub signature_hash: u64,

    /// Hash of the full entity body.
    /// Used to detect implementation changes.
    pub body_hash: u64,
}

/// A snapshot of all code entities in a project at a point in time.
#[derive(Debug, Clone)]
pub struct SemanticSnapshot {
    /// All entities found, keyed by their unique path.
    pub entities: HashMap<EntityPath, EntityInfo>,

    /// How many files were scanned.
    pub files_scanned: usize,
}

impl SemanticSnapshot {
    /// Creates an empty snapshot.
    pub fn empty() -> Self {
        Self {
            entities: HashMap::new(),
            files_scanned: 0,
        }
    }
}

/// Scans an entire project and builds a SemanticSnapshot.
/// Only scans Rust files for now.
pub fn snapshot_project(root: &Path) -> Result<SemanticSnapshot> {
    let root = root.canonicalize()?;
    let root = root.as_path();

    let mut entities: HashMap<EntityPath, EntityInfo> = HashMap::new();
    let mut files_scanned = 0;

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            // Skip target/ and .jj/ directories entirely
            let name = e.file_name().to_str().unwrap_or("");
            name != "target" && name != ".jj" && name != ".git"
        })
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Skip directories
        if path.is_dir() {
            continue;
        }

        // Only process Rust files
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }

        // Read file source
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        files_scanned += 1;

        // Parse file and collect entities
        let file_entities = parse_file(path, &source)?;

        for entity in file_entities {
            entities.insert(entity.path.clone(), entity);
        }
    }

    Ok(SemanticSnapshot {
        entities,
        files_scanned,
    })
}

/// Parses a Rust source file and extracts code entities.
pub fn parse_file(path: &Path, source: &str) -> Result<Vec<EntityInfo>> {
    let mut parser = tree_sitter::Parser::new();

    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .map_err(|e| anyhow::anyhow!("failed to set language: {}", e))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("failed to parse file: {}", path.display()))?;

    let root = tree.root_node();
    let mut entities = Vec::new();
    let mut cursor = root.walk();

    for child in root.named_children(&mut cursor) {
        match child.kind() {
            "function_item" => {
                if let Some(entity) = extract_function(path, child, source, false) {
                    entities.push(entity);
                }
            }
            "struct_item" => {
                if let Some(entity) = extract_named_item(path, child, source, EntityKind::Struct) {
                    entities.push(entity);
                }
            }
            "enum_item" => {
                if let Some(entity) = extract_named_item(path, child, source, EntityKind::Enum) {
                    entities.push(entity);
                }
            }
            "trait_item" => {
                if let Some(entity) = extract_named_item(path, child, source, EntityKind::Trait) {
                    entities.push(entity);
                }
            }
            "impl_item" => {
                if let Some(entity) = extract_impl(path, child, source) {
                    entities.push(entity);
                }
            }
            "use_declaration" => {
                if let Some(entity) = extract_import(path, child, source) {
                    entities.push(entity);
                }
            }
            "const_item" | "static_item" => {
                if let Some(entity) = extract_named_item(path, child, source, EntityKind::Constant)
                {
                    entities.push(entity);
                }
            }
            "mod_item" => {
                if let Some(entity) = extract_named_item(path, child, source, EntityKind::Module) {
                    entities.push(entity);
                }
            }
            _ => {}
        }
    }

    Ok(entities)
}

/// Extracts a function entity.
fn extract_function(
    file: &Path,
    node: tree_sitter::Node,
    source: &str,
    is_test: bool,
) -> Option<EntityInfo> {
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(name_node, source).to_string();
    let is_public = node_text(node, source).starts_with("pub");

    let body_node = node.child_by_field_name("body");
    let signature_text = if let Some(body) = body_node {
        &source[node.start_byte()..body.start_byte()]
    } else {
        node_text(node, source)
    };

    let body_text = if let Some(body) = body_node {
        node_text(body, source)
    } else {
        ""
    };

    let kind = if is_test {
        EntityKind::Test
    } else {
        EntityKind::Function
    };

    let path = EntityPath {
        file: file.to_path_buf(),
        name: name.clone(),
        kind: kind.clone(),
    };

    Some(EntityInfo {
        path,
        is_public,
        is_test,
        line_start: node.start_position().row,
        line_end: node.end_position().row,
        signature_hash: hash_str(signature_text),
        body_hash: hash_str(body_text),
    })
}

/// Extracts a named item (struct, enum, trait, constant, module).
fn extract_named_item(
    file: &Path,
    node: tree_sitter::Node,
    source: &str,
    kind: EntityKind,
) -> Option<EntityInfo> {
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(name_node, source).to_string();
    let is_public = node_text(node, source).starts_with("pub");
    let full_text = node_text(node, source);

    let path = EntityPath {
        file: file.to_path_buf(),
        name: name.clone(),
        kind: kind.clone(),
    };

    Some(EntityInfo {
        path,
        is_public,
        is_test: false,
        line_start: node.start_position().row,
        line_end: node.end_position().row,
        signature_hash: hash_str(full_text),
        body_hash: hash_str(full_text),
    })
}

/// Extracts an impl block.
fn extract_impl(file: &Path, node: tree_sitter::Node, source: &str) -> Option<EntityInfo> {
    let type_node = node.child_by_field_name("type")?;
    let name = format!("impl {}", node_text(type_node, source));
    let full_text = node_text(node, source);

    let path = EntityPath {
        file: file.to_path_buf(),
        name: name.clone(),
        kind: EntityKind::Impl,
    };

    Some(EntityInfo {
        path,
        is_public: false,
        is_test: false,
        line_start: node.start_position().row,
        line_end: node.end_position().row,
        signature_hash: hash_str(&name),
        body_hash: hash_str(full_text),
    })
}

/// Extracts a use/import declaration.
fn extract_import(file: &Path, node: tree_sitter::Node, source: &str) -> Option<EntityInfo> {
    let full_text = node_text(node, source);
    let hash = hash_str(full_text);

    let path = EntityPath {
        file: file.to_path_buf(),
        name: full_text.to_string(),
        kind: EntityKind::Import,
    };

    Some(EntityInfo {
        path,
        is_public: full_text.starts_with("pub"),
        is_test: false,
        line_start: node.start_position().row,
        line_end: node.end_position().row,
        signature_hash: hash,
        body_hash: hash,
    })
}

/// Gets the source text of a node.
fn node_text<'a>(node: tree_sitter::Node, source: &'a str) -> &'a str {
    &source[node.start_byte()..node.end_byte()]
}

/// Hashes a string to a u64.
fn hash_str(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

/// The type of change that occurred to an entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticChangeType {
    /// Entity was added (exists in new, not in old).
    Added,
    /// Entity was removed (exists in old, not in new).
    Removed,
    /// Entity signature changed (name, params, return type, fields).
    SignatureChanged,
    /// Entity implementation changed (body only, signature same).
    ImplementationChanged,
}

/// A single semantic change to one entity.
#[derive(Debug, Clone)]
pub struct SemanticChange {
    /// The entity that changed.
    pub entity: EntityPath,

    /// What kind of change occurred.
    pub change_type: SemanticChangeType,

    /// Whether this entity is public (for breaking change detection).
    pub is_public: bool,
}

/// The full semantic diff between two snapshots.
#[derive(Debug, Clone)]
pub struct SemanticDiff {
    /// All changes detected.
    pub changes: Vec<SemanticChange>,
}

impl SemanticDiff {
    /// Returns true if no changes were detected.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Returns all added entities.
    pub fn added(&self) -> Vec<&SemanticChange> {
        self.changes
            .iter()
            .filter(|c| c.change_type == SemanticChangeType::Added)
            .collect()
    }

    /// Returns all removed entities.
    pub fn removed(&self) -> Vec<&SemanticChange> {
        self.changes
            .iter()
            .filter(|c| c.change_type == SemanticChangeType::Removed)
            .collect()
    }

    /// Returns all entities with signature changes.
    pub fn signature_changed(&self) -> Vec<&SemanticChange> {
        self.changes
            .iter()
            .filter(|c| c.change_type == SemanticChangeType::SignatureChanged)
            .collect()
    }

    /// Returns all entities with implementation changes.
    pub fn implementation_changed(&self) -> Vec<&SemanticChange> {
        self.changes
            .iter()
            .filter(|c| c.change_type == SemanticChangeType::ImplementationChanged)
            .collect()
    }

    /// Returns true if any public entity had a signature change.
    /// This indicates a potentially breaking change.
    pub fn has_breaking_changes(&self) -> bool {
        self.changes.iter().any(|c| {
            c.is_public
                && (c.change_type == SemanticChangeType::SignatureChanged
                    || c.change_type == SemanticChangeType::Removed)
        })
    }
}

/// Compares two snapshots and produces a semantic diff.
pub fn diff_snapshots(old: &SemanticSnapshot, new: &SemanticSnapshot) -> SemanticDiff {
    let mut changes = Vec::new();

    // Check for removed and modified entities
    for (path, old_entity) in &old.entities {
        match new.entities.get(path) {
            None => {
                // Entity was removed
                changes.push(SemanticChange {
                    entity: path.clone(),
                    change_type: SemanticChangeType::Removed,
                    is_public: old_entity.is_public,
                });
            }
            Some(new_entity) => {
                // Entity exists in both — check for changes
                if old_entity.signature_hash != new_entity.signature_hash {
                    // Signature changed
                    changes.push(SemanticChange {
                        entity: path.clone(),
                        change_type: SemanticChangeType::SignatureChanged,
                        is_public: new_entity.is_public,
                    });
                } else if old_entity.body_hash != new_entity.body_hash {
                    // Only body changed
                    changes.push(SemanticChange {
                        entity: path.clone(),
                        change_type: SemanticChangeType::ImplementationChanged,
                        is_public: new_entity.is_public,
                    });
                }
                // else: unchanged, skip
            }
        }
    }

    // Check for added entities
    for (path, new_entity) in &new.entities {
        if !old.entities.contains_key(path) {
            changes.push(SemanticChange {
                entity: path.clone(),
                change_type: SemanticChangeType::Added,
                is_public: new_entity.is_public,
            });
        }
    }

    SemanticDiff { changes }
}
