use roots_core::{Language, RelationshipKind};
use roots_parser::extract;

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name);
    std::fs::read(path).expect("fixture file not found")
}

#[test]
fn kotlin_fqn_uses_package() {
    let src = fixture("UserService.kt");
    let output = extract(&src, &Language::Kotlin, "users/UserService.kt", "users", "test-workspace").unwrap();
    let cls = output.symbols.iter().find(|s| s.name == "UserService").unwrap();
    assert_eq!(cls.fqn, "com.example.users.UserService");
}

#[test]
fn kotlin_function_fqn_includes_class() {
    let src = fixture("UserService.kt");
    let output = extract(&src, &Language::Kotlin, "users/UserService.kt", "users", "test-workspace").unwrap();
    let fn_sym = output.symbols.iter().find(|s| s.name == "findUser").unwrap();
    assert!(fn_sym.fqn.contains("UserService"),
        "fqn should contain class name: {}", fn_sym.fqn);
    assert!(fn_sym.fqn.contains("findUser"),
        "fqn should contain function name: {}", fn_sym.fqn);
}

#[test]
fn kotlin_extracts_import() {
    let src = fixture("UserService.kt");
    let output = extract(&src, &Language::Kotlin, "users/UserService.kt", "users", "test-workspace").unwrap();
    let imports: Vec<_> = output.relationships.iter()
        .filter(|r| r.kind == RelationshipKind::Imports)
        .collect();
    assert!(!imports.is_empty(), "expected at least one import");
    let to_symbols: Vec<&str> = imports.iter().map(|r| r.to_symbol.as_str()).collect();
    assert!(to_symbols.iter().any(|s| s.contains("BaseService")),
        "expected BaseService import in {to_symbols:?}");
}

#[test]
fn kotlin_extracts_extends() {
    let src = fixture("UserService.kt");
    let output = extract(&src, &Language::Kotlin, "users/UserService.kt", "users", "test-workspace").unwrap();
    let extends: Vec<_> = output.relationships.iter()
        .filter(|r| r.kind == RelationshipKind::Extends)
        .collect();
    assert!(!extends.is_empty(), "expected at least one extends relationship");
    let r = &extends[0];
    assert!(r.from_symbol.contains("UserService"),
        "from_symbol should contain UserService: {}", r.from_symbol);
    assert_eq!(r.to_symbol, "BaseService");
}

#[test]
fn kotlin_relationships_have_file_and_line() {
    let src = fixture("UserService.kt");
    let output = extract(&src, &Language::Kotlin, "users/UserService.kt", "users", "test-workspace").unwrap();
    for r in &output.relationships {
        assert_eq!(r.file, "users/UserService.kt");
        assert!(r.line.unwrap_or(0) > 0);
    }
}
