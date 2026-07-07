use roots_core::Language;

use crate::extractor::{LanguageExtractor, ParseError, ParseOutput};
use crate::java::JavaExtractor;
use crate::kotlin::KotlinExtractor;
use crate::rust::RustExtractor;
use crate::typescript::TypeScriptExtractor;

pub fn extract(
    source:        &[u8],
    language:      &Language,
    relative_path: &str,
    project:       &str,
    workspace_id:  &str,
) -> Result<ParseOutput, ParseError> {
    let source_str = std::str::from_utf8(source)
        .map_err(|e| ParseError::Encoding(e.to_string()))?;

    match language {
        Language::Java => {
            let extractor = JavaExtractor::new()?;
            extractor.extract(source_str, relative_path, project, workspace_id)
        }
        Language::Kotlin => {
            let extractor = KotlinExtractor::new()?;
            extractor.extract(source_str, relative_path, project, workspace_id)
        }
        Language::TypeScript => {
            let extractor = TypeScriptExtractor::new()?;
            extractor.extract(source_str, relative_path, project, workspace_id)
        }
        Language::Rust => {
            let extractor = RustExtractor::new()?;
            extractor.extract(source_str, relative_path, project, workspace_id)
        }
    }
}
