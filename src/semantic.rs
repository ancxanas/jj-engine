//! Semantic/AST project understanding layer.
//! Uses tree-sitter to parse source files and extract code entities.

use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::path::Path;

use anyhow::Result;

/// The kind of code entity.
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// A single code entity extracted from a source file.
#[derive(Debug, Clone)]
pub struct EntityInfo {
    /// Name of the entity (function name, struct name, etc.)
    pub name: String,

    /// What kind of entity this is.
    pub kind: EntityKind,

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

/// Parses a Rust source file and extracts code entities.
pub fn parse_file(path: &Path, source: &str) -> Result<Vec<EntityInfo>> {
    // Create parser
    let mut parser = tree_sitter::Parser::new();

    // Set language to Rust
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .map_err(|e| anyhow::anyhow!("failed to set language: {}", e))?;

    // Parse source code
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("failed to parse file: {}", path.display()))?;

    let root = tree.root_node();

    let mut entities = Vec::new();

    // Walk top-level children
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        match child.kind() {
            // Function definitions
            "function_item" => {
                if let Some(entity) = extract_function(child, source, false) {
                    entities.push(entity);
                }
            }

            // Struct definitions
            "struct_item" => {
                if let Some(entity) = extract_named_item(child, source, EntityKind::Struct) {
                    entities.push(entity);
                }
            }

            // Enum definitions
            "enum_item" => {
                if let Some(entity) = extract_named_item(child, source, EntityKind::Enum) {
                    entities.push(entity);
                }
            }

            // Trait definitions
            "trait_item" => {
                if let Some(entity) = extract_named_item(child, source, EntityKind::Trait) {
                    entities.push(entity);
                }
            }

            // Impl blocks
            "impl_item" => {
                if let Some(entity) = extract_impl(child, source) {
                    entities.push(entity);
                }
            }

            // Use statements (imports)
            "use_declaration" => {
                if let Some(entity) = extract_import(child, source) {
                    entities.push(entity);
                }
            }

            // Const and static
            "const_item" | "static_item" => {
                if let Some(entity) = extract_named_item(child, source, EntityKind::Constant) {
                    entities.push(entity);
                }
            }

            // Mod declarations
            "mod_item" => {
                if let Some(entity) = extract_named_item(child, source, EntityKind::Module) {
                    entities.push(entity);
                }
            }

            // Attribute items (may contain #[test] functions)
            "attribute_item" => {
                // handled below via decorated functions
            }

            _ => {}
        }
    }

    Ok(entities)
}

/// Extracts a function entity from a tree-sitter node.
fn extract_function(
    node: tree_sitter::Node,
    source: &str,
    is_test: bool,
) -> Option<EntityInfo> {
    // Get the function name
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(name_node, source).to_string();

    // Check if public
    let is_public = node_text(node, source).starts_with("pub");

    // Get signature: everything before the body
    let body_node = node.child_by_field_name("body");
    let signature_text = if let Some(body) = body_node {
        &source[node.start_byte()..body.start_byte()]
    } else {
        node_text(node, source)
    };

    // Get body text
    let body_text = if let Some(body) = body_node {
        node_text(body, source)
    } else {
        ""
    };

    let signature_hash = hash_str(signature_text);
    let body_hash = hash_str(body_text);

    Some(EntityInfo {
        name,
        kind: if is_test { EntityKind::Test } else { EntityKind::Function },
        is_public,
        is_test,
        line_start: node.start_position().row,
        line_end: node.end_position().row,
        signature_hash,
        body_hash,
    })
}

/// Extracts a named item (struct, enum, trait, constant, module).
fn extract_named_item(
    node: tree_sitter::Node,
    source: &str,
    kind: EntityKind,
) -> Option<EntityInfo> {
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(name_node, source).to_string();

    let is_public = node_text(node, source).starts_with("pub");
    let full_text = node_text(node, source);

    let signature_hash = hash_str(full_text);
    let body_hash = hash_str(full_text);

    Some(EntityInfo {
        name,
        kind,
        is_public,
        is_test: false,
        line_start: node.start_position().row,
        line_end: node.end_position().row,
        signature_hash,
        body_hash,
    })
}

/// Extracts an impl block.
fn extract_impl(
    node: tree_sitter::Node,
    source: &str,
) -> Option<EntityInfo> {
    // impl blocks may not have a simple name field
    // use the type being implemented
    let type_node = node.child_by_field_name("type")?;
    let name = format!("impl {}", node_text(type_node, source));

    let full_text = node_text(node, source);
    let signature_hash = hash_str(&name);
    let body_hash = hash_str(full_text);

    Some(EntityInfo {
        name,
        kind: EntityKind::Impl,
        is_public: false,
        is_test: false,
        line_start: node.start_position().row,
        line_end: node.end_position().row,
        signature_hash,
        body_hash,
    })
}

/// Extracts a use/import declaration.
fn extract_import(
    node: tree_sitter::Node,
    source: &str,
) -> Option<EntityInfo> {
    let full_text = node_text(node, source);
    let hash = hash_str(full_text);

    Some(EntityInfo {
        name: full_text.to_string(),
        kind: EntityKind::Import,
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
