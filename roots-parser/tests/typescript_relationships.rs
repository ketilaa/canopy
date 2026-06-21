use roots_core::{Language, RelationshipKind};
use roots_parser::extract;

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name);
    std::fs::read(path).expect("fixture file not found")
}

#[test]
fn ts_fqn_uses_file_path() {
    let src = fixture("notification.ts");
    let output = extract(&src, &Language::TypeScript, "src/notification.ts", "svc", "test-workspace").unwrap();
    let cls = output.symbols.iter().find(|s| s.name == "EmailNotifier").unwrap();
    assert_eq!(cls.fqn, "src/notification.ts#EmailNotifier");
}

#[test]
fn ts_extracts_named_import() {
    let src = fixture("notification.ts");
    let output = extract(&src, &Language::TypeScript, "src/notification.ts", "svc", "test-workspace").unwrap();
    let imports: Vec<_> = output.relationships.iter()
        .filter(|r| r.kind == RelationshipKind::Imports)
        .collect();
    assert!(!imports.is_empty(), "expected at least one import");
    let to_symbols: Vec<&str> = imports.iter().map(|r| r.to_symbol.as_str()).collect();
    assert!(to_symbols.contains(&"Logger"),
        "expected Logger import in {to_symbols:?}");
}

#[test]
fn ts_extracts_implements() {
    let src = fixture("notification.ts");
    let output = extract(&src, &Language::TypeScript, "src/notification.ts", "svc", "test-workspace").unwrap();
    let impls: Vec<_> = output.relationships.iter()
        .filter(|r| r.kind == RelationshipKind::Implements)
        .collect();
    assert!(!impls.is_empty(), "expected at least one implements relationship");
    let r = &impls[0];
    assert!(r.from_symbol.contains("EmailNotifier"),
        "from_symbol should contain class name, got: {}", r.from_symbol);
    assert_eq!(r.to_symbol, "NotificationService");
}

#[test]
fn ts_extracts_calls() {
    let src = fixture("notification.ts");
    let output = extract(&src, &Language::TypeScript, "src/notification.ts", "svc", "test-workspace").unwrap();
    let calls: Vec<_> = output.relationships.iter()
        .filter(|r| r.kind == RelationshipKind::Calls)
        .collect();
    assert!(!calls.is_empty(), "expected at least one call relationship");
    let callees: Vec<&str> = calls.iter().map(|r| r.to_symbol.as_str()).collect();
    assert!(callees.contains(&"log"),
        "expected log call (Logger.log) in {callees:?}");
}

#[test]
fn ts_relationships_have_file_and_line() {
    let src = fixture("notification.ts");
    let output = extract(&src, &Language::TypeScript, "src/notification.ts", "svc", "test-workspace").unwrap();
    for r in &output.relationships {
        assert_eq!(r.file, "src/notification.ts");
        assert!(r.line.unwrap_or(0) > 0);
    }
}
