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

(lexical_declaration
  (variable_declarator
    name: (identifier) @name
    value: [(arrow_function) (function_expression)])) @arrow_var

(variable_declaration
  (variable_declarator
    name: (identifier) @name
    value: [(arrow_function) (function_expression)])) @arrow_var
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
    cap_arrow_var: u32,
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
        let cap_arrow_var = sym_query.capture_index_for_name("arrow_var").unwrap();

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
            cap_name, cap_class, cap_interface, cap_enum, cap_function, cap_method, cap_arrow_var,
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
            } else if m.captures.iter().any(|c| c.index == self.cap_arrow_var) {
                SymbolKind::Function
            } else {
                continue;
            };

            if let Some(name_cap) = m.captures.iter().find(|c| c.index == self.cap_name) {
                let node = name_cap.node;
                let name = source[node.byte_range()].to_string();
                let fqn = ts_symbol_fqn(source, node, relative_path, &name, kind == SymbolKind::Method);
                let signature = match kind {
                    SymbolKind::Function | SymbolKind::Method => ts_extract_params(source, node),
                    _ => None,
                };
                symbols.push(Symbol {
                    name,
                    kind,
                    file:         relative_path.to_string(),
                    language:     language.clone(),
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

/// Walk up from the name node to find the enclosing function/method/arrow declaration
/// and extract its `formal_parameters` text.
fn ts_extract_params<'a>(source: &str, name_node: Node<'a>) -> Option<String> {
    let mut cur = name_node.parent();
    while let Some(n) = cur {
        if matches!(n.kind(), "function_declaration" | "method_definition" | "arrow_function" | "function_expression") {
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

/// One call site found on the variable constructed via `new <class_name>(...)` in a test file
/// — e.g. `service.registerProduct(productData)`. Used to ground stub generation in what the
/// already-written test ACTUALLY calls, instead of asking the model to re-derive the same fact
/// by reading the test under time pressure — the exact self-check that's already been observed,
/// on a real dogfooding run, to be wrong about half the time even when correctly worded.
pub struct ObservedCall {
    pub method_name: String,
    /// Raw source text of each argument expression, in call order.
    pub argument_texts: Vec<String>,
    pub line: u32,
}

/// Parses `source` as TypeScript and finds every call site on the variable assigned from
/// `new <class_name>(...)`. Returns one `ObservedCall` per call site found, in source order.
/// Returns an empty Vec if no such `new` expression is found, or its target isn't a plain
/// identifier (destructuring targets etc. aren't handled) — callers should treat an empty
/// result as "couldn't determine this deterministically," not as "no calls exist."
pub fn find_subject_calls(source: &str, class_name: &str) -> Vec<ObservedCall> {
    let ts_language = TsLanguage::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT);
    let mut parser = Parser::new();
    if parser.set_language(&ts_language).is_err() {
        return Vec::new();
    }
    let Some(tree) = parser.parse(source, None) else { return Vec::new() };

    let Some(subject_var) = find_new_expression_target(tree.root_node(), source, class_name) else {
        return Vec::new();
    };

    let mut calls = Vec::new();
    collect_calls_on(tree.root_node(), source, &subject_var, &mut calls);
    calls
}

/// Walks the tree looking for `<var> = new <class_name>(...)`, as either a `variable_declarator`
/// initializer (`let subject = new Foo()`) or a plain assignment (`subject = new Foo()`), and
/// returns the variable's identifier text.
fn find_new_expression_target(node: Node, source: &str, class_name: &str) -> Option<String> {
    if node.kind() == "variable_declarator" {
        if let (Some(name_node), Some(value_node)) =
            (node.child_by_field_name("name"), node.child_by_field_name("value"))
        {
            if name_node.kind() == "identifier" && is_new_expression_for(value_node, source, class_name) {
                return Some(source[name_node.byte_range()].to_string());
            }
        }
    }
    if node.kind() == "assignment_expression" {
        if let (Some(left), Some(right)) =
            (node.child_by_field_name("left"), node.child_by_field_name("right"))
        {
            if left.kind() == "identifier" && is_new_expression_for(right, source, class_name) {
                return Some(source[left.byte_range()].to_string());
            }
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(found) = find_new_expression_target(child, source, class_name) {
            return Some(found);
        }
    }
    None
}

fn is_new_expression_for(node: Node, source: &str, class_name: &str) -> bool {
    node.kind() == "new_expression"
        && node.child_by_field_name("constructor")
            .map(|c| &source[c.byte_range()] == class_name)
            .unwrap_or(false)
}

/// Walks the tree collecting every `<subject_var>.<method>(...)` call expression.
fn collect_calls_on(node: Node, source: &str, subject_var: &str, out: &mut Vec<ObservedCall>) {
    if node.kind() == "call_expression" {
        if let Some(function_node) = node.child_by_field_name("function") {
            if function_node.kind() == "member_expression" {
                let object = function_node.child_by_field_name("object");
                let property = function_node.child_by_field_name("property");
                if let (Some(object), Some(property)) = (object, property) {
                    if object.kind() == "identifier" && &source[object.byte_range()] == subject_var {
                        if let Some(args_node) = node.child_by_field_name("arguments") {
                            if args_node.kind() == "arguments" {
                                let mut arg_cursor = args_node.walk();
                                let argument_texts = args_node.named_children(&mut arg_cursor)
                                    .map(|a| source[a.byte_range()].to_string())
                                    .collect();
                                out.push(ObservedCall {
                                    method_name: source[property.byte_range()].to_string(),
                                    argument_texts,
                                    line: node.start_position().row as u32 + 1,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_calls_on(child, source, subject_var, out);
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extractor::LanguageExtractor;

    fn extractor() -> TypeScriptExtractor {
        TypeScriptExtractor::new().expect("extractor init")
    }

    #[test]
    fn extracts_arrow_function_component() {
        let source = r#"
import React, { useState } from 'react';

interface ProductFormProps {
  onSubmit: (data: any) => void;
}

const ProductForm: React.FC<ProductFormProps> = ({ onSubmit }) => {
  const [name, setName] = useState('');
  return <form />;
};

export default ProductForm;
"#;
        let out = extractor().extract(source, "src/components/ProductForm.tsx", "project", "ws")
            .expect("parse ok");
        let names: Vec<&str> = out.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"ProductForm"), "should extract arrow component: got {names:?}");
        let sym = out.symbols.iter().find(|s| s.name == "ProductForm").unwrap();
        assert_eq!(sym.kind, SymbolKind::Function);
    }

    #[test]
    fn extracts_plain_arrow_function() {
        let source = r#"
export const registerProduct = async (data: any): Promise<void> => {
  await fetch('/products', { method: 'POST', body: JSON.stringify(data) });
};
"#;
        let out = extractor().extract(source, "src/api/ProductApi.ts", "project", "ws")
            .expect("parse ok");
        let names: Vec<&str> = out.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"registerProduct"), "should extract arrow fn: got {names:?}");
    }

    #[test]
    fn still_extracts_interface() {
        let source = r#"
interface ProductFormProps {
  onSubmit: (data: any) => void;
  errorMessages: Record<string, string>;
}
"#;
        let out = extractor().extract(source, "src/types.ts", "project", "ws")
            .expect("parse ok");
        let names: Vec<&str> = out.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"ProductFormProps"), "should extract interface: got {names:?}");
        let sym = out.symbols.iter().find(|s| s.name == "ProductFormProps").unwrap();
        assert_eq!(sym.kind, SymbolKind::Interface);
    }

    #[test]
    fn finds_single_object_call_on_subject() {
        // Mirrors the actual ProductService.test.ts observed on a real dogfooding run: the
        // test correctly calls with a single object argument, but the stub kept declaring
        // 5 positional parameters instead — this is the fact that should have stopped it.
        let source = r#"
let service: ProductService

beforeEach(() => {
  service = new ProductService(mockRepo, mockPublisher)
})

it('registers a product', async () => {
  const result = await service.registerProduct(productData)
  expect(result.id).toEqual(expect.any(String))
})
"#;
        let calls = find_subject_calls(source, "ProductService");
        assert_eq!(calls.len(), 1, "should find exactly one call site: got {:?}",
            calls.iter().map(|c| &c.method_name).collect::<Vec<_>>());
        assert_eq!(calls[0].method_name, "registerProduct");
        assert_eq!(calls[0].argument_texts, vec!["productData".to_string()]);
    }

    #[test]
    fn finds_multiple_consistent_call_sites() {
        let source = r#"
let subject: ProductRepository

beforeEach(() => {
  subject = new ProductRepository(mockPool)
})

it('saves', async () => {
  await subject.saveProduct(product)
})

it('saves with description', async () => {
  await subject.saveProduct(productWithDescription)
})
"#;
        let calls = find_subject_calls(source, "ProductRepository");
        assert_eq!(calls.len(), 2);
        assert!(calls.iter().all(|c| c.method_name == "saveProduct" && c.argument_texts.len() == 1));
    }

    #[test]
    fn returns_empty_when_class_not_constructed() {
        let source = r#"
it('does something unrelated', () => {
  const x = someOtherThing()
})
"#;
        let calls = find_subject_calls(source, "ProductService");
        assert!(calls.is_empty());
    }

    #[test]
    fn ignores_calls_on_other_variables() {
        let source = r#"
let service = new ProductService(mockRepo, mockPublisher)
let logger = new Logger()

it('logs', () => {
  logger.info('hello', 'world', 'extra')
})
"#;
        let calls = find_subject_calls(source, "ProductService");
        assert!(calls.is_empty(), "should not pick up calls on unrelated variables: {:?}",
            calls.iter().map(|c| &c.method_name).collect::<Vec<_>>());
    }
}
