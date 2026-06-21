use streaming_iterator::StreamingIterator;
use tree_sitter::{Language as TsLanguage, Node, Parser, Query, QueryCursor};

use roots_core::{Language, Relationship, RelationshipKind, Symbol, SymbolKind};

use crate::extractor::{LanguageExtractor, ParseError, ParseOutput};

const TS_SYMBOL_QUERY: &str = r#"
(class_declaration     name: (type_identifier)     @name) @class
(interface_declaration name: (type_identifier)     @name) @interface
(enum_declaration      name: (identifier)          @name) @enum
(function_declaration  name: (identifier)          @name) @function
(method_definition     name: (property_identifier) @name) @method
"#;

const TS_REL_QUERY: &str = r#"
(import_statement
  (import_clause
    (named_imports
      (import_specifier name: (identifier) @named_import))))

(import_statement
  (import_clause (identifier) @default_import))

(class_declaration
  name: (type_identifier) @extends_class
  (class_heritage
    (extends_clause value: (identifier) @extends_name)))

(class_declaration
  name: (type_identifier) @impl_class
  (class_heritage
    (implements_clause (type_identifier) @impl_name)))

(call_expression
  function: (identifier) @free_call) @free_call_site

(call_expression
  function: (member_expression
    property: (property_identifier) @method_call)) @method_call_site
"#;

struct TsExtractorInner {
    ts_language: TsLanguage,
    sym_query: Query,
    rel_query: Query,
    // symbol captures
    cap_name:      u32,
    cap_class:     u32,
    cap_interface: u32,
    cap_enum:      u32,
    cap_function:  u32,
    cap_method:    u32,
    // relationship captures
    rel_cap_named_import:  u32,
    rel_cap_default_import: u32,
    rel_cap_extends_class: u32,
    rel_cap_extends_name:  u32,
    rel_cap_impl_class:    u32,
    rel_cap_impl_name:     u32,
    rel_cap_free_call:     u32,
    rel_cap_free_call_site: u32,
    rel_cap_method_call:   u32,
    rel_cap_method_call_site: u32,
}

impl TsExtractorInner {
    fn new(ts_language: TsLanguage) -> Result<Self, ParseError> {
        let sym_query = Query::new(&ts_language, TS_SYMBOL_QUERY)
            .map_err(|e| ParseError::QueryCompile(e.to_string()))?;
        let cap_name      = sym_query.capture_index_for_name("name").unwrap();
        let cap_class     = sym_query.capture_index_for_name("class").unwrap();
        let cap_interface = sym_query.capture_index_for_name("interface").unwrap();
        let cap_enum      = sym_query.capture_index_for_name("enum").unwrap();
        let cap_function  = sym_query.capture_index_for_name("function").unwrap();
        let cap_method    = sym_query.capture_index_for_name("method").unwrap();

        let rel_query = Query::new(&ts_language, TS_REL_QUERY)
            .map_err(|e| ParseError::QueryCompile(format!("ts rel query: {e}")))?;
        let rel_cap_named_import    = rel_query.capture_index_for_name("named_import").unwrap();
        let rel_cap_default_import  = rel_query.capture_index_for_name("default_import").unwrap();
        let rel_cap_extends_class   = rel_query.capture_index_for_name("extends_class").unwrap();
        let rel_cap_extends_name    = rel_query.capture_index_for_name("extends_name").unwrap();
        let rel_cap_impl_class      = rel_query.capture_index_for_name("impl_class").unwrap();
        let rel_cap_impl_name       = rel_query.capture_index_for_name("impl_name").unwrap();
        let rel_cap_free_call       = rel_query.capture_index_for_name("free_call").unwrap();
        let rel_cap_free_call_site  = rel_query.capture_index_for_name("free_call_site").unwrap();
        let rel_cap_method_call     = rel_query.capture_index_for_name("method_call").unwrap();
        let rel_cap_method_call_site = rel_query.capture_index_for_name("method_call_site").unwrap();

        Ok(Self {
            ts_language, sym_query, rel_query,
            cap_name, cap_class, cap_interface, cap_enum, cap_function, cap_method,
            rel_cap_named_import, rel_cap_default_import,
            rel_cap_extends_class, rel_cap_extends_name,
            rel_cap_impl_class, rel_cap_impl_name,
            rel_cap_free_call, rel_cap_free_call_site,
            rel_cap_method_call, rel_cap_method_call_site,
        })
    }

    fn extract_all(&self, source: &str, relative_path: &str, project: &str, workspace_id: &str, language: Language) -> Result<ParseOutput, ParseError> {
        let mut parser = Parser::new();
        parser.set_language(&self.ts_language)
            .map_err(|_| ParseError::ParserInit { language: language.to_string() })?;

        let tree = parser.parse(source, None)
            .ok_or_else(|| ParseError::ParserInit { language: language.to_string() })?;

        // --- Symbol extraction ---
        let mut cursor = QueryCursor::new();
        let mut symbols = Vec::new();

        let mut matches = cursor.matches(&self.sym_query, tree.root_node(), source.as_bytes());
        while let Some(m) = matches.next() {
            let kind = if m.captures.iter().any(|c| c.index == self.cap_class) {
                SymbolKind::Class
            } else if m.captures.iter().any(|c| c.index == self.cap_interface) {
                SymbolKind::Interface
            } else if m.captures.iter().any(|c| c.index == self.cap_enum) {
                SymbolKind::Enum
            } else if m.captures.iter().any(|c| c.index == self.cap_function) {
                SymbolKind::Function
            } else if m.captures.iter().any(|c| c.index == self.cap_method) {
                SymbolKind::Method
            } else {
                continue;
            };

            if let Some(name_cap) = m.captures.iter().find(|c| c.index == self.cap_name) {
                let node = name_cap.node;
                let name = source[node.byte_range()].to_string();
                let fqn = ts_symbol_fqn(source, node, relative_path, &name, kind == SymbolKind::Method);
                symbols.push(Symbol {
                    name,
                    kind,
                    file:         relative_path.to_string(),
                    language:     language.clone(),
                    project:      project.to_string(),
                    workspace_id: workspace_id.to_string(),
                    line:         node.start_position().row as u32 + 1,
                    fqn,
                });
            }
        }

        // --- Relationship extraction ---
        let mut rel_cursor = QueryCursor::new();
        let mut relationships = Vec::new();

        let mut rel_matches = rel_cursor.matches(&self.rel_query, tree.root_node(), source.as_bytes());
        while let Some(m) = rel_matches.next() {
            // Named import: import { Foo } from './mod'
            if let Some(cap) = m.captures.iter().find(|c| c.index == self.rel_cap_named_import) {
                let to_symbol = source[cap.node.byte_range()].to_string();
                relationships.push(Relationship {
                    from_symbol:  relative_path.to_string(),
                    to_symbol,
                    kind:         RelationshipKind::Imports,
                    file:         relative_path.to_string(),
                    line:         Some(cap.node.start_position().row as u32 + 1),
                    workspace_id: workspace_id.to_string(),
                });
                continue;
            }

            // Default import: import Foo from './mod'
            if let Some(cap) = m.captures.iter().find(|c| c.index == self.rel_cap_default_import) {
                let to_symbol = source[cap.node.byte_range()].to_string();
                relationships.push(Relationship {
                    from_symbol:  relative_path.to_string(),
                    to_symbol,
                    kind:         RelationshipKind::Imports,
                    file:         relative_path.to_string(),
                    line:         Some(cap.node.start_position().row as u32 + 1),
                    workspace_id: workspace_id.to_string(),
                });
                continue;
            }

            // EXTENDS
            if let (Some(class_cap), Some(extends_cap)) = (
                m.captures.iter().find(|c| c.index == self.rel_cap_extends_class),
                m.captures.iter().find(|c| c.index == self.rel_cap_extends_name),
            ) {
                let class_name = &source[class_cap.node.byte_range()];
                relationships.push(Relationship {
                    from_symbol:  format!("{}#{}", relative_path, class_name),
                    to_symbol:    source[extends_cap.node.byte_range()].to_string(),
                    kind:         RelationshipKind::Extends,
                    file:         relative_path.to_string(),
                    line:         Some(class_cap.node.start_position().row as u32 + 1),
                    workspace_id: workspace_id.to_string(),
                });
                continue;
            }

            // IMPLEMENTS
            if let (Some(class_cap), Some(iface_cap)) = (
                m.captures.iter().find(|c| c.index == self.rel_cap_impl_class),
                m.captures.iter().find(|c| c.index == self.rel_cap_impl_name),
            ) {
                let class_name = &source[class_cap.node.byte_range()];
                relationships.push(Relationship {
                    from_symbol:  format!("{}#{}", relative_path, class_name),
                    to_symbol:    source[iface_cap.node.byte_range()].to_string(),
                    kind:         RelationshipKind::Implements,
                    file:         relative_path.to_string(),
                    line:         Some(class_cap.node.start_position().row as u32 + 1),
                    workspace_id: workspace_id.to_string(),
                });
                continue;
            }

            // Free function call
            if let (Some(site_cap), Some(callee_cap)) = (
                m.captures.iter().find(|c| c.index == self.rel_cap_free_call_site),
                m.captures.iter().find(|c| c.index == self.rel_cap_free_call),
            ) {
                let from_symbol = ts_enclosing_fqn(source, site_cap.node, relative_path);
                relationships.push(Relationship {
                    from_symbol,
                    to_symbol:    source[callee_cap.node.byte_range()].to_string(),
                    kind:         RelationshipKind::Calls,
                    file:         relative_path.to_string(),
                    line:         Some(site_cap.node.start_position().row as u32 + 1),
                    workspace_id: workspace_id.to_string(),
                });
                continue;
            }

            // Method call (obj.method())
            if let (Some(site_cap), Some(callee_cap)) = (
                m.captures.iter().find(|c| c.index == self.rel_cap_method_call_site),
                m.captures.iter().find(|c| c.index == self.rel_cap_method_call),
            ) {
                let from_symbol = ts_enclosing_fqn(source, site_cap.node, relative_path);
                relationships.push(Relationship {
                    from_symbol,
                    to_symbol:    source[callee_cap.node.byte_range()].to_string(),
                    kind:         RelationshipKind::Calls,
                    file:         relative_path.to_string(),
                    line:         Some(site_cap.node.start_position().row as u32 + 1),
                    workspace_id: workspace_id.to_string(),
                });
            }
        }

        Ok(ParseOutput { symbols, relationships })
    }
}

fn ts_symbol_fqn(source: &str, node: Node, relative_path: &str, name: &str, is_method: bool) -> String {
    if is_method {
        // Walk up to find class
        let mut cur = node.parent();
        while let Some(n) = cur {
            if n.kind() == "class_declaration" {
                if let Some(class_name_node) = n.child_by_field_name("name") {
                    let class_name = &source[class_name_node.byte_range()];
                    return format!("{}#{}#{}", relative_path, class_name, name);
                }
            }
            cur = n.parent();
        }
        format!("{}#{}", relative_path, name)
    } else {
        format!("{}#{}", relative_path, name)
    }
}

fn ts_enclosing_fqn(source: &str, node: Node, relative_path: &str) -> String {
    let mut cur = Some(node);
    while let Some(n) = cur {
        match n.kind() {
            "method_definition" => {
                if let Some(name_node) = n.child_by_field_name("name") {
                    let method_name = &source[name_node.byte_range()];
                    // Find enclosing class
                    let mut cls = n.parent();
                    while let Some(c) = cls {
                        if c.kind() == "class_declaration" {
                            if let Some(cn) = c.child_by_field_name("name") {
                                let class_name = &source[cn.byte_range()];
                                return format!("{}#{}#{}", relative_path, class_name, method_name);
                            }
                        }
                        cls = c.parent();
                    }
                    return format!("{}#{}", relative_path, method_name);
                }
            }
            "function_declaration" => {
                if let Some(name_node) = n.child_by_field_name("name") {
                    let fn_name = &source[name_node.byte_range()];
                    return format!("{}#{}", relative_path, fn_name);
                }
            }
            _ => {}
        }
        cur = n.parent();
    }
    relative_path.to_string()
}

pub struct TypeScriptExtractor {
    ts: TsExtractorInner,
    tsx: TsExtractorInner,
}

impl TypeScriptExtractor {
    pub fn new() -> Result<Self, ParseError> {
        let ts = TsExtractorInner::new(TsLanguage::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT))?;
        let tsx = TsExtractorInner::new(TsLanguage::from(tree_sitter_typescript::LANGUAGE_TSX))?;
        Ok(Self { ts, tsx })
    }
}

impl LanguageExtractor for TypeScriptExtractor {
    fn extract(&self, source: &str, relative_path: &str, project: &str, workspace_id: &str) -> Result<ParseOutput, ParseError> {
        let is_tsx = relative_path.ends_with(".tsx");
        if is_tsx {
            self.tsx.extract_all(source, relative_path, project, workspace_id, Language::TypeScript)
        } else {
            self.ts.extract_all(source, relative_path, project, workspace_id, Language::TypeScript)
        }
    }
}
