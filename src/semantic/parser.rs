use tree_sitter::{Parser, Tree};

pub mod engine {
    use super::{Parser, Tree};
    use anyhow::Result;

    pub fn create_parser(language: &tree_sitter::Language) -> Result<Parser> {
        let mut parser = Parser::new();
        parser.set_language(language)?;
        Ok(parser)
    }

    pub fn parse(parser: &mut Parser, source: &[u8]) -> Result<Tree> {
        parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("failed to parse source"))
    }
}
