use anyhow::Result;
use std::path::PathBuf;
use tree_sitter::Tree;

use super::nodes::{self, SignificantNode};
use crate::core::types::{Location, StructuralChange, StructuralChangeKind};

pub fn diff_trees(
    before: &Tree,
    after: &Tree,
    before_src: &[u8],
    after_src: &[u8],
) -> Result<Vec<StructuralChange>> {
    let before_nodes = nodes::extract(before, before_src);
    let after_nodes = nodes::extract(after, after_src);
    let mut changes = Vec::new();

    for (key, before_node) in &before_nodes {
        match after_nodes.get(key) {
            None => changes.push(make_change(
                removal_kind(&key.kind),
                &before_node.key.name,
                "removed",
                before_node,
            )),
            Some(after_node) if before_node.content != after_node.content => {
                changes.push(make_change(
                    modification_kind(&key.kind),
                    &after_node.key.name,
                    "modified",
                    after_node,
                ));
            }
            _ => {}
        }
    }

    for (key, after_node) in &after_nodes {
        if !before_nodes.contains_key(key) {
            changes.push(make_change(
                addition_kind(&key.kind),
                &after_node.key.name,
                "added",
                after_node,
            ));
        }
    }

    Ok(changes)
}

fn make_change(
    kind: StructuralChangeKind,
    name: &str,
    detail: &str,
    node: &SignificantNode,
) -> StructuralChange {
    StructuralChange {
        kind,
        name: name.to_string(),
        detail: detail.to_string(),
        location: Location {
            file: PathBuf::new(),
            line: node.start_line,
            column: node.start_col,
        },
    }
}

fn addition_kind(kind: &str) -> StructuralChangeKind {
    match kind {
        "class_declaration" => StructuralChangeKind::ClassAdded,
        "method_definition" => StructuralChangeKind::MethodAdded,
        "interface_declaration" => StructuralChangeKind::InterfaceAdded,
        "type_alias_declaration" => StructuralChangeKind::TypeAdded,
        "import_statement" => StructuralChangeKind::ImportAdded,
        "export_statement" => StructuralChangeKind::ExportAdded,
        "call_expression" => StructuralChangeKind::TestCaseAdded,
        "try_statement" => StructuralChangeKind::TryCatchAdded,
        _ => StructuralChangeKind::FunctionAdded,
    }
}

fn removal_kind(kind: &str) -> StructuralChangeKind {
    match kind {
        "class_declaration" => StructuralChangeKind::ClassRemoved,
        "method_definition" => StructuralChangeKind::MethodRemoved,
        "interface_declaration" => StructuralChangeKind::InterfaceRemoved,
        "type_alias_declaration" => StructuralChangeKind::TypeRemoved,
        "import_statement" => StructuralChangeKind::ImportRemoved,
        "export_statement" => StructuralChangeKind::ExportRemoved,
        _ => StructuralChangeKind::FunctionRemoved,
    }
}

fn modification_kind(kind: &str) -> StructuralChangeKind {
    match kind {
        "class_declaration" => StructuralChangeKind::ClassModified,
        "method_definition" => StructuralChangeKind::MethodModified,
        "interface_declaration" => StructuralChangeKind::InterfaceModified,
        "type_alias_declaration" => StructuralChangeKind::TypeModified,
        "call_expression" => StructuralChangeKind::TestCaseModified,
        _ => StructuralChangeKind::FunctionModified,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic::parser::engine;

    fn parse_ts(src: &[u8]) -> anyhow::Result<Tree> {
        let lang = tree_sitter_typescript::language_typescript();
        let mut parser = engine::create_parser(&lang)?;
        engine::parse(&mut parser, src)
    }

    #[test]
    fn detects_added_function() -> anyhow::Result<()> {
        let before = b"const x = 1;";
        let after = b"const x = 1;\nfunction greet() { return 'hi'; }";
        let before_tree = parse_ts(before)?;
        let after_tree = parse_ts(after)?;
        let changes = diff_trees(&before_tree, &after_tree, before, after)?;

        assert!(changes
            .iter()
            .any(|c| c.kind == StructuralChangeKind::FunctionAdded && c.name == "greet"));
        Ok(())
    }

    #[test]
    fn detects_removed_function() -> anyhow::Result<()> {
        let before = b"function greet() { return 'hi'; }";
        let after = b"const x = 1;";
        let before_tree = parse_ts(before)?;
        let after_tree = parse_ts(after)?;
        let changes = diff_trees(&before_tree, &after_tree, before, after)?;

        assert!(changes
            .iter()
            .any(|c| c.kind == StructuralChangeKind::FunctionRemoved && c.name == "greet"));
        Ok(())
    }

    #[test]
    fn detects_modified_function() -> anyhow::Result<()> {
        let before = b"function greet() { return 'hi'; }";
        let after = b"function greet() { return 'hello'; }";
        let before_tree = parse_ts(before)?;
        let after_tree = parse_ts(after)?;
        let changes = diff_trees(&before_tree, &after_tree, before, after)?;

        assert!(changes
            .iter()
            .any(|c| c.kind == StructuralChangeKind::FunctionModified && c.name == "greet"));
        Ok(())
    }

    #[test]
    fn no_changes_for_identical_code() -> anyhow::Result<()> {
        let src = b"function greet() { return 'hi'; }";
        let tree1 = parse_ts(src)?;
        let tree2 = parse_ts(src)?;
        let changes = diff_trees(&tree1, &tree2, src, src)?;

        assert!(changes.is_empty());
        Ok(())
    }
}
