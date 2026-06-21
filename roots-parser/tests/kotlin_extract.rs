use roots_core::{Language, SymbolKind};
use roots_parser::extract;

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name);
    std::fs::read(path).expect("fixture file not found")
}

#[test]
fn extracts_class_from_kotlin() {
    let src = fixture("UserService.kt");
    let symbols = extract(&src, &Language::Kotlin, "users/UserService.kt", "users", "test-workspace").unwrap().symbols;
    let names: Vec<_> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"UserService"), "expected UserService in {names:?}");
}

#[test]
fn extracts_object_from_kotlin() {
    let src = fixture("UserService.kt");
    let symbols = extract(&src, &Language::Kotlin, "users/UserService.kt", "users", "test-workspace").unwrap().symbols;
    let names: Vec<_> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"UserCache"), "expected UserCache in {names:?}");
}

#[test]
fn extracts_functions_from_kotlin() {
    let src = fixture("UserService.kt");
    let symbols = extract(&src, &Language::Kotlin, "users/UserService.kt", "users", "test-workspace").unwrap().symbols;
    let functions: Vec<_> = symbols.iter()
        .filter(|s| s.kind == SymbolKind::Function)
        .map(|s| s.name.as_str())
        .collect();
    assert!(functions.contains(&"findUser"), "expected findUser in {functions:?}");
    assert!(functions.contains(&"createUser"), "expected createUser in {functions:?}");
}

#[test]
fn kotlin_symbols_have_correct_metadata() {
    let src = fixture("UserService.kt");
    let symbols = extract(&src, &Language::Kotlin, "users/UserService.kt", "users", "test-workspace").unwrap().symbols;
    let class = symbols.iter().find(|s| s.name == "UserService").unwrap();
    assert_eq!(class.language, Language::Kotlin);
    assert_eq!(class.project, "users");
    assert_eq!(class.file, "users/UserService.kt");
    assert!(class.line > 0);
}
