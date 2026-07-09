/// True for files that should go through the Red/Green TDD cycle.
/// Strategy: include everything under src/ except framework entry points that have
/// no unit-testable logic. Blacklisting is safer than whitelisting — any new layer
/// the planner invents (src/utils/, src/validators/, etc.) gets TDD automatically.
pub(crate) fn is_tdd_candidate(file: &str) -> bool {
    // Java: all source files except framework entry points and config
    if file.ends_with(".java") && file.contains("/src/main/java/") {
        let filename = std::path::Path::new(file)
            .file_name().and_then(|n| n.to_str()).unwrap_or("");
        return !filename.ends_with("Application.java")
            && !filename.ends_with("Repository.java")
            && !filename.ends_with("Configuration.java")
            && !filename.ends_with("Config.java");
    }
    // TypeScript / TSX: everything under src/ except entry points and app wiring
    if (file.ends_with(".ts") || file.ends_with(".tsx")) && !file.ends_with(".d.ts") {
        if file.contains("/src/") {
            let filename = std::path::Path::new(file)
                .file_name().and_then(|n| n.to_str()).unwrap_or("");
            // index.ts — server launch script (app.listen), not unit-testable
            // app.ts   — Express app wiring, covered by route tests
            return filename != "index.ts"
                && filename != "index.tsx"
                && filename != "app.ts";
        }
    }
    false
}

/// Returns true for test files that should be skipped when TDD already wrote them.
/// Suffix-based, not path-based — Java keeps a mirrored `/src/test/java/` tree (its own
/// ecosystem's convention), but TypeScript/TSX tests are co-located next to their
/// implementation file, so the directory alone can't distinguish a test from a source file.
pub(crate) fn is_test_file(file: &str) -> bool {
    file.contains("/src/test/java/")
        || file.ends_with(".test.ts")
        || file.ends_with(".test.tsx")
        || file.ends_with("Test.java")
        || file.ends_with("IT.java")
}

/// Maps implementation file → test file path.
pub(crate) fn derive_test_file_path(impl_file: &str) -> Option<String> {
    // Java: mirrors the full directory structure under a parallel src/test/java root — this
    // project's own ecosystem convention (Maven standard layout).
    if impl_file.contains("/src/main/java/") {
        let test_path = impl_file.replace("/src/main/java/", "/src/test/java/");
        let p = std::path::Path::new(&test_path);
        let stem = p.file_stem()?.to_str()?;
        let parent = p.parent()?.to_str()?;
        return Some(format!("{}/{}Test.java", parent, stem));
    }
    // TypeScript / TSX: co-located next to the implementation file, per JS/TS ecosystem
    // convention — e.g. services/product/src/services/ProductService.ts
    //               → services/product/src/services/ProductService.test.ts
    if impl_file.ends_with(".ts") || impl_file.ends_with(".tsx") {
        let p = std::path::Path::new(impl_file);
        let stem = p.file_stem()?.to_str()?;
        let parent = p.parent()?.to_str()?;
        let ext = if impl_file.ends_with(".tsx") { "tsx" } else { "ts" };
        return Some(format!("{}/{}.test.{}", parent, stem, ext));
    }
    None
}

/// Extracts the simple class name from a test file path (the file stem).
pub(crate) fn test_class_name(test_file: &str) -> Option<String> {
    std::path::Path::new(test_file)
        .file_stem().and_then(|s| s.to_str()).map(|s| s.to_string())
}
