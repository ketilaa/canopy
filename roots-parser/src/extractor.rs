use roots_core::{Relationship, Symbol};

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("failed to create parser for {language}")]
    ParserInit { language: String },
    #[error("source is not valid UTF-8: {0}")]
    Encoding(String),
    #[error("query compilation failed: {0}")]
    QueryCompile(String),
}

pub struct ParseOutput {
    pub symbols:       Vec<Symbol>,
    pub relationships: Vec<Relationship>,
}

pub trait LanguageExtractor: Send + Sync {
    fn extract(
        &self,
        source:        &str,
        relative_path: &str,
        project:       &str,
        workspace_id:  &str,
    ) -> Result<ParseOutput, ParseError>;
}
