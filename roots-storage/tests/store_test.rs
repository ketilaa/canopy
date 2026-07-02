use roots_core::{Language, Relationship, RelationshipKind, Symbol, SymbolKind};
use roots_storage::Store;

fn open_store() -> Store {
    let s = Store::open_in_memory().unwrap();
    s.init_schema().unwrap();
    s.upsert_workspace("test-ws", "Test Workspace").unwrap();
    s
}

// ---- project / file upsert tests ----

#[test]
fn upsert_project_returns_id() {
    let s = open_store();
    let id = s.upsert_project("test-ws", "orders", "/workspace/orders", &Language::Java).unwrap();
    assert!(id > 0);
}

#[test]
fn upsert_project_idempotent() {
    let s = open_store();
    let id1 = s.upsert_project("test-ws", "orders", "/workspace/orders", &Language::Java).unwrap();
    let id2 = s.upsert_project("test-ws", "orders", "/workspace/orders", &Language::Java).unwrap();
    assert_eq!(id1, id2);
}

#[test]
fn upsert_file_returns_id() {
    let s = open_store();
    let pid = s.upsert_project("test-ws", "orders", "/workspace/orders", &Language::Java).unwrap();
    let fid = s.upsert_file("test-ws", pid, "orders/src/Foo.java", &Language::Java, "2024-01-01T00:00:00Z").unwrap();
    assert!(fid > 0);
}

#[test]
fn upsert_file_idempotent() {
    let s = open_store();
    let pid = s.upsert_project("test-ws", "orders", "/workspace/orders", &Language::Java).unwrap();
    let fid1 = s.upsert_file("test-ws", pid, "orders/src/Foo.java", &Language::Java, "2024-01-01T00:00:00Z").unwrap();
    let fid2 = s.upsert_file("test-ws", pid, "orders/src/Foo.java", &Language::Java, "2024-01-02T00:00:00Z").unwrap();
    assert_eq!(fid1, fid2);
}

// ---- symbol query tests ----

#[test]
fn insert_and_query_exact() {
    let s = open_store();
    let pid = s.upsert_project("test-ws", "orders", "/workspace/orders", &Language::Java).unwrap();
    let fid = s.upsert_file("test-ws", pid, "orders/src/OrderService.java", &Language::Java, "2024-01-01T00:00:00Z").unwrap();
    let symbols = vec![
        Symbol {
            name: "OrderService".into(),
            kind: SymbolKind::Class,
            file: "orders/src/OrderService.java".into(),
            language: Language::Java,
            project: "orders".into(),
            workspace_id: "test-ws".into(),
            line: 5,
            fqn: "com.example.OrderService".into(),
                signature:    None,
        },
        Symbol {
            name: "placeOrder".into(),
            kind: SymbolKind::Method,
            file: "orders/src/OrderService.java".into(),
            language: Language::Java,
            project: "orders".into(),
            workspace_id: "test-ws".into(),
            line: 10,
            fqn: "com.example.OrderService.placeOrder".into(),
                signature:    None,
        },
    ];
    s.insert_symbols("test-ws", pid, fid, &symbols).unwrap();

    let results = s.query_exact("test-ws", "OrderService").unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "OrderService");
    assert_eq!(results[0].kind, "class");
    assert_eq!(results[0].language, "java");
    assert_eq!(results[0].project, "orders");
    assert_eq!(results[0].workspace_id, "test-ws");
}

#[test]
fn query_prefix_case_insensitive() {
    let s = open_store();
    let pid = s.upsert_project("test-ws", "orders", "/workspace/orders", &Language::Java).unwrap();
    let fid = s.upsert_file("test-ws", pid, "orders/src/OrderService.java", &Language::Java, "2024-01-01T00:00:00Z").unwrap();
    let symbols = vec![
        Symbol {
            name: "OrderService".into(),
            kind: SymbolKind::Class,
            file: "orders/src/OrderService.java".into(),
            language: Language::Java,
            project: "orders".into(),
            workspace_id: "test-ws".into(),
            line: 5,
            fqn: "com.example.OrderService".into(),
                signature:    None,
        },
    ];
    s.insert_symbols("test-ws", pid, fid, &symbols).unwrap();

    let results = s.query_prefix("test-ws", "order").unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "OrderService");

    let results2 = s.query_prefix("test-ws", "ORDER").unwrap();
    assert_eq!(results2.len(), 1);
}

#[test]
fn query_prefix_no_match() {
    let s = open_store();
    let pid = s.upsert_project("test-ws", "orders", "/workspace/orders", &Language::Java).unwrap();
    let fid = s.upsert_file("test-ws", pid, "orders/src/OrderService.java", &Language::Java, "2024-01-01T00:00:00Z").unwrap();
    let symbols = vec![Symbol {
        name: "OrderService".into(),
        kind: SymbolKind::Class,
        file: "orders/src/OrderService.java".into(),
        language: Language::Java,
        project: "orders".into(),
        workspace_id: "test-ws".into(),
        line: 5,
        fqn: "com.example.OrderService".into(),
            signature:    None,
        }];
    s.insert_symbols("test-ws", pid, fid, &symbols).unwrap();

    let results = s.query_prefix("test-ws", "Notification").unwrap();
    assert!(results.is_empty());
}

#[test]
fn dump_all_returns_all_symbols() {
    let s = open_store();
    let pid = s.upsert_project("test-ws", "svc", "/workspace/svc", &Language::TypeScript).unwrap();
    let fid = s.upsert_file("test-ws", pid, "svc/src/index.ts", &Language::TypeScript, "2024-01-01T00:00:00Z").unwrap();
    let symbols = vec![
        Symbol { name: "UserService".into(), kind: SymbolKind::Class, file: "svc/src/index.ts".into(), language: Language::TypeScript, project: "svc".into(), workspace_id: "test-ws".into(), line: 1, fqn: "svc/src/index.ts#UserService".into(), signature: None },
        Symbol { name: "IUserRepo".into(), kind: SymbolKind::Interface, file: "svc/src/index.ts".into(), language: Language::TypeScript, project: "svc".into(), workspace_id: "test-ws".into(), line: 10, fqn: "svc/src/index.ts#IUserRepo".into(), signature: None },
    ];
    s.insert_symbols("test-ws", pid, fid, &symbols).unwrap();

    let all = s.dump_all("test-ws").unwrap();
    assert_eq!(all.len(), 2);
}

#[test]
fn delete_symbols_for_file() {
    let s = open_store();
    let pid = s.upsert_project("test-ws", "svc", "/workspace/svc", &Language::Java).unwrap();
    let fid = s.upsert_file("test-ws", pid, "svc/Foo.java", &Language::Java, "2024-01-01T00:00:00Z").unwrap();
    let symbols = vec![Symbol {
        name: "Foo".into(), kind: SymbolKind::Class,
        file: "svc/Foo.java".into(), language: Language::Java, project: "svc".into(),
        workspace_id: "test-ws".into(), line: 1, fqn: "com.example.Foo".into(),
            signature:    None,
        }];
    s.insert_symbols("test-ws", pid, fid, &symbols).unwrap();
    assert_eq!(s.dump_all("test-ws").unwrap().len(), 1);

    s.delete_symbols_for_file(fid).unwrap();
    assert!(s.dump_all("test-ws").unwrap().is_empty());
}

#[test]
fn status_counts_correctly() {
    let s = open_store();
    let pid = s.upsert_project("test-ws", "svc", "/workspace/svc", &Language::Java).unwrap();
    let fid = s.upsert_file("test-ws", pid, "svc/Foo.java", &Language::Java, "2024-01-01T00:00:00Z").unwrap();
    let symbols = vec![
        Symbol { name: "Foo".into(), kind: SymbolKind::Class, file: "svc/Foo.java".into(), language: Language::Java, project: "svc".into(), workspace_id: "test-ws".into(), line: 1, fqn: "com.example.Foo".into(), signature: None },
        Symbol { name: "bar".into(), kind: SymbolKind::Method, file: "svc/Foo.java".into(), language: Language::Java, project: "svc".into(), workspace_id: "test-ws".into(), line: 5, fqn: "com.example.Foo.bar".into(), signature: Some("(int count)".into()) },
    ];
    s.insert_symbols("test-ws", pid, fid, &symbols).unwrap();

    let status = s.status("test-ws").unwrap();
    assert_eq!(status.projects, 1);
    assert_eq!(status.files, 1);
    assert_eq!(status.symbols, 2);
    assert_eq!(status.relationships, 0);
}

// ---- relationship helpers and tests ----

fn make_rel(workspace_id: &str, from: &str, to: &str, kind: RelationshipKind) -> Relationship {
    Relationship {
        from_symbol:  from.into(),
        to_symbol:    to.into(),
        kind,
        file:         "svc/Foo.java".into(),
        line:         Some(1),
        workspace_id: workspace_id.into(),
    }
}

#[test]
fn insert_and_query_callers() {
    let s = open_store();
    let rels = vec![
        make_rel("test-ws", "com.example.A.process", "com.example.B.execute", RelationshipKind::Calls),
        make_rel("test-ws", "com.example.C.run",     "com.example.B.execute", RelationshipKind::Calls),
    ];
    s.insert_relationships("test-ws", "svc/Foo.java", &rels).unwrap();

    let callers = s.query_callers("test-ws", "com.example.B.execute").unwrap();
    assert_eq!(callers.len(), 2);
    let froms: Vec<&str> = callers.iter().map(|r| r.from_symbol.as_str()).collect();
    assert!(froms.contains(&"com.example.A.process"));
    assert!(froms.contains(&"com.example.C.run"));
}

#[test]
fn insert_and_query_deps() {
    let s = open_store();
    let rels = vec![
        make_rel("test-ws", "com.example.A", "com.example.B", RelationshipKind::Imports),
        make_rel("test-ws", "com.example.A", "com.example.Base", RelationshipKind::Extends),
    ];
    s.insert_relationships("test-ws", "svc/Foo.java", &rels).unwrap();

    let deps = s.query_deps("test-ws", "com.example.A").unwrap();
    assert_eq!(deps.len(), 2);
    let kinds: Vec<&str> = deps.iter().map(|r| r.kind.as_str()).collect();
    assert!(kinds.contains(&"IMPORTS"));
    assert!(kinds.contains(&"EXTENDS"));
}

#[test]
fn query_impact_recursive() {
    let s = open_store();
    // A → B → C: changing C affects B and A
    let rels = vec![
        make_rel("test-ws", "com.example.A", "com.example.B", RelationshipKind::Calls),
        make_rel("test-ws", "com.example.B", "com.example.C", RelationshipKind::Calls),
    ];
    s.insert_relationships("test-ws", "svc/Foo.java", &rels).unwrap();

    let impact = s.query_impact("test-ws", "com.example.C").unwrap();
    assert!(impact.contains(&"com.example.B".to_string()), "B should be impacted: {impact:?}");
    assert!(impact.contains(&"com.example.A".to_string()), "A should be transitively impacted: {impact:?}");
}

#[test]
fn delete_relationships_for_file() {
    let s = open_store();
    let rels = vec![
        make_rel("test-ws", "A", "B", RelationshipKind::Calls),
        make_rel("test-ws", "A", "C", RelationshipKind::Imports),
    ];
    s.insert_relationships("test-ws", "svc/Foo.java", &rels).unwrap();
    assert_eq!(s.query_deps("test-ws", "A").unwrap().len(), 2);

    s.delete_relationships_for_file("test-ws", "svc/Foo.java").unwrap();
    assert!(s.query_deps("test-ws", "A").unwrap().is_empty());
}

#[test]
fn query_graph_returns_both_directions() {
    let s = open_store();
    let rels = vec![
        make_rel("test-ws", "com.example.A", "com.example.B", RelationshipKind::Calls),
        make_rel("test-ws", "com.example.C", "com.example.A", RelationshipKind::Calls),
    ];
    s.insert_relationships("test-ws", "svc/Foo.java", &rels).unwrap();

    let graph = s.query_graph("test-ws", "com.example.A").unwrap();
    assert_eq!(graph.outgoing.len(), 1);
    assert_eq!(graph.incoming.len(), 1);
    assert_eq!(graph.outgoing[0].to_symbol, "com.example.B");
    assert_eq!(graph.incoming[0].from_symbol, "com.example.C");
}

#[test]
fn status_counts_relationships() {
    let s = open_store();
    let rels = vec![
        make_rel("test-ws", "A", "B", RelationshipKind::Calls),
        make_rel("test-ws", "A", "C", RelationshipKind::Imports),
        make_rel("test-ws", "B", "C", RelationshipKind::Extends),
    ];
    s.insert_relationships("test-ws", "svc/Foo.java", &rels).unwrap();

    let status = s.status("test-ws").unwrap();
    assert_eq!(status.relationships, 3);
}

// ---- V3 workspace isolation tests ----

#[test]
fn upsert_and_list_workspaces() {
    let s = open_store();
    s.upsert_workspace("ws-b", "Workspace B").unwrap();

    let list = s.list_workspaces().unwrap();
    let ids: Vec<&str> = list.iter().map(|w| w.id.as_str()).collect();
    assert!(ids.contains(&"test-ws"), "test-ws should be listed");
    assert!(ids.contains(&"ws-b"), "ws-b should be listed");
}

#[test]
fn workspace_exists_true_and_false() {
    let s = open_store();
    assert!(s.workspace_exists("test-ws").unwrap());
    assert!(!s.workspace_exists("nonexistent").unwrap());
}

#[test]
fn upsert_project_scoped_uniqueness() {
    let s = open_store();
    s.upsert_workspace("ws-b", "Workspace B").unwrap();

    let id_a = s.upsert_project("test-ws", "frontend", "/a/frontend", &Language::TypeScript).unwrap();
    let id_b = s.upsert_project("ws-b",    "frontend", "/b/frontend", &Language::TypeScript).unwrap();

    assert_ne!(id_a, id_b, "same project name in different workspaces must get distinct ids");

    // Idempotent within same workspace
    let id_a2 = s.upsert_project("test-ws", "frontend", "/a/frontend", &Language::TypeScript).unwrap();
    assert_eq!(id_a, id_a2);
}

#[test]
fn two_projects_same_name_different_workspaces_are_isolated() {
    let s = open_store();
    s.upsert_workspace("ws-b", "Workspace B").unwrap();

    let pid_a = s.upsert_project("test-ws", "frontend", "/a/frontend", &Language::TypeScript).unwrap();
    let pid_b = s.upsert_project("ws-b",    "frontend", "/b/frontend", &Language::TypeScript).unwrap();

    let fid_a = s.upsert_file("test-ws", pid_a, "frontend/src/App.ts", &Language::TypeScript, "2024-01-01T00:00:00Z").unwrap();
    let fid_b = s.upsert_file("ws-b",    pid_b, "frontend/src/App.ts", &Language::TypeScript, "2024-01-01T00:00:00Z").unwrap();

    let sym_a = Symbol {
        name: "MyComponent".into(), kind: SymbolKind::Class,
        file: "frontend/src/App.ts".into(), language: Language::TypeScript,
        project: "frontend".into(), workspace_id: "test-ws".into(),
        line: 1, fqn: "frontend/src/App.ts#MyComponent-ws-a".into(),
            signature:    None,
        };
    let sym_b = Symbol {
        name: "MyComponent".into(), kind: SymbolKind::Class,
        file: "frontend/src/App.ts".into(), language: Language::TypeScript,
        project: "frontend".into(), workspace_id: "ws-b".into(),
        line: 1, fqn: "frontend/src/App.ts#MyComponent-ws-b".into(),
            signature:    None,
        };

    s.insert_symbols("test-ws", pid_a, fid_a, &[sym_a]).unwrap();
    s.insert_symbols("ws-b",    pid_b, fid_b, &[sym_b]).unwrap();

    let results_a = s.query_exact("test-ws", "MyComponent").unwrap();
    assert_eq!(results_a.len(), 1);
    assert_eq!(results_a[0].workspace_id, "test-ws");

    let results_b = s.query_exact("ws-b", "MyComponent").unwrap();
    assert_eq!(results_b.len(), 1);
    assert_eq!(results_b[0].workspace_id, "ws-b");
}

#[test]
fn query_does_not_leak_across_workspace() {
    let s = open_store();
    s.upsert_workspace("ws-b", "Workspace B").unwrap();

    let pid_a = s.upsert_project("test-ws", "orders", "/a/orders", &Language::Java).unwrap();
    let fid_a = s.upsert_file("test-ws", pid_a, "orders/OrderService.java", &Language::Java, "2024-01-01T00:00:00Z").unwrap();
    let sym = Symbol {
        name: "OrderService".into(), kind: SymbolKind::Class,
        file: "orders/OrderService.java".into(), language: Language::Java,
        project: "orders".into(), workspace_id: "test-ws".into(),
        line: 1, fqn: "com.example.OrderService".into(),
            signature:    None,
        };
    s.insert_symbols("test-ws", pid_a, fid_a, &[sym]).unwrap();

    // Query from a different workspace should return nothing
    let results = s.query_exact("ws-b", "OrderService").unwrap();
    assert!(results.is_empty(), "query should not leak across workspace boundary");

    let all = s.dump_all("ws-b").unwrap();
    assert!(all.is_empty());
}

#[test]
fn status_scoped_by_workspace() {
    let s = open_store();
    s.upsert_workspace("ws-b", "Workspace B").unwrap();

    let pid_a = s.upsert_project("test-ws", "svc", "/a/svc", &Language::Java).unwrap();
    let fid_a = s.upsert_file("test-ws", pid_a, "svc/Foo.java", &Language::Java, "2024-01-01T00:00:00Z").unwrap();
    let sym = Symbol {
        name: "Foo".into(), kind: SymbolKind::Class,
        file: "svc/Foo.java".into(), language: Language::Java,
        project: "svc".into(), workspace_id: "test-ws".into(),
        line: 1, fqn: "com.example.Foo".into(),
            signature:    None,
        };
    s.insert_symbols("test-ws", pid_a, fid_a, &[sym]).unwrap();

    let status_a = s.status("test-ws").unwrap();
    assert_eq!(status_a.projects, 1);
    assert_eq!(status_a.symbols, 1);

    let status_b = s.status("ws-b").unwrap();
    assert_eq!(status_b.projects, 0);
    assert_eq!(status_b.symbols, 0);

    // Total workspaces reflects global count (includes the 'default' workspace from migration)
    assert!(status_a.workspaces >= 2);
    assert_eq!(status_a.workspaces, status_b.workspaces);
}

#[test]
fn recursive_impact_scoped_by_workspace() {
    let s = open_store();
    s.upsert_workspace("ws-b", "Workspace B").unwrap();

    // ws-a: A -> B -> C
    let rels_a = vec![
        make_rel("test-ws", "ws-a.A", "ws-a.B", RelationshipKind::Calls),
        make_rel("test-ws", "ws-a.B", "ws-a.C", RelationshipKind::Calls),
    ];
    s.insert_relationships("test-ws", "svc/a.java", &rels_a).unwrap();

    // ws-b: X -> Y -> Z
    let rels_b = vec![
        make_rel("ws-b", "ws-b.X", "ws-b.Y", RelationshipKind::Calls),
        make_rel("ws-b", "ws-b.Y", "ws-b.Z", RelationshipKind::Calls),
    ];
    s.insert_relationships("ws-b", "svc/b.java", &rels_b).unwrap();

    // Impacting ws-b.Z must only return ws-b symbols
    let impact = s.query_impact("ws-b", "ws-b.Z").unwrap();
    assert!(impact.contains(&"ws-b.Y".to_string()), "Y should be impacted");
    assert!(impact.contains(&"ws-b.X".to_string()), "X should be transitively impacted");
    assert!(!impact.contains(&"ws-a.B".to_string()), "ws-a symbols must not appear");
    assert!(!impact.contains(&"ws-a.A".to_string()), "ws-a symbols must not appear");
}
