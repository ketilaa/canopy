use streaming_iterator::StreamingIterator;
use tree_sitter::{Language as TsLanguage, Node, Parser, Query, QueryCursor, Tree};

use roots_core::{Language, Relationship, RelationshipKind, Symbol, SymbolKind};

use crate::extractor::{LanguageExtractor, ParseError, ParseOutput};

const JAVA_SYMBOL_QUERY: &str = r#"
(class_declaration       name: (identifier) @name) @class
(interface_declaration   name: (identifier) @name) @interface
(enum_declaration        name: (identifier) @name) @enum
(method_declaration      name: (identifier) @name) @method
(constructor_declaration name: (identifier) @name) @constructor
"#;

const JAVA_REL_QUERY: &str = r#"
(import_declaration) @import

(class_declaration
  name: (identifier) @extends_class
  superclass: (superclass (type_identifier) @extends_name))

(class_declaration
  name: (identifier) @impl_class
  interfaces: (super_interfaces
    (type_list (type_identifier) @impl_name)))

(method_invocation
  name: (identifier) @callee_name) @call_site
"#;

pub struct JavaExtractor {
    ts_language: TsLanguage,
    sym_query: Query,
    rel_query: Query,
    // symbol query capture indices
    cap_name:        u32,
    cap_class:       u32,
    cap_interface:   u32,
    cap_enum:        u32,
    cap_method:      u32,
    cap_constructor: u32,
    // relationship query capture indices
    rel_cap_import:       u32,
    rel_cap_extends_class: u32,
    rel_cap_extends_name:  u32,
    rel_cap_impl_class:    u32,
    rel_cap_impl_name:     u32,
    rel_cap_callee:        u32,
    rel_cap_call_site:     u32,
}

impl JavaExtractor {
    pub fn new() -> Result<Self, ParseError> {
        let ts_language = TsLanguage::from(tree_sitter_java::LANGUAGE);

        let sym_query = Query::new(&ts_language, JAVA_SYMBOL_QUERY)
            .map_err(|e| ParseError::QueryCompile(e.to_string()))?;
        let cap_name        = sym_query.capture_index_for_name("name").unwrap();
        let cap_class       = sym_query.capture_index_for_name("class").unwrap();
        let cap_interface   = sym_query.capture_index_for_name("interface").unwrap();
        let cap_enum        = sym_query.capture_index_for_name("enum").unwrap();
        let cap_method      = sym_query.capture_index_for_name("method").unwrap();
        let cap_constructor = sym_query.capture_index_for_name("constructor").unwrap();

        let rel_query = Query::new(&ts_language, JAVA_REL_QUERY)
            .map_err(|e| ParseError::QueryCompile(format!("java rel query: {e}")))?;
        let rel_cap_import        = rel_query.capture_index_for_name("import").unwrap();
        let rel_cap_extends_class = rel_query.capture_index_for_name("extends_class").unwrap();
        let rel_cap_extends_name  = rel_query.capture_index_for_name("extends_name").unwrap();
        let rel_cap_impl_class    = rel_query.capture_index_for_name("impl_class").unwrap();
        let rel_cap_impl_name     = rel_query.capture_index_for_name("impl_name").unwrap();
        let rel_cap_callee        = rel_query.capture_index_for_name("callee_name").unwrap();
        let rel_cap_call_site     = rel_query.capture_index_for_name("call_site").unwrap();

        Ok(Self {
            ts_language,
            sym_query,
            rel_query,
            cap_name, cap_class, cap_interface, cap_enum, cap_method, cap_constructor,
            rel_cap_import, rel_cap_extends_class, rel_cap_extends_name,
            rel_cap_impl_class, rel_cap_impl_name, rel_cap_callee, rel_cap_call_site,
        })
    }
}

/// Walk up from a name node to find the enclosing constructor or method declaration,
/// then extract the text of its `formal_parameters` child.
fn extract_params<'a>(source: &str, name_node: Node<'a>) -> Option<String> {
    let mut cur = name_node.parent();
    while let Some(n) = cur {
        if matches!(n.kind(), "constructor_declaration" | "method_declaration") {
            for i in 0..n.child_count() {
                if let Some(child) = n.child(i) {
                    if child.kind() == "formal_parameters" {
                        return Some(source[child.byte_range()].to_string());
                    }
                }
            }
        }
        cur = n.parent();
    }
    None
}

fn extract_package(source: &str, tree: &Tree) -> String {
    let root = tree.root_node();
    for i in 0..root.child_count() {
        if let Some(child) = root.child(i) {
            if child.kind() == "package_declaration" {
                let text = source[child.byte_range()].trim();
                let without_kw = text.strip_prefix("package").unwrap_or(text).trim();
                return without_kw.trim_end_matches(';').trim().to_string();
            }
        }
    }
    String::new()
}

fn build_class_fqn(class_name: &str, package: &str) -> String {
    if package.is_empty() {
        class_name.to_string()
    } else {
        format!("{}.{}", package, class_name)
    }
}

fn class_fqn_for_node<'a>(source: &str, node: Node<'a>, package: &str) -> String {
    let mut cur = Some(node);
    while let Some(n) = cur {
        if n.kind() == "class_declaration" {
            if let Some(name_node) = n.child_by_field_name("name") {
                let name = &source[name_node.byte_range()];
                return build_class_fqn(name, package);
            }
        }
        cur = n.parent();
    }
    String::new()
}

fn method_fqn_for_node<'a>(source: &str, node: Node<'a>, package: &str) -> Option<String> {
    let mut cur = Some(node);
    while let Some(n) = cur {
        match n.kind() {
            "method_declaration" | "constructor_declaration" => {
                if let Some(name_node) = n.child_by_field_name("name") {
                    let method_name = &source[name_node.byte_range()];
                    let class_fqn = class_fqn_for_node(source, n, package);
                    return Some(if class_fqn.is_empty() {
                        method_name.to_string()
                    } else {
                        format!("{}.{}", class_fqn, method_name)
                    });
                }
            }
            _ => {}
        }
        cur = n.parent();
    }
    None
}

impl LanguageExtractor for JavaExtractor {
    fn extract(&self, source: &str, relative_path: &str, project: &str, workspace_id: &str) -> Result<ParseOutput, ParseError> {
        let mut parser = Parser::new();
        parser.set_language(&self.ts_language)
            .map_err(|_| ParseError::ParserInit { language: "java".into() })?;

        let tree = parser.parse(source, None)
            .ok_or_else(|| ParseError::ParserInit { language: "java".into() })?;

        let package = extract_package(source, &tree);

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
            } else if m.captures.iter().any(|c| c.index == self.cap_method) {
                SymbolKind::Method
            } else if m.captures.iter().any(|c| c.index == self.cap_constructor) {
                SymbolKind::Method
            } else {
                continue;
            };

            if let Some(name_cap) = m.captures.iter().find(|c| c.index == self.cap_name) {
                let node = name_cap.node;
                let name = source[node.byte_range()].to_string();
                let fqn = match kind {
                    SymbolKind::Method => {
                        method_fqn_for_node(source, node, &package)
                            .unwrap_or_else(|| name.clone())
                    }
                    _ => build_class_fqn(&name, &package),
                };
                let signature = match kind {
                    SymbolKind::Method => extract_params(source, node),
                    _ => None,
                };
                symbols.push(Symbol {
                    name,
                    kind,
                    file:         relative_path.to_string(),
                    language:     Language::Java,
                    project:      project.to_string(),
                    workspace_id: workspace_id.to_string(),
                    line:         node.start_position().row as u32 + 1,
                    fqn,
                    signature,
                });
            }
        }

        // --- Relationship extraction ---
        let mut rel_cursor = QueryCursor::new();
        let mut relationships = Vec::new();

        let mut rel_matches = rel_cursor.matches(&self.rel_query, tree.root_node(), source.as_bytes());
        while let Some(m) = rel_matches.next() {
            // IMPORTS
            if let Some(cap) = m.captures.iter().find(|c| c.index == self.rel_cap_import) {
                let text = source[cap.node.byte_range()].trim();
                // "import com.example.Foo;" or "import static com.example.Foo.method;"
                let without_kw = text.strip_prefix("import").unwrap_or(text).trim();
                let without_static = without_kw.strip_prefix("static").unwrap_or(without_kw).trim();
                let to_symbol = without_static.trim_end_matches(';').trim().to_string();
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
                let from_fqn = build_class_fqn(class_name, &package);
                let to_symbol = source[extends_cap.node.byte_range()].to_string();
                let line = Some(class_cap.node.start_position().row as u32 + 1);
                relationships.push(Relationship {
                    from_symbol:  from_fqn,
                    to_symbol,
                    kind:         RelationshipKind::Extends,
                    file:         relative_path.to_string(),
                    line,
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
                let from_fqn = build_class_fqn(class_name, &package);
                let to_symbol = source[iface_cap.node.byte_range()].to_string();
                let line = Some(class_cap.node.start_position().row as u32 + 1);
                relationships.push(Relationship {
                    from_symbol:  from_fqn,
                    to_symbol,
                    kind:         RelationshipKind::Implements,
                    file:         relative_path.to_string(),
                    line,
                    workspace_id: workspace_id.to_string(),
                });
                continue;
            }

            // CALLS
            if let (Some(call_cap), Some(callee_cap)) = (
                m.captures.iter().find(|c| c.index == self.rel_cap_call_site),
                m.captures.iter().find(|c| c.index == self.rel_cap_callee),
            ) {
                let to_symbol = source[callee_cap.node.byte_range()].to_string();
                let from_symbol = method_fqn_for_node(source, call_cap.node, &package)
                    .unwrap_or_else(|| relative_path.to_string());
                let line = Some(call_cap.node.start_position().row as u32 + 1);
                relationships.push(Relationship {
                    from_symbol,
                    to_symbol,
                    kind:         RelationshipKind::Calls,
                    file:         relative_path.to_string(),
                    line,
                    workspace_id: workspace_id.to_string(),
                });
            }
        }

        Ok(ParseOutput { symbols, relationships })
    }
}
