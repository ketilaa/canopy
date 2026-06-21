use roots_core::{Language, RelationshipKind};
use roots_parser::extract;

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name);
    std::fs::read(path).expect("fixture file not found")
}

#[test]
fn java_fqn_uses_package() {
    let src = fixture("OrderService.java");
    let output = extract(&src, &Language::Java, "orders/OrderService.java", "orders", "test-workspace").unwrap();
    let class = output.symbols.iter().find(|s| s.name == "OrderService").unwrap();
    assert_eq!(class.fqn, "com.example.orders.OrderService");
}

#[test]
fn java_method_fqn_includes_class() {
    let src = fixture("OrderService.java");
    let output = extract(&src, &Language::Java, "orders/OrderService.java", "orders", "test-workspace").unwrap();
    let method = output.symbols.iter().find(|s| s.name == "placeOrder").unwrap();
    assert_eq!(method.fqn, "com.example.orders.OrderService.placeOrder");
}

#[test]
fn java_extracts_imports() {
    let src = fixture("OrderService.java");
    let output = extract(&src, &Language::Java, "orders/OrderService.java", "orders", "test-workspace").unwrap();
    let imports: Vec<_> = output.relationships.iter()
        .filter(|r| r.kind == RelationshipKind::Imports)
        .collect();
    assert!(!imports.is_empty(), "expected at least one import");
    let to_symbols: Vec<&str> = imports.iter().map(|r| r.to_symbol.as_str()).collect();
    assert!(to_symbols.iter().any(|s| s.contains("BaseService")),
        "expected BaseService import in {to_symbols:?}");
    assert!(to_symbols.iter().any(|s| s.contains("Repository")),
        "expected Repository import in {to_symbols:?}");
}

#[test]
fn java_extracts_extends() {
    let src = fixture("OrderService.java");
    let output = extract(&src, &Language::Java, "orders/OrderService.java", "orders", "test-workspace").unwrap();
    let extends: Vec<_> = output.relationships.iter()
        .filter(|r| r.kind == RelationshipKind::Extends)
        .collect();
    assert!(!extends.is_empty(), "expected at least one extends relationship");
    let r = &extends[0];
    assert_eq!(r.from_symbol, "com.example.orders.OrderService");
    assert_eq!(r.to_symbol, "BaseService");
}

#[test]
fn java_extracts_implements() {
    let src = fixture("OrderService.java");
    let output = extract(&src, &Language::Java, "orders/OrderService.java", "orders", "test-workspace").unwrap();
    let impls: Vec<_> = output.relationships.iter()
        .filter(|r| r.kind == RelationshipKind::Implements)
        .collect();
    assert!(!impls.is_empty(), "expected at least one implements relationship");
    let to_symbols: Vec<&str> = impls.iter().map(|r| r.to_symbol.as_str()).collect();
    assert!(to_symbols.contains(&"Repository"),
        "expected Repository in implements {to_symbols:?}");
}

#[test]
fn java_extracts_calls() {
    let src = fixture("OrderService.java");
    let output = extract(&src, &Language::Java, "orders/OrderService.java", "orders", "test-workspace").unwrap();
    let calls: Vec<_> = output.relationships.iter()
        .filter(|r| r.kind == RelationshipKind::Calls)
        .collect();
    assert!(!calls.is_empty(), "expected at least one call relationship");
    let callees: Vec<&str> = calls.iter().map(|r| r.to_symbol.as_str()).collect();
    assert!(callees.contains(&"validateInput"),
        "expected validateInput call in {callees:?}");
    assert!(callees.contains(&"notifyCustomer"),
        "expected notifyCustomer call in {callees:?}");
}

#[test]
fn java_calls_have_from_symbol() {
    let src = fixture("OrderService.java");
    let output = extract(&src, &Language::Java, "orders/OrderService.java", "orders", "test-workspace").unwrap();
    let call = output.relationships.iter()
        .find(|r| r.kind == RelationshipKind::Calls && r.to_symbol == "validateInput")
        .expect("validateInput call not found");
    assert!(call.from_symbol.contains("OrderService"),
        "expected from_symbol to contain class, got: {}", call.from_symbol);
    assert!(call.from_symbol.contains("placeOrder"),
        "expected from_symbol to contain method, got: {}", call.from_symbol);
}

#[test]
fn java_relationships_have_file_and_line() {
    let src = fixture("OrderService.java");
    let output = extract(&src, &Language::Java, "orders/OrderService.java", "orders", "test-workspace").unwrap();
    for r in &output.relationships {
        assert_eq!(r.file, "orders/OrderService.java", "file should match relative_path");
        assert!(r.line.unwrap_or(0) > 0, "line should be > 0 for {:?}", r.kind);
    }
}
