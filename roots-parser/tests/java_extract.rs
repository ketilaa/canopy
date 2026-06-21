use roots_core::SymbolKind;
use roots_parser::extract;
use roots_core::Language;

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name);
    std::fs::read(path).expect("fixture file not found")
}

#[test]
fn extracts_class_from_java() {
    let src = fixture("OrderService.java");
    let symbols = extract(&src, &Language::Java, "orders/OrderService.java", "orders", "test-workspace").unwrap().symbols;
    let names: Vec<_> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"OrderService"), "expected OrderService in {names:?}");
}

#[test]
fn extracts_interface_from_java() {
    let src = fixture("OrderService.java");
    let symbols = extract(&src, &Language::Java, "orders/OrderService.java", "orders", "test-workspace").unwrap().symbols;
    let interfaces: Vec<_> = symbols.iter()
        .filter(|s| s.kind == SymbolKind::Interface)
        .map(|s| s.name.as_str())
        .collect();
    assert!(interfaces.contains(&"OrderRepository"), "expected OrderRepository in {interfaces:?}");
}

#[test]
fn extracts_enum_from_java() {
    let src = fixture("OrderService.java");
    let symbols = extract(&src, &Language::Java, "orders/OrderService.java", "orders", "test-workspace").unwrap().symbols;
    let enums: Vec<_> = symbols.iter()
        .filter(|s| s.kind == SymbolKind::Enum)
        .map(|s| s.name.as_str())
        .collect();
    assert!(enums.contains(&"OrderStatus"), "expected OrderStatus in {enums:?}");
}

#[test]
fn extracts_methods_from_java() {
    let src = fixture("OrderService.java");
    let symbols = extract(&src, &Language::Java, "orders/OrderService.java", "orders", "test-workspace").unwrap().symbols;
    let methods: Vec<_> = symbols.iter()
        .filter(|s| s.kind == SymbolKind::Method)
        .map(|s| s.name.as_str())
        .collect();
    assert!(methods.contains(&"placeOrder"), "expected placeOrder in {methods:?}");
    assert!(methods.contains(&"cancelOrder"), "expected cancelOrder in {methods:?}");
}

#[test]
fn java_symbols_have_correct_metadata() {
    let src = fixture("OrderService.java");
    let symbols = extract(&src, &Language::Java, "orders/OrderService.java", "orders", "test-workspace").unwrap().symbols;
    let class = symbols.iter().find(|s| s.name == "OrderService").unwrap();
    assert_eq!(class.language, Language::Java);
    assert_eq!(class.project, "orders");
    assert_eq!(class.file, "orders/OrderService.java");
    assert!(class.line > 0);
}
