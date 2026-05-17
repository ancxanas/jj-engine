use crate::core::types::AnalysisResult;

pub trait Renderer {
    fn render(&self, result: &AnalysisResult) -> String;
}
