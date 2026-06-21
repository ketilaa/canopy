use roots_core::{Language, SymbolKind};
use roots_parser::extract;

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name);
    std::fs::read(path).expect("fixture file not found")
}

#[test]
fn extracts_interface_from_typescript() {
    let src = fixture("notification.ts");
    let symbols = extract(&src, &Language::TypeScript, "src/notification.ts", "svc", "test-workspace").unwrap().symbols;
    let interfaces: Vec<_> = symbols.iter()
        .filter(|s| s.kind == SymbolKind::Interface)
        .map(|s| s.name.as_str())
        .collect();
    assert!(interfaces.contains(&"NotificationService"), "expected NotificationService in {interfaces:?}");
}

#[test]
fn extracts_class_from_typescript() {
    let src = fixture("notification.ts");
    let symbols = extract(&src, &Language::TypeScript, "src/notification.ts", "svc", "test-workspace").unwrap().symbols;
    let classes: Vec<_> = symbols.iter()
        .filter(|s| s.kind == SymbolKind::Class)
        .map(|s| s.name.as_str())
        .collect();
    assert!(classes.contains(&"EmailNotifier"), "expected EmailNotifier in {classes:?}");
}

#[test]
fn extracts_enum_from_typescript() {
    let src = fixture("notification.ts");
    let symbols = extract(&src, &Language::TypeScript, "src/notification.ts", "svc", "test-workspace").unwrap().symbols;
    let enums: Vec<_> = symbols.iter()
        .filter(|s| s.kind == SymbolKind::Enum)
        .map(|s| s.name.as_str())
        .collect();
    assert!(enums.contains(&"NotificationChannel"), "expected NotificationChannel in {enums:?}");
}

#[test]
fn extracts_function_from_typescript() {
    let src = fixture("notification.ts");
    let symbols = extract(&src, &Language::TypeScript, "src/notification.ts", "svc", "test-workspace").unwrap().symbols;
    let functions: Vec<_> = symbols.iter()
        .filter(|s| s.kind == SymbolKind::Function)
        .map(|s| s.name.as_str())
        .collect();
    assert!(functions.contains(&"createNotifier"), "expected createNotifier in {functions:?}");
}

#[test]
fn extracts_method_from_typescript() {
    let src = fixture("notification.ts");
    let symbols = extract(&src, &Language::TypeScript, "src/notification.ts", "svc", "test-workspace").unwrap().symbols;
    let methods: Vec<_> = symbols.iter()
        .filter(|s| s.kind == SymbolKind::Method)
        .map(|s| s.name.as_str())
        .collect();
    assert!(methods.contains(&"send"), "expected send in {methods:?}");
}

#[test]
fn typescript_symbols_have_correct_metadata() {
    let src = fixture("notification.ts");
    let symbols = extract(&src, &Language::TypeScript, "src/notification.ts", "svc", "test-workspace").unwrap().symbols;
    let cls = symbols.iter().find(|s| s.name == "EmailNotifier").unwrap();
    assert_eq!(cls.language, Language::TypeScript);
    assert_eq!(cls.project, "svc");
    assert_eq!(cls.file, "src/notification.ts");
    assert!(cls.line > 0);
}
