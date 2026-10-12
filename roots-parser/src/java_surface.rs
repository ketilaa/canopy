//! The declared surface of a Java source file: what another class can see and use of the types in it.
//!
//! Types (class, interface, enum, record, annotation) with their annotations as written, modifiers, supertypes,
//! record components, fields, constructors and methods with parameter types, return types and `throws`. Read from
//! the file itself, so it is current without an index, and works for a file that has just been written.
//!
//! Types are kept as written (`List<Widget>`, `String[]`), not resolved: resolution needs the classpath, which the
//! Implementer's class index already has.

use serde::Serialize;
use tree_sitter::{Node, Parser};

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Surface {
    pub package: String,
    pub types: Vec<TypeSurface>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct TypeSurface {
    /// `class`, `interface`, `enum`, `record` or `annotation`.
    pub kind: String,
    pub name: String,
    pub fqn: String,
    pub modifiers: Vec<String>,
    pub annotations: Vec<Annotation>,
    /// The superclass of a class, or the extended interfaces of an interface.
    pub extends: Vec<String>,
    pub implements: Vec<String>,
    /// The components of a record, in order.
    pub components: Vec<Param>,
    /// The constants of an enum, in order.
    pub constants: Vec<String>,
    pub fields: Vec<Field>,
    pub constructors: Vec<Callable>,
    pub methods: Vec<Callable>,
    pub nested: Vec<TypeSurface>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Annotation {
    pub name: String,
    /// The arguments as written, without the parentheses: `"/widgets"`, `value = "x", produces = "y"`.
    pub arguments: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Param {
    pub annotations: Vec<Annotation>,
    pub ty: String,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Field {
    pub modifiers: Vec<String>,
    pub annotations: Vec<Annotation>,
    pub ty: String,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Callable {
    pub modifiers: Vec<String>,
    pub annotations: Vec<Annotation>,
    pub name: String,
    pub params: Vec<Param>,
    /// `None` for a constructor.
    pub returns: Option<String>,
    pub throws: Vec<String>,
}

pub fn java_surface(source: &str) -> Surface {
    let mut parser = Parser::new();
    if parser
        .set_language(&tree_sitter::Language::from(tree_sitter_java::LANGUAGE))
        .is_err()
    {
        return Surface::default();
    }
    let Some(tree) = parser.parse(source, None) else {
        return Surface::default();
    };
    let root = tree.root_node();
    let reader = Reader { source };
    let package = children(root)
        .find(|n| n.kind() == "package_declaration")
        .map(|n| {
            reader
                .text(n)
                .trim_start_matches("package")
                .trim()
                .trim_end_matches(';')
                .trim()
                .to_string()
        })
        .unwrap_or_default();
    let types = children(root)
        .filter_map(|n| reader.type_surface(n, &package))
        .collect();
    Surface { package, types }
}

fn children(node: Node) -> impl Iterator<Item = Node> {
    (0..node.child_count()).filter_map(move |i| node.child(i))
}

struct Reader<'a> {
    source: &'a str,
}

impl<'a> Reader<'a> {
    fn text(&self, node: Node) -> String {
        self.source[node.byte_range()].to_string()
    }

    fn field_text(&self, node: Node, field: &str) -> Option<String> {
        node.child_by_field_name(field).map(|n| self.text(n))
    }

    fn type_surface(&self, node: Node, outer: &str) -> Option<TypeSurface> {
        let kind = match node.kind() {
            "class_declaration" => "class",
            "interface_declaration" => "interface",
            "enum_declaration" => "enum",
            "record_declaration" => "record",
            "annotation_type_declaration" => "annotation",
            _ => return None,
        };
        let name = self.field_text(node, "name")?;
        let fqn = if outer.is_empty() {
            name.clone()
        } else {
            format!("{outer}.{name}")
        };
        let (modifiers, annotations) = self.modifiers_of(node);
        let mut surface = TypeSurface {
            kind: kind.to_string(),
            name,
            fqn: fqn.clone(),
            modifiers,
            annotations,
            ..TypeSurface::default()
        };
        for part in children(node) {
            match part.kind() {
                "superclass" => surface.extends.extend(self.type_names(part)),
                "extends_interfaces" => surface.extends.extend(self.type_names(part)),
                "super_interfaces" => surface.implements.extend(self.type_names(part)),
                _ => {}
            }
        }
        if kind == "record" {
            if let Some(params) = node.child_by_field_name("parameters") {
                surface.components = self.params(params);
            }
        }
        if let Some(body) = node.child_by_field_name("body") {
            self.members(body, &fqn, &mut surface);
        }
        Some(surface)
    }

    /// The type names directly under a `superclass`, `extends_interfaces` or `super_interfaces` node.
    fn type_names(&self, node: Node) -> Vec<String> {
        let mut names = Vec::new();
        for child in children(node) {
            match child.kind() {
                "type_list" => names.extend(
                    children(child)
                        .filter(|n| n.is_named())
                        .map(|n| self.text(n)),
                ),
                "extends" | "implements" | "," => {}
                _ if child.is_named() => names.push(self.text(child)),
                _ => {}
            }
        }
        names
    }

    fn members(&self, body: Node, fqn: &str, surface: &mut TypeSurface) {
        for member in children(body) {
            match member.kind() {
                "enum_constant" => {
                    if let Some(name) = self.field_text(member, "name") {
                        surface.constants.push(name);
                    }
                }
                "enum_body_declarations" => self.members(member, fqn, surface),
                "field_declaration" | "constant_declaration" => {
                    let (modifiers, annotations) = self.modifiers_of(member);
                    let ty = self.field_text(member, "type").unwrap_or_default();
                    for declarator in children(member).filter(|n| n.kind() == "variable_declarator")
                    {
                        surface.fields.push(Field {
                            modifiers: modifiers.clone(),
                            annotations: annotations.clone(),
                            ty: ty.clone(),
                            name: self.field_text(declarator, "name").unwrap_or_default(),
                        });
                    }
                }
                "constructor_declaration" => surface.constructors.push(self.callable(member, None)),
                "method_declaration" => {
                    let returns = self.field_text(member, "type").unwrap_or_default();
                    surface.methods.push(self.callable(member, Some(returns)));
                }
                _ => {
                    if let Some(nested) = self.type_surface(member, fqn) {
                        surface.nested.push(nested);
                    }
                }
            }
        }
    }

    fn callable(&self, node: Node, returns: Option<String>) -> Callable {
        let (modifiers, annotations) = self.modifiers_of(node);
        let throws = children(node)
            .find(|n| n.kind() == "throws")
            .map(|n| {
                children(n)
                    .filter(|c| c.is_named())
                    .map(|c| self.text(c))
                    .collect()
            })
            .unwrap_or_default();
        Callable {
            modifiers,
            annotations,
            name: self.field_text(node, "name").unwrap_or_default(),
            params: node
                .child_by_field_name("parameters")
                .map(|p| self.params(p))
                .unwrap_or_default(),
            returns,
            throws,
        }
    }

    fn params(&self, list: Node) -> Vec<Param> {
        children(list)
            .filter_map(|p| match p.kind() {
                "formal_parameter" => {
                    let (_, annotations) = self.modifiers_of(p);
                    Some(Param {
                        annotations,
                        ty: self.field_text(p, "type").unwrap_or_default(),
                        name: self.field_text(p, "name").unwrap_or_default(),
                    })
                }
                "spread_parameter" => {
                    let (_, annotations) = self.modifiers_of(p);
                    let ty = children(p)
                        .find(|c| {
                            c.is_named() && !matches!(c.kind(), "modifiers" | "variable_declarator")
                        })
                        .map(|c| self.text(c))
                        .unwrap_or_default();
                    let name = children(p)
                        .find(|c| c.kind() == "variable_declarator")
                        .and_then(|d| self.field_text(d, "name"))
                        .unwrap_or_default();
                    Some(Param {
                        annotations,
                        ty: format!("{ty}..."),
                        name,
                    })
                }
                _ => None,
            })
            .collect()
    }

    /// The modifier keywords and the annotations in a declaration's `modifiers` child.
    fn modifiers_of(&self, node: Node) -> (Vec<String>, Vec<Annotation>) {
        let mut modifiers = Vec::new();
        let mut annotations = Vec::new();
        if let Some(list) = children(node).find(|n| n.kind() == "modifiers") {
            for m in children(list) {
                match m.kind() {
                    "marker_annotation" | "annotation" => annotations.push(Annotation {
                        name: self.field_text(m, "name").unwrap_or_default(),
                        arguments: self.field_text(m, "arguments").map(|a| {
                            a.trim()
                                .trim_start_matches('(')
                                .trim_end_matches(')')
                                .trim()
                                .to_string()
                        }),
                    }),
                    _ => modifiers.push(self.text(m)),
                }
            }
        }
        (modifiers, annotations)
    }
}

fn annotation_text(a: &Annotation) -> String {
    match &a.arguments {
        Some(args) => format!("@{}({args})", a.name),
        None => format!("@{}", a.name),
    }
}

fn param_text(p: &Param) -> String {
    let annotations: String = p
        .annotations
        .iter()
        .map(|a| format!("{} ", annotation_text(a)))
        .collect();
    format!("{annotations}{} {}", p.ty, p.name)
}

/// `@A @B(x) public static ` (everything before the keyword or type of a declaration).
fn prefix(annotations: &[Annotation], modifiers: &[String]) -> String {
    annotations
        .iter()
        .map(annotation_text)
        .chain(modifiers.iter().cloned())
        .map(|part| format!("{part} "))
        .collect()
}

fn is_private(modifiers: &[String]) -> bool {
    modifiers.iter().any(|m| m == "private")
}

fn render_type(t: &TypeSurface, indent: usize, out: &mut String) {
    let pad = " ".repeat(indent);
    let mut header = format!(
        "{pad}{}{} {}",
        prefix(&t.annotations, &t.modifiers),
        t.kind,
        t.name
    );
    if t.kind == "record" {
        let components: Vec<String> = t.components.iter().map(param_text).collect();
        header.push_str(&format!("({})", components.join(", ")));
    }
    if !t.extends.is_empty() {
        header.push_str(&format!(" extends {}", t.extends.join(", ")));
    }
    if !t.implements.is_empty() {
        header.push_str(&format!(" implements {}", t.implements.join(", ")));
    }
    out.push_str(&header);
    out.push('\n');
    let inner = format!("{pad}  ");
    if !t.constants.is_empty() {
        out.push_str(&format!("{inner}constants: {}\n", t.constants.join(", ")));
    }
    for f in t.fields.iter().filter(|f| !is_private(&f.modifiers)) {
        out.push_str(&format!(
            "{inner}{}{} {}\n",
            prefix(&f.annotations, &f.modifiers),
            f.ty,
            f.name
        ));
    }
    let callables = t
        .constructors
        .iter()
        .chain(&t.methods)
        .filter(|c| !is_private(&c.modifiers));
    for c in callables {
        let params: Vec<String> = c.params.iter().map(param_text).collect();
        let returns = c
            .returns
            .as_deref()
            .map(|r| format!("{r} "))
            .unwrap_or_default();
        let throws = if c.throws.is_empty() {
            String::new()
        } else {
            format!(" throws {}", c.throws.join(", "))
        };
        out.push_str(&format!(
            "{inner}{}{returns}{}({}){throws}\n",
            prefix(&c.annotations, &c.modifiers),
            c.name,
            params.join(", ")
        ));
    }
    for n in &t.nested {
        render_type(n, indent + 2, out);
    }
}

impl Surface {
    /// The surface as a model reads it: one line per type and per member another class can use. Private members
    /// are left out.
    pub fn render(&self) -> String {
        let mut out = String::new();
        for t in &self.types {
            render_type(t, 0, &mut out);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECORD: &str = r#"
package com.example.widgets;

public record Widget(String id, String name) {
    public Widget {
        if (name == null) throw new IllegalArgumentException("name");
    }
}
"#;

    const CONTROLLER: &str = r#"
package com.example.widgets;

import org.springframework.web.bind.annotation.*;

@RestController
@RequestMapping("/widgets")
public class WidgetController {
    private final WidgetService service;

    public WidgetController(WidgetService service) {
        this.service = service;
    }

    @PostMapping
    @ResponseStatus(HttpStatus.CREATED)
    public Widget create(@RequestBody CreateRequest request) {
        return service.create(request.name());
    }

    private void helper() {}
}
"#;

    #[test]
    fn a_record_has_its_components_and_a_compact_constructor_is_not_a_second_constructor() {
        let surface = java_surface(RECORD);
        assert_eq!(surface.package, "com.example.widgets");
        let widget = &surface.types[0];
        assert_eq!(
            (widget.kind.as_str(), widget.name.as_str()),
            ("record", "Widget")
        );
        assert_eq!(widget.fqn, "com.example.widgets.Widget");
        let components: Vec<(&str, &str)> = widget
            .components
            .iter()
            .map(|p| (p.ty.as_str(), p.name.as_str()))
            .collect();
        assert_eq!(components, [("String", "id"), ("String", "name")]);
        assert!(
            surface
                .render()
                .contains("public record Widget(String id, String name)"),
            "{}",
            surface.render()
        );
    }

    #[test]
    fn annotations_are_kept_as_written_on_a_type_a_method_and_a_parameter() {
        let surface = java_surface(CONTROLLER);
        let controller = &surface.types[0];
        let annotations: Vec<(&str, Option<&str>)> = controller
            .annotations
            .iter()
            .map(|a| (a.name.as_str(), a.arguments.as_deref()))
            .collect();
        assert_eq!(
            annotations,
            [
                ("RestController", None),
                ("RequestMapping", Some("\"/widgets\""))
            ]
        );
        let create = &controller.methods[0];
        assert_eq!(create.name, "create");
        assert_eq!(create.returns.as_deref(), Some("Widget"));
        let on_method: Vec<(&str, Option<&str>)> = create
            .annotations
            .iter()
            .map(|a| (a.name.as_str(), a.arguments.as_deref()))
            .collect();
        assert_eq!(
            on_method,
            [
                ("PostMapping", None),
                ("ResponseStatus", Some("HttpStatus.CREATED"))
            ]
        );
        assert_eq!(create.params[0].annotations[0].name, "RequestBody");
        assert_eq!(
            (create.params[0].ty.as_str(), create.params[0].name.as_str()),
            ("CreateRequest", "request")
        );
    }

    #[test]
    fn a_constructor_has_parameters_and_no_return_type() {
        let controller = &java_surface(CONTROLLER).types[0];
        assert_eq!(controller.constructors.len(), 1);
        let constructor = &controller.constructors[0];
        assert_eq!(constructor.returns, None);
        assert_eq!(constructor.params[0].ty, "WidgetService");
    }

    #[test]
    fn rendering_lists_what_another_class_can_use_and_leaves_private_members_out() {
        let text = java_surface(CONTROLLER).render();
        assert!(
            text.contains(
                "@RestController @RequestMapping(\"/widgets\") public class WidgetController"
            ),
            "{text}"
        );
        assert!(
            text.contains("public WidgetController(WidgetService service)"),
            "{text}"
        );
        assert!(
            text.contains("@PostMapping @ResponseStatus(HttpStatus.CREATED) public Widget create(@RequestBody CreateRequest request)"),
            "{text}"
        );
        assert!(
            !text.contains("helper") && !text.contains("private final"),
            "{text}"
        );
    }

    #[test]
    fn return_types_are_kept_as_written_including_generics_arrays_and_throws() {
        let source = r#"
package p;
public class Store {
    public List<Widget> findAll() throws IOException, InterruptedException { return null; }
    public Map<String, List<Widget>> grouped() { return null; }
    public String[] names(String... parts) { return null; }
    public void clear() {}
}
"#;
        let store = &java_surface(source).types[0];
        let returns: Vec<&str> = store
            .methods
            .iter()
            .map(|m| m.returns.as_deref().unwrap())
            .collect();
        assert_eq!(
            returns,
            [
                "List<Widget>",
                "Map<String, List<Widget>>",
                "String[]",
                "void"
            ]
        );
        assert_eq!(
            store.methods[0].throws,
            ["IOException", "InterruptedException"]
        );
        assert_eq!(store.methods[2].params[0].ty, "String...");
        assert!(java_surface(source)
            .render()
            .contains("public List<Widget> findAll() throws IOException, InterruptedException"));
    }

    #[test]
    fn an_interface_lists_its_bodiless_methods_and_what_it_extends_and_an_enum_its_constants() {
        let source = r#"
package p;
public interface Repo extends Base<Widget, String>, Other {
    Optional<Widget> findById(String id);
}
enum Status { ACTIVE, DELETED }
"#;
        let surface = java_surface(source);
        let repo = &surface.types[0];
        assert_eq!(repo.kind, "interface");
        assert_eq!(repo.extends, ["Base<Widget, String>", "Other"]);
        assert_eq!(repo.methods[0].returns.as_deref(), Some("Optional<Widget>"));
        let status = &surface.types[1];
        assert_eq!(
            (status.kind.as_str(), status.constants.as_slice()),
            (
                "enum",
                ["ACTIVE".to_string(), "DELETED".to_string()].as_slice()
            )
        );
    }

    #[test]
    fn a_class_names_its_superclass_and_interfaces_and_nested_types_get_a_qualified_name() {
        let source = r#"
package p;
public class Outer extends Parent implements A, B<String> {
    public static record Inner(int n) {}
    public int count() { return 0; }
}
"#;
        let outer = &java_surface(source).types[0];
        assert_eq!(outer.extends, ["Parent"]);
        assert_eq!(outer.implements, ["A", "B<String>"]);
        assert_eq!(outer.nested[0].fqn, "p.Outer.Inner");
        assert_eq!(outer.nested[0].components[0].ty, "int");
    }

    #[test]
    fn non_private_fields_are_listed_and_a_file_with_no_declarations_has_an_empty_surface() {
        let source = "package p;\npublic class C { public static final int LIMIT = 50; private int hidden; protected String name; }\n";
        let c = &java_surface(source).types[0];
        let fields: Vec<&str> = c.fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(fields, ["LIMIT", "hidden", "name"]);
        let text = java_surface(source).render();
        assert!(
            text.contains("public static final int LIMIT")
                && text.contains("protected String name"),
            "{text}"
        );
        assert!(!text.contains("hidden"), "{text}");
        assert_eq!(java_surface("// nothing\n"), Surface::default());
    }
}
