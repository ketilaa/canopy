/// Remove ANSI escape sequences from a string so error pattern matching works on raw text.
pub(crate) fn strip_ansi(s: impl AsRef<str>) -> String {
    let s = s.as_ref();
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&'[') {
            chars.next(); // consume '['
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() { break; }
            }
        } else {
            out.push(ch);
        }
    }
    out
}

pub(crate) fn extract_error_files(output: &str, service_dir: &str) -> Vec<String> {
    let mut files: Vec<String> = Vec::new();
    let svc_path = std::path::Path::new(service_dir);

    let resolve = |path: &str| -> Option<String> {
        let p = std::path::Path::new(path);
        let candidates = if p.is_absolute() {
            vec![p.to_path_buf()]
        } else {
            vec![
                svc_path.join(p),                          // relative to service dir
                std::env::current_dir().ok()?.join(p),     // relative to project root
            ]
        };
        candidates.into_iter()
            .map(|c| c.to_string_lossy().to_string())
            .find(|s| std::path::Path::new(s).exists())
    };

    for line in output.lines() {
        // Maven: [ERROR] /abs/path/to/File.java:[line,col] message
        if let Some(rest) = line.strip_prefix("[ERROR] ") {
            if let Some(bracket) = rest.find(":[") {
                let path = rest[..bracket].trim();
                if path.ends_with(".java") || path.ends_with(".ts") || path.ends_with(".tsx") {
                    if let Some(resolved) = resolve(path) {
                        if !files.contains(&resolved) { files.push(resolved); }
                    }
                }
            }
        }
        // TypeScript / vite: path/to/File.ts(line,col): error TS...
        // Must contain ): error or ): warning after the (line,col) part
        if line.contains("): error ") || line.contains("): warning ") {
            if let Some(paren) = line.find('(') {
                let path = line[..paren].trim_start();
                if path.ends_with(".ts") || path.ends_with(".tsx") {
                    if let Some(resolved) = resolve(path) {
                        if !files.contains(&resolved) { files.push(resolved); }
                    }
                }
            }
        }
        // ts-jest / tsc watch: path/to/File.ts:line:col - error TSxxxx: ...
        // Leading whitespace is common in jest output; "declared here" refs lack "- error TS".
        if line.contains(" - error TS") || line.contains(" - warning TS") {
            let trimmed = line.trim_start();
            if let Some(colon_pos) = trimmed.find(':') {
                let path = &trimmed[..colon_pos];
                if path.ends_with(".ts") || path.ends_with(".tsx") {
                    if let Some(resolved) = resolve(path) {
                        if !files.contains(&resolved) { files.push(resolved); }
                    }
                }
            }
        }
        // Jest module resolution: Cannot find module '...' from 'tests/Foo.ts'
        // The file after `from '` is the test file that contains the bad import.
        if line.contains("Cannot find module") {
            if let Some(from_pos) = line.find(" from '") {
                let rest = &line[from_pos + 7..];
                if let Some(end) = rest.find('\'') {
                    let path = rest[..end].trim();
                    if path.ends_with(".ts") || path.ends_with(".tsx") {
                        if let Some(resolved) = resolve(path) {
                            if !files.contains(&resolved) { files.push(resolved); }
                        }
                    }
                }
            }
        }
        // Jest stack trace: "    at Something (path/to/file.ts:line:col)"
        // Captures the source file where the runtime error originates.
        // Skips node_modules frames — only project source files are fixable.
        if line.trim_start().starts_with("at ") && line.contains(".ts:") && !line.contains("node_modules") {
            if let Some(open) = line.rfind('(') {
                if let Some(close) = line.rfind(')') {
                    let inner = &line[open + 1..close];
                    if let Some(colon) = inner.rfind(':') {
                        if let Some(colon2) = inner[..colon].rfind(':') {
                            let path = inner[..colon2].trim();
                            if path.ends_with(".ts") || path.ends_with(".tsx") {
                                if let Some(resolved) = resolve(path) {
                                    if !files.contains(&resolved) { files.push(resolved); }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    files
}
/// Detect Maven POM validation errors that fire before compilation.
/// Pattern: [ERROR] 'dependencies.dependency.version' for X:jar is missing. @ line N
/// These indicate an artifact that has no version and isn't managed by the parent BOM.
pub(crate) fn extract_pom_validation_errors(output: &str) -> Vec<String> {
    output.lines()
        .filter(|line| line.contains("[ERROR]") && line.contains("is missing."))
        .map(|line| line.trim().trim_start_matches("[ERROR]").trim().to_string())
        .collect()
}

/// Extract Maven artifact coordinates that could not be resolved.
/// Matches lines like:
///   [ERROR]     Could not find artifact com.example:foo:jar:1.0-SNAPSHOT
///   [ERROR] dependency: com.example:foo:jar:1.0-SNAPSHOT (compile)
pub(crate) fn extract_unresolvable_dependencies(output: &str) -> Vec<String> {
    let mut coords: Vec<String> = Vec::new();
    for line in output.lines() {
        if !line.contains("[ERROR]") {
            continue;
        }
        if line.contains("Could not find artifact") || line.contains("Could not resolve") {
            // Extract the coordinates — the groupId:artifactId:... token
            if let Some(coord) = line
                .split_whitespace()
                .find(|tok| tok.matches(':').count() >= 2)
            {
                // Strip trailing parenthetical e.g. "(compile)"
                let coord = coord.trim_end_matches(|c: char| c == ')' || c == '(');
                let coord = coord.trim();
                if !coord.is_empty() && !coords.contains(&coord.to_string()) {
                    coords.push(coord.to_string());
                }
            }
        }
    }
    coords
}

pub(crate) fn extract_missing_packages(output: &str) -> Vec<String> {
    let mut packages = Vec::new();
    for line in output.lines() {
        // javac: error: package javax.validation does not exist
        if line.contains("does not exist") {
            if let Some(start) = line.rfind("package ") {
                let rest = &line[start + 8..];
                if let Some(end) = rest.find(" does not exist") {
                    let pkg = rest[..end].trim().to_string();
                    if !pkg.is_empty() && !packages.contains(&pkg) {
                        packages.push(pkg);
                    }
                }
            }
        }
        // javac: error: cannot access X — class file for X not found
        if line.contains("class file for") && line.contains("not found") {
            if let Some(start) = line.rfind("class file for ") {
                let rest = &line[start + 15..];
                let cls = rest.split_whitespace().next().unwrap_or("").trim_end_matches(" not").to_string();
                if !cls.is_empty() && !packages.contains(&cls) {
                    packages.push(cls);
                }
            }
        }
    }
    packages
}

/// Normalise a path by resolving `.` and `..` components without touching the filesystem.
fn normalize_path(path: &std::path::Path) -> std::path::PathBuf {
    let mut out = std::path::PathBuf::new();
    for c in path.components() {
        match c {
            std::path::Component::ParentDir => { out.pop(); }
            std::path::Component::CurDir    => {}
            other                           => out.push(other),
        }
    }
    out
}

/// Parse `import … from '…'` lines and return the service-relative paths of any
/// local imports (i.e. those starting with `./` or `../`).
///
/// `file_path` is the project-relative path of the importing file (e.g.
/// `frontend/admin-portal/src/api/ProductApi.ts`). `service_dir` is the service
/// root (e.g. `frontend/admin-portal`). Returns paths like `src/components/ProductForm.tsx`.
pub(crate) fn parse_ts_imports(content: &str, file_path: &str, service_dir: &str) -> Vec<String> {
    let file_rel = std::path::Path::new(file_path)
        .strip_prefix(service_dir)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| file_path.to_string());

    let file_dir = std::path::Path::new(&file_rel)
        .parent()
        .unwrap_or(std::path::Path::new(""))
        .to_path_buf();

    let mut result: Vec<String> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("import ") && !trimmed.starts_with("export ") { continue; }

        // Extract the module specifier from: … from 'path' or "path"
        let module = if let Some(pos) = trimmed.rfind(" from ") {
            let rest = trimmed[pos + 6..].trim().trim_end_matches(';');
            rest.trim_matches('\'').trim_matches('"').to_string()
        } else {
            continue;
        };

        if !module.starts_with('.') { continue; } // skip node_modules

        let resolved = normalize_path(&file_dir.join(&module));
        let base = resolved.to_string_lossy();

        // Try the common TypeScript module resolution order.
        for candidate in &[
            format!("{}.ts",       base),
            format!("{}.tsx",      base),
            format!("{}/index.ts", base),
            format!("{}/index.tsx",base),
        ] {
            if candidate == &file_rel { continue; } // skip self-imports
            if std::path::Path::new(&format!("{}/{}", service_dir, candidate)).exists() {
                if !result.contains(candidate) {
                    result.push(candidate.clone());
                }
                break;
            }
        }
    }

    result
}

pub(crate) fn errors_for_file(output: &str, file_path: &str) -> String {
    // tsc emits relative paths from within the service dir; file_path may be the
    // fully-resolved absolute-or-project-relative path. Match on any suffix.
    let suffixes: Vec<&str> = {
        let mut v = vec![file_path];
        // strip leading service-dir prefix variants (frontend/admin-portal/, services/product-service/)
        for sep in &['/', '\\'] {
            if let Some(pos) = file_path.find(*sep) {
                // try each progressively shorter tail
                let mut rest = &file_path[pos + 1..];
                loop {
                    v.push(rest);
                    match rest.find(*sep) {
                        Some(p) => rest = &rest[p + 1..],
                        None => break,
                    }
                }
            }
        }
        v
    };
    output
        .lines()
        .filter(|l| suffixes.iter().any(|s| l.contains(s)))
        .collect::<Vec<_>>()
        .join("\n")
}
