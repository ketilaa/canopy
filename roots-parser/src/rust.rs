use streaming_iterator::StreamingIterator;
use tree_sitter::{Language as TsLanguage, Node, Parser, Query, QueryCursor};

use roots_core::{Language, Relationship, RelationshipKind, Symbol, SymbolKind};

use crate::extractor::{LanguageExtractor, ParseError, ParseOutput};

// Captures top-level and impl-level public items.
// Struct → Class, Enum → Enum, Trait → Interface, fn → Function/Method.
const RS_SYMBOL_QUERY: &str = r#"
(struct_item name: (type_identifier) @name) @struct
(enum_item   name: (type_identifier) @name) @enum
(trait_item  name: (type_identifier) @name) @trait
(function_item name: (identifier)    @name) @function
"#;

const RS_REL_QUERY: &str = r#"
(use_declaration argument: (scoped_identifier name: (identifier) @use_import))
(use_declaration argument: (use_list (identifier) @use_import))
(use_declaration argument: (identifier) @use_import)
"#;

struct RustExtractorInner {
    language:     TsLanguage,
    sym_query:    Query,
    rel_query:    Query,
    cap_name:     u32,
    cap_struct:   u32,
    cap_enum:     u32,
    cap_trait:    u32,
    cap_function: u32,
    rel_cap_use:  u32,
}

impl RustExtractorInner {
    fn new(language: TsLanguage) -> Result<Self, ParseError> {
        let sym_query = Query::new(&language, RS_SYMBOL_QUERY)
            .map_err(|e| ParseError::QueryCompile(e.to_string()))?;
        let cap_name     = sym_query.capture_index_for_name("name").unwrap();
        let cap_struct   = sym_query.capture_index_for_name("struct").unwrap();
        let cap_enum     = sym_query.capture_index_for_name("enum").unwrap();
        let cap_trait    = sym_query.capture_index_for_name("trait").unwrap();
        let cap_function = sym_query.capture_index_for_name("function").unwrap();

        let rel_query = Query::new(&language, RS_REL_QUERY)
            .map_err(|e| ParseError::QueryCompile(format!("rs rel query: {e}")))?;
        let rel_cap_use = rel_query.capture_index_for_name("use_import").unwrap();

        Ok(Self {
            language, sym_query, rel_query,
            cap_name, cap_struct, cap_enum, cap_trait, cap_function,
            rel_cap_use,
        })
    }

    fn extract_all(
        &self,
        source:        &str,
        relative_path: &str,
        project:       &str,
        workspace_id:  &str,
    ) -> Result<ParseOutput, ParseError> {
        let mut parser = Parser::new();
        parser.set_language(&self.language)
            .map_err(|_| ParseError::ParserInit { language: "rust".to_string() })?;

        let tree = parser.parse(source, None)
            .ok_or_else(|| ParseError::ParserInit { language: "rust".to_string() })?;

        // --- Symbol extraction ---
        let mut cursor = QueryCursor::new();
        let mut symbols = Vec::new();

        let mut matches = cursor.matches(&self.sym_query, tree.root_node(), source.as_bytes());
        while let Some(m) = matches.next() {
            let is_struct   = m.captures.iter().any(|c| c.index == self.cap_struct);
            let is_enum     = m.captures.iter().any(|c| c.index == self.cap_enum);
            let is_trait    = m.captures.iter().any(|c| c.index == self.cap_trait);
            let is_function = m.captures.iter().any(|c| c.index == self.cap_function);

            let outer_idx = if is_struct        { self.cap_struct }
                else if is_enum                 { self.cap_enum }
                else if is_trait                { self.cap_trait }
                else if is_function             { self.cap_function }
                else                            { continue };

            let outer_node = match m.captures.iter().find(|c| c.index == outer_idx) {
                Some(c) => c.node,
                None    => continue,
            };

            if !is_public(source, outer_node) {
                continue;
            }

            let name_cap = match m.captures.iter().find(|c| c.index == self.cap_name) {
                Some(c) => c,
                None    => continue,
            };
            let name = source[name_cap.node.byte_range()].to_string();

            let (kind, fqn) = if is_function {
                match enclosing_impl_type(source, outer_node) {
                    Some(type_name) => (
                        SymbolKind::Method,
                        format!("{}#{}#{}", relative_path, type_name, name),
                    ),
                    None => (
                        SymbolKind::Function,
                        format!("{}#{}", relative_path, name),
                    ),
                }
            } else if is_struct {
                (SymbolKind::Class,     format!("{}#{}", relative_path, name))
            } else if is_enum {
                (SymbolKind::Enum,      format!("{}#{}", relative_path, name))
            } else {
                (SymbolKind::Interface, format!("{}#{}", relative_path, name))
            };

            let signature = if matches!(kind, SymbolKind::Function | SymbolKind::Method) {
                rs_extract_signature(source, outer_node)
            } else {
                None
            };

            symbols.push(Symbol {
                name,
                kind,
                file:         relative_path.to_string(),
                language:     Language::Rust,
                project:      project.to_string(),
                workspace_id: workspace_id.to_string(),
                line:         name_cap.node.start_position().row as u32 + 1,
                fqn,
                signature,
            });
        }

        // --- Relationship extraction ---
        let mut rel_cursor = QueryCursor::new();
        let mut relationships = Vec::new();

        let mut rel_matches = rel_cursor.matches(&self.rel_query, tree.root_node(), source.as_bytes());
        while let Some(m) = rel_matches.next() {
            if let Some(cap) = m.captures.iter().find(|c| c.index == self.rel_cap_use) {
                relationships.push(Relationship {
                    from_symbol:  relative_path.to_string(),
                    to_symbol:    source[cap.node.byte_range()].to_string(),
                    kind:         RelationshipKind::Imports,
                    file:         relative_path.to_string(),
                    line:         Some(cap.node.start_position().row as u32 + 1),
                    workspace_id: workspace_id.to_string(),
                });
            }
        }

        Ok(ParseOutput { symbols, relationships })
    }
}

/// Returns true when the item node has a `pub` or `pub(...)` visibility modifier.
fn is_public(source: &str, node: Node) -> bool {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "visibility_modifier" {
                let text = &source[child.byte_range()];
                return text == "pub" || text.starts_with("pub(");
            }
        }
    }
    false
}

/// Walks up from a function_item node looking for an enclosing impl_item.
/// Returns the name of the implementing type when found.
fn enclosing_impl_type(source: &str, node: Node) -> Option<String> {
    let mut cur = node.parent();
    while let Some(n) = cur {
        if n.kind() == "impl_item" {
            if let Some(type_node) = n.child_by_field_name("type") {
                return Some(source[type_node.byte_range()].to_string());
            }
        }
        cur = n.parent();
    }
    None
}

/// Extracts `(params) -> ReturnType` from a function_item node.
fn rs_extract_signature(source: &str, function_node: Node) -> Option<String> {
    let params = function_node.child_by_field_name("parameters")?;
    let params_text = source[params.byte_range()].to_string();
    if let Some(ret) = function_node.child_by_field_name("return_type") {
        let ret_text = source[ret.byte_range()].to_string();
        Some(format!("{} -> {}", params_text, ret_text))
    } else {
        Some(params_text)
    }
}

pub struct RustExtractor {
    inner: RustExtractorInner,
}

impl RustExtractor {
    pub fn new() -> Result<Self, ParseError> {
        let lang = TsLanguage::from(tree_sitter_rust::LANGUAGE);
        Ok(Self { inner: RustExtractorInner::new(lang)? })
    }
}

impl LanguageExtractor for RustExtractor {
    fn extract(
        &self,
        source:        &str,
        relative_path: &str,
        project:       &str,
        workspace_id:  &str,
    ) -> Result<ParseOutput, ParseError> {
        self.inner.extract_all(source, relative_path, project, workspace_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extractor::LanguageExtractor;

    fn extractor() -> RustExtractor {
        RustExtractor::new().expect("extractor init")
    }

    #[test]
    fn extracts_public_struct() {
        let source = r#"
pub struct TechStackSkill {
    pub name: String,
    pub file_layout: String,
}
"#;
        let out = extractor().extract(source, "canopy-llm/src/lib.rs", "canopy-llm", "ws")
            .expect("parse ok");
        let sym = out.symbols.iter().find(|s| s.name == "TechStackSkill").expect("missing TechStackSkill");
        assert_eq!(sym.kind, SymbolKind::Class);
    }

    #[test]
    fn skips_private_struct() {
        let source = r#"
struct InternalState {
    value: u32,
}
"#;
        let out = extractor().extract(source, "src/lib.rs", "proj", "ws")
            .expect("parse ok");
        assert!(out.symbols.is_empty(), "private struct should not be indexed");
    }

    #[test]
    fn extracts_public_enum() {
        let source = r#"
pub enum ProviderKind {
    Anthropic,
    Ollama,
}
"#;
        let out = extractor().extract(source, "canopy-core/src/lib.rs", "canopy-core", "ws")
            .expect("parse ok");
        let sym = out.symbols.iter().find(|s| s.name == "ProviderKind").expect("missing ProviderKind");
        assert_eq!(sym.kind, SymbolKind::Enum);
    }

    #[test]
    fn extracts_public_trait() {
        let source = r#"
pub trait LanguageExtractor: Send + Sync {
    fn extract(&self, source: &str) -> Result<(), ()>;
}
"#;
        let out = extractor().extract(source, "roots-parser/src/extractor.rs", "roots-parser", "ws")
            .expect("parse ok");
        let sym = out.symbols.iter().find(|s| s.name == "LanguageExtractor").expect("missing trait");
        assert_eq!(sym.kind, SymbolKind::Interface);
    }

    #[test]
    fn extracts_public_free_function_with_signature() {
        let source = r#"
pub fn skill_for_technology(tech: &str, pkg: &str) -> String {
    String::new()
}
"#;
        let out = extractor().extract(source, "canopy-llm/src/lib.rs", "canopy-llm", "ws")
            .expect("parse ok");
        let sym = out.symbols.iter().find(|s| s.name == "skill_for_technology").expect("missing fn");
        assert_eq!(sym.kind, SymbolKind::Function);
        assert!(sym.signature.is_some(), "should have signature");
        let sig = sym.signature.as_ref().unwrap();
        assert!(sig.contains("tech"), "signature should contain param name: {sig}");
        assert!(sig.contains("String"), "signature should contain return type: {sig}");
    }

    #[test]
    fn extracts_impl_method_as_method_kind() {
        let source = r#"
pub struct MyService;

impl MyService {
    pub fn do_thing(&self, input: &str) -> bool {
        true
    }
}
"#;
        let out = extractor().extract(source, "src/service.rs", "proj", "ws")
            .expect("parse ok");
        let method = out.symbols.iter().find(|s| s.name == "do_thing").expect("missing method");
        assert_eq!(method.kind, SymbolKind::Method);
        assert!(method.fqn.contains("MyService"), "FQN should include impl type: {}", method.fqn);
    }

    #[test]
    fn skips_private_impl_method() {
        let source = r#"
pub struct Foo;

impl Foo {
    fn private_method(&self) {}
    pub fn public_method(&self) {}
}
"#;
        let out = extractor().extract(source, "src/foo.rs", "proj", "ws")
            .expect("parse ok");
        let names: Vec<&str> = out.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(!names.contains(&"private_method"), "private method should be skipped: {names:?}");
        assert!(names.contains(&"public_method"), "public method should be present: {names:?}");
    }
}
