use anyhow::Result;
use std::path::Path;
use tree_sitter::Tree;

use crate::core::types::{Evidence, StructuralChange};
use crate::semantic::differ::ast::diff_trees;
use crate::semantic::languages::LanguageAnalyzer;
use crate::semantic::parser::engine::ParserEngine;

pub struct TypeScriptAnalyzer;

impl TypeScriptAnalyzer {
    const fn new() -> Self {
        Self
    }

    fn create_parser(&self) -> Result<ParserEngine> {
        let lang = tree_sitter_typescript::language_typescript();
        ParserEngine::create_parser(&lang)
    }
}

impl LanguageAnalyzer for TypeScriptAnalyzer {
    fn language_id(&self) -> &'static str {
        "typescript"
    }

    fn parse(&self, source: &[u8]) -> Result<Tree> {
        let mut engine = self.create_parser()?;
        engine.parse(source)
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
