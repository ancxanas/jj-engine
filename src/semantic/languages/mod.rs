use anyhow::Result;
use std::path::Path;
use tree_sitter::Tree;

use crate::core::types::{Evidence, StructuralChange};
use crate::semantic::differ::ast::diff_trees;

pub trait LanguageAnalyzer: Send + Sync {
    fn language_id(&self) -> &'static str;
    fn parse(&self, source: &[u8]) -> Result<Tree>;
    fn extract_changes(
        &self,
        before: &Tree,
        after: &Tree,
        before_src: &[u8],
        after_src: &[u8],
    ) -> Result<Vec<StructuralChange>>;
    fn detect_evidence(&self, changes: &[StructuralChange]) -> Vec<Evidence>;
}

pub struct TypeScriptAnalyzer;

impl TypeScriptAnalyzer {
    const fn new() -> Self {
        Self
    }

    fn create_parser() -> Result<tree_sitter::Parser> {
        let lang = tree_sitter_typescript::language_typescript();
        crate::semantic::parser::engine::create_parser(&lang)
    }
}

impl LanguageAnalyzer for TypeScriptAnalyzer {
    fn language_id(&self) -> &'static str {
        "typescript"
    }

    fn parse(&self, source: &[u8]) -> Result<Tree> {
        let mut parser = Self::create_parser()?;
        crate::semantic::parser::engine::parse(&mut parser, source)
    }

    fn extract_changes(
        &self,
        before: &Tree,
        after: &Tree,
        before_src: &[u8],
        after_src: &[u8],
    ) -> Result<Vec<StructuralChange>> {
        diff_trees(before, after, before_src, after_src)
    }

    fn detect_evidence(&self, changes: &[StructuralChange]) -> Vec<Evidence> {
        crate::intent::evidence::detect_all(changes)
    }
}

#[must_use]
pub fn analyzer_for(path: &Path) -> Option<&'static dyn LanguageAnalyzer> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    let filename = path.file_name()?.to_str()?;

    // Check for TypeScript/JavaScript files
    if matches!(
        ext.as_str(),
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "jsm"
    ) || {
        // Also check common filenames without extension that are typically JS/TS
        filename == "package.json" || filename == "tsconfig.json"
    } {
        Some(&TS_ANALYZER)
    } else {
        None
    }
}

// Global static analyzer instance
static TS_ANALYZER: TypeScriptAnalyzer = TypeScriptAnalyzer::new();

#[allow(dead_code)]
fn get_analyzer() -> &'static dyn LanguageAnalyzer {
    &TS_ANALYZER
}
