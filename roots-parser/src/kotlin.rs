use streaming_iterator::StreamingIterator;
use tree_sitter::{Language as TsLanguage, Node, Parser, Query, QueryCursor, Tree};

use roots_core::{Language, Relationship, RelationshipKind, Symbol, SymbolKind};

use crate::extractor::{LanguageExtractor, ParseError, ParseOutput};

// Kotlin does not use named fields for declaration names (positional children).
// interface/enum class are both class_declaration with modifier children — emit all as Class for V1.
const KOTLIN_SYMBOL_QUERY: &str = r#"
(class_declaration    (type_identifier)   @name) @class
(object_declaration   (type_identifier)   @name) @object
(function_declaration (simple_identifier) @name) @function
"#;

// Delegation specifiers come in two shapes:
//   class A : B()              → delegation_specifier → constructor_invocation → user_type
//   class A : UserRepository   → delegation_specifier → user_type
// Both are captured with separate patterns.
const KOTLIN_REL_QUERY: &str = r#"
(import_header (identifier) @import_path)

(class_declaration
  (type_identifier) @ctor_class
  (delegation_specifier
    (constructor_invocation
      (user_type (type_identifier) @ctor_super))))

(class_declaration
  (type_identifier) @type_class
  (delegation_specifier
    (user_type (type_identifier) @type_super)))

(call_expression
  (simple_identifier) @free_callee) @free_call

(call_expression
  (navigation_expression
    (navigation_suffix
      (simple_identifier) @nav_callee))) @nav_call
"#;

pub struct KotlinExtractor {
    ts_language: TsLanguage,
    sym_query: Query,
    rel_query: Query,
    cap_name:     u32,
    cap_class:    u32,
    cap_object:   u32,
    cap_function: u32,
    // relationship captures
    rel_cap_import:     u32,
    rel_cap_ctor_class: u32,
    rel_cap_ctor_super: u32,
    rel_cap_type_class: u32,
    rel_cap_type_super: u32,
    rel_cap_free_callee: u32,
    rel_cap_free_call:   u32,
    rel_cap_nav_callee:  u32,
    rel_cap_nav_call:    u32,
}

impl KotlinExtractor {
    pub fn new() -> Result<Self, ParseError> {
        let ts_language = TsLanguage::from(tree_sitter_kotlin::LANGUAGE);

        let sym_query = Query::new(&ts_language, KOTLIN_SYMBOL_QUERY)
            .map_err(|e| ParseError::QueryCompile(e.to_string()))?;
        let cap_name     = sym_query.capture_index_for_name("name").unwrap();
        let cap_class    = sym_query.capture_index_for_name("class").unwrap();
        let cap_object   = sym_query.capture_index_for_name("object").unwrap();
        let cap_function = sym_query.capture_index_for_name("function").unwrap();

        let rel_query = Query::new(&ts_language, KOTLIN_REL_QUERY)
            .map_err(|e| ParseError::QueryCompile(format!("kotlin rel query: {e}")))?;
        let rel_cap_import      = rel_query.capture_index_for_name("import_path").unwrap();
        let rel_cap_ctor_class  = rel_query.capture_index_for_name("ctor_class").unwrap();
        let rel_cap_ctor_super  = rel_query.capture_index_for_name("ctor_super").unwrap();
        let rel_cap_type_class  = rel_query.capture_index_for_name("type_class").unwrap();
        let rel_cap_type_super  = rel_query.capture_index_for_name("type_super").unwrap();
        let rel_cap_free_callee = rel_query.capture_index_for_name("free_callee").unwrap();
        let rel_cap_free_call   = rel_query.capture_index_for_name("free_call").unwrap();
        let rel_cap_nav_callee  = rel_query.capture_index_for_name("nav_callee").unwrap();
        let rel_cap_nav_call    = rel_query.capture_index_for_name("nav_call").unwrap();

        Ok(Self {
            ts_language, sym_query, rel_query,
            cap_name, cap_class, cap_object, cap_function,
            rel_cap_import, rel_cap_ctor_class, rel_cap_ctor_super,
            rel_cap_type_class, rel_cap_type_super,
            rel_cap_free_callee, rel_cap_free_call,
            rel_cap_nav_callee, rel_cap_nav_call,
        })
    }
}

/// Walk up from the name node to find the enclosing function declaration
/// and extract its `function_value_parameters` text.
fn kt_extract_params<'a>(source: &str, name_node: Node<'a>) -> Option<String> {
    let mut cur = name_node.parent();
    while let Some(n) = cur {
        if n.kind() == "function_declaration" {
            for i in 0..n.child_count() {
                if let Some(child) = n.child(i) {
                    if child.kind() == "function_value_parameters" {
                        return Some(source[child.byte_range()].to_string());
                    }
                }
            }
        }
        cur = n.parent();
    }
    None
}

fn extract_kt_package(source: &str, tree: &Tree) -> String {
    let root = tree.root_node();
    for i in 0..root.child_count() {
        if let Some(child) = root.child(i) {
            if child.kind() == "package_header" {
                let text = source[child.byte_range()].trim();
                let without_kw = text.strip_prefix("package").unwrap_or(text).trim();
                return without_kw.to_string();
            }
        }
    }
    String::new()
}

fn kt_build_fqn(name: &str, package: &str) -> String {
    if package.is_empty() {
        name.to_string()
    } else {
        format!("{}.{}", package, name)
    }
}

fn kt_method_fqn<'a>(source: &str, node: Node<'a>, package: &str) -> Option<String> {
    let mut cur = Some(node);
    while let Some(n) = cur {
        if n.kind() == "function_declaration" {
            // Find first simple_identifier child = function name
            for i in 0..n.child_count() {
                if let Some(c) = n.child(i) {
                    if c.kind() == "simple_identifier" {
                        let fn_name = &source[c.byte_range()];
                        let class_fqn = kt_class_fqn(source, n, package);
                        return Some(if class_fqn.is_empty() {
                            kt_build_fqn(fn_name, package)
                        } else {
                            format!("{}.{}", class_fqn, fn_name)
                        });
                    }
                }
            }
        }
        cur = n.parent();
    }
    None
}

fn kt_class_fqn<'a>(source: &str, node: Node<'a>, package: &str) -> String {
    let mut cur = Some(node);
    while let Some(n) = cur {
        if n.kind() == "class_declaration" {
            for i in 0..n.child_count() {
                if let Some(c) = n.child(i) {
                    if c.kind() == "type_identifier" {
                        let name = &source[c.byte_range()];
                        return kt_build_fqn(name, package);
                    }
                }
            }
        }
        cur = n.parent();
    }
    String::new()
}

impl LanguageExtractor for KotlinExtractor {
    fn extract(&self, source: &str, relative_path: &str, project: &str, workspace_id: &str) -> Result<ParseOutput, ParseError> {
        let mut parser = Parser::new();
        parser.set_language(&self.ts_language)
            .map_err(|_| ParseError::ParserInit { language: "kotlin".into() })?;

        let tree = parser.parse(source, None)
            .ok_or_else(|| ParseError::ParserInit { language: "kotlin".into() })?;

        let package = extract_kt_package(source, &tree);

        // --- Symbol extraction ---
        let mut cursor = QueryCursor::new();
        let mut symbols = Vec::new();

        let mut matches = cursor.matches(&self.sym_query, tree.root_node(), source.as_bytes());
        while let Some(m) = matches.next() {
            let kind = if m.captures.iter().any(|c| c.index == self.cap_class) {
                SymbolKind::Class
            } else if m.captures.iter().any(|c| c.index == self.cap_object) {
                SymbolKind::Class
            } else if m.captures.iter().any(|c| c.index == self.cap_function) {
                SymbolKind::Function
            } else {
                continue;
            };

            if let Some(name_cap) = m.captures.iter().find(|c| c.index == self.cap_name) {
                let node = name_cap.node;
                let name = source[node.byte_range()].to_string();
                let fqn = match kind {
                    SymbolKind::Function => {
                        kt_method_fqn(source, node, &package)
                            .unwrap_or_else(|| kt_build_fqn(&name, &package))
                    }
                    _ => kt_build_fqn(&name, &package),
                };
                let signature = match kind {
                    SymbolKind::Function => kt_extract_params(source, node),
                    _ => None,
                };
                symbols.push(Symbol {
                    name,
                    kind,
                    file:         relative_path.to_string(),
                    language:     Language::Kotlin,
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

            // EXTENDS via constructor invocation: class A : B()
            if let (Some(class_cap), Some(super_cap)) = (
                m.captures.iter().find(|c| c.index == self.rel_cap_ctor_class),
                m.captures.iter().find(|c| c.index == self.rel_cap_ctor_super),
            ) {
                let class_name = &source[class_cap.node.byte_range()];
                relationships.push(Relationship {
                    from_symbol:  kt_build_fqn(class_name, &package),
                    to_symbol:    source[super_cap.node.byte_range()].to_string(),
                    kind:         RelationshipKind::Extends,
                    file:         relative_path.to_string(),
                    line:         Some(class_cap.node.start_position().row as u32 + 1),
                    workspace_id: workspace_id.to_string(),
                });
                continue;
            }

            // EXTENDS/IMPLEMENTS via plain user_type: class A : B
            // Kotlin doesn't distinguish extends vs implements at syntax level; use Extends for all.
            if let (Some(class_cap), Some(super_cap)) = (
                m.captures.iter().find(|c| c.index == self.rel_cap_type_class),
                m.captures.iter().find(|c| c.index == self.rel_cap_type_super),
            ) {
                let class_name = &source[class_cap.node.byte_range()];
                relationships.push(Relationship {
                    from_symbol:  kt_build_fqn(class_name, &package),
                    to_symbol:    source[super_cap.node.byte_range()].to_string(),
                    kind:         RelationshipKind::Extends,
                    file:         relative_path.to_string(),
                    line:         Some(class_cap.node.start_position().row as u32 + 1),
                    workspace_id: workspace_id.to_string(),
                });
                continue;
            }

            // CALLS — free call
            if let (Some(call_cap), Some(callee_cap)) = (
                m.captures.iter().find(|c| c.index == self.rel_cap_free_call),
                m.captures.iter().find(|c| c.index == self.rel_cap_free_callee),
            ) {
                let to_symbol = source[callee_cap.node.byte_range()].to_string();
                let from_symbol = kt_method_fqn(source, call_cap.node, &package)
                    .unwrap_or_else(|| relative_path.to_string());
                relationships.push(Relationship {
                    from_symbol,
                    to_symbol,
                    kind:         RelationshipKind::Calls,
                    file:         relative_path.to_string(),
                    line:         Some(call_cap.node.start_position().row as u32 + 1),
                    workspace_id: workspace_id.to_string(),
                });
                continue;
            }

            // CALLS — navigation call (obj.method())
            if let (Some(call_cap), Some(callee_cap)) = (
                m.captures.iter().find(|c| c.index == self.rel_cap_nav_call),
                m.captures.iter().find(|c| c.index == self.rel_cap_nav_callee),
            ) {
                let to_symbol = source[callee_cap.node.byte_range()].to_string();
                let from_symbol = kt_method_fqn(source, call_cap.node, &package)
                    .unwrap_or_else(|| relative_path.to_string());
                relationships.push(Relationship {
                    from_symbol,
                    to_symbol,
                    kind:         RelationshipKind::Calls,
                    file:         relative_path.to_string(),
                    line:         Some(call_cap.node.start_position().row as u32 + 1),
                    workspace_id: workspace_id.to_string(),
                });
            }
        }

        Ok(ParseOutput { symbols, relationships })
    }
}
