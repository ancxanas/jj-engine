use std::collections::HashMap;
use std::str;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeKey {
    pub kind: String,
    pub name: String,
}

pub struct SignificantNode {
    pub key: NodeKey,
    pub content: String,
    pub start_line: usize,
    pub start_col: usize,
}

/// Extracts significant nodes from a tree-sitter AST.
///
/// Traverses the tree and collects nodes that represent code structures:
/// - `function_declaration`
/// - `method_definition`
/// - `class_declaration`
/// - `call_expression` (test cases)
/// - `export_statement`
/// - `import_statement`
///
/// # Panics
///
/// Panics if tree-sitter child access returns `None` (should not happen for valid trees).
#[must_use]
pub fn extract(tree: &tree_sitter::Tree, source: &[u8]) -> HashMap<NodeKey, SignificantNode> {
    let mut nodes = HashMap::new();
    let mut cursor = tree.walk();

    // Use a simple stack-based traversal
    let mut stack = vec![tree.root_node()];

    while let Some(node) = stack.pop() {
        cursor.reset(node);
        let kind = node.kind();

        if is_significant_kind(kind) {
            if let Some(name) = extract_name(&node, source) {
                let key = NodeKey {
                    kind: kind.to_string(),
                    name: name.clone(),
                };
                if let Some(content_str) = extract_content(node, source) {
                    nodes.insert(
                        key.clone(),
                        SignificantNode {
                            key,
                            content: content_str,
                            start_line: node.start_position().row + 1,
                            start_col: node.start_position().column,
                        },
                    );
                }
            }
        }

        // Add children to stack
        // SAFETY: child_count() returns the number of children; i is always < child_count()
        for i in 0..node.child_count() {
            #[allow(clippy::unwrap_used)]
            stack.push(node.child(i).unwrap());
        }
    }

    nodes
}

fn is_significant_kind(kind: &str) -> bool {
    matches!(
        kind,
        "function_declaration"
            | "method_definition"
            | "class_declaration"
            | "call_expression"
            | "export_statement"
            | "import_statement"
            | "try_statement"
    )
}

fn extract_name(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
    // For functions/methods: look for identifier child
    // For classes: identifier child
    // For call_expression: the function name being called
    // For import/export: look for module name or imported identifiers

    match node.kind() {
        "function_declaration" | "method_definition" | "class_declaration" => {
            find_identifier_child(node, source)
        }
        "call_expression" => {
            // For tests, we want the function name being called (e.g., "it", "describe", "test")
            find_identifier_child(node, source)
        }
        "export_statement" => {
            // Try to get the exported name
            find_identifier_child(node, source).or_else(|| {
                // For "export default ...", capture "default"
                if node.child_by_field_name("default").is_some() {
                    Some("default".to_string())
                } else {
                    Some("export".to_string())
                }
            })
        }
        "import_statement" => node
            .child_by_field_name("source")
            .map_or_else(|| Some("import".to_string()), |module_node| extract_content(module_node, source)),
        "try_statement" => {
            // Try to find enclosing function name for context
            find_enclosing_function_name(*node, source).or_else(|| Some("try".to_string()))
        }
        _ => None,
    }
}

fn find_identifier_child(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
    for i in 0..node.child_count() {
        // SAFETY: child_count() returns the number of children; i is always < child_count()
        #[allow(clippy::unwrap_used)]
        let child = node.child(i).unwrap();
        if child.kind() == "identifier" {
            return extract_content(child, source);
        }
    }
    None
}

/// Walk up the tree to find the enclosing function name for context.
fn find_enclosing_function_name(node: tree_sitter::Node, source: &[u8]) -> Option<String> {
    let mut current = node.parent();
    while let Some(parent) = current {
        match parent.kind() {
            "function_declaration" | "method_definition" | "arrow_function" | "function" => {
                return find_identifier_child(&parent, source);
            }
            _ => {
                current = parent.parent();
            }
        }
    }
    None
}

fn extract_content(node: tree_sitter::Node, source: &[u8]) -> Option<String> {
    let range = node.byte_range();
    if range.start < source.len() && range.end <= source.len() {
        std::str::from_utf8(&source[range]).ok().map(String::from)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tree_sitter::{Language, Parser};

    fn parse(source: &[u8], language: &Language) -> tree_sitter::Tree {
        let mut parser = Parser::new();
        parser.set_language(language).unwrap();
        parser.parse(source, None).unwrap()
    }

    #[test]
    fn test_extract_simple_function() {
        let src = b"function greet() { return 'hello'; }";
        let lang = tree_sitter_javascript::language();
        let tree = parse(src, &lang);
        let nodes = extract(&tree, src);

        assert!(nodes.contains_key(&NodeKey {
            kind: "function_declaration".to_string(),
            name: "greet".to_string()
        }));
    }

    #[test]
    fn test_extract_class() {
        let src = b"class User { constructor(name) { this.name = name; } }";
        let lang = tree_sitter_javascript::language();
        let tree = parse(src, &lang);
        let nodes = extract(&tree, src);

        assert!(nodes.contains_key(&NodeKey {
            kind: "class_declaration".to_string(),
            name: "User".to_string()
        }));
    }
}
