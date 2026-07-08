use canopy_core::{ServiceEntry, ServicesRegistry};

pub(crate) fn scan_project_files(services: &ServicesRegistry) -> Vec<String> {
    let mut files = Vec::new();
    for service in &services.services {
        if service.component_type.as_deref() == Some("infrastructure") { continue; }
        let base = match service.component_type.as_deref().unwrap_or("service") {
            "frontend" => format!("frontend/{}", service.name),
            _ => format!("services/{}", service.name),
        };
        collect_files(std::path::Path::new(&base), &mut files);
    }
    files
}

/// Walk a JVM service's src/main/java tree to find *Application.java and read its package declaration.
/// Returns the fully qualified base package as declared in the file (e.g. "com.example.canopyecommerce.product_service").
pub(crate) fn detect_service_package(service_name: &str) -> Option<String> {
    let root = std::path::Path::new("services")
        .join(service_name)
        .join("src/main/java");
    find_application_package(&root)
}

fn find_application_package(dir: &std::path::Path) -> Option<String> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if path.is_dir() {
            if let Some(pkg) = find_application_package(&path) {
                return Some(pkg);
            }
        } else if name.ends_with("Application.java") {
            let content = std::fs::read_to_string(&path).ok()?;
            for line in content.lines() {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("package ") {
                    return Some(rest.trim_end_matches(';').trim().to_string());
                }
            }
        }
    }
    None
}

pub(crate) fn collect_files(dir: &std::path::Path, out: &mut Vec<String>) {
    let skip = ["target", "node_modules", ".git", ".roots"];
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if skip.contains(&name) { continue; }
            if path.is_dir() {
                collect_files(&path, out);
            } else {
                out.push(path.to_string_lossy().to_string());
            }
        }
    }
}
/// Returns a compile-only command (no test execution) for the service's build tool.
pub(crate) fn compile_command_for_service(service: &ServiceEntry, _service_dir: &str) -> String {
    let tech = service.technology.as_deref().unwrap_or("");
    match canopy_core::TechFamily::classify(tech) {
        canopy_core::TechFamily::JvmGradle => "./gradlew compileTestJava".to_string(),
        canopy_core::TechFamily::JvmMaven => "./mvnw test-compile -B".to_string(),
        // Use locally installed tsc — avoids picking up stale global packages via npx
        canopy_core::TechFamily::Npm => "./node_modules/.bin/tsc --noEmit".to_string(),
    }
}

/// Run `npm install` if the service directory has a package.json but no node_modules.
pub(crate) fn ensure_npm_installed(service_dir: &str) {
    if std::path::Path::new(&format!("{service_dir}/package.json")).exists()
        && !std::path::Path::new(&format!("{service_dir}/node_modules")).exists()
    {
        println!("  running npm install in {service_dir}...");
        let _ = crate::shell::npm_install(service_dir, &[], false);
    }
}

/// Returns a test command scoped to a single test class.
pub(crate) fn test_class_command_for_service(service: &ServiceEntry, test_class: &str) -> String {
    let tech = service.technology.as_deref().unwrap_or("");
    match canopy_core::TechFamily::classify(tech) {
        canopy_core::TechFamily::JvmGradle => format!("./gradlew test --tests '*.{}'", test_class),
        canopy_core::TechFamily::JvmMaven => format!("./mvnw test -Dtest={} -B", test_class),
        canopy_core::TechFamily::Npm => format!("npm test -- --testPathPatterns={} --watchAll=false", test_class),
    }
}
pub(crate) fn test_command_for_service(service: &ServiceEntry, service_dir: &str) -> String {
    let tech = service.technology.as_deref().unwrap_or("");
    match canopy_core::TechFamily::classify(tech) {
        canopy_core::TechFamily::JvmGradle => return "./gradlew test".to_string(),
        canopy_core::TechFamily::JvmMaven => return "./mvnw test -B".to_string(),
        canopy_core::TechFamily::Npm => {}
    }
    let is_frontend = service.component_type.as_deref() == Some("frontend");
    let pkg_path = format!("{service_dir}/package.json");
    if let Ok(pkg) = std::fs::read_to_string(&pkg_path) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&pkg) {
            let scripts = json.get("scripts");
            if scripts.and_then(|s| s.get("test")).is_some() {
                // --watchAll=false is a CRA/Vite flag; plain jest doesn't accept it
                return if is_frontend {
                    "npm test -- --watchAll=false".to_string()
                } else {
                    "npx jest --forceExit".to_string()
                };
            }
            if scripts.and_then(|s| s.get("build")).is_some() {
                return "npm run build".to_string();
            }
        }
    }
    "npx tsc --noEmit".to_string()
}
pub(crate) fn read_installed_deps(service_dir: &str, tech: &str) -> Vec<String> {
    match canopy_core::TechFamily::classify(tech) {
        canopy_core::TechFamily::JvmGradle => read_gradle_deps(service_dir),
        canopy_core::TechFamily::JvmMaven => read_pom_deps(service_dir),
        canopy_core::TechFamily::Npm => read_package_json_deps(service_dir),
    }
}

fn read_pom_deps(service_dir: &str) -> Vec<String> {
    let path = std::path::Path::new(service_dir).join("pom.xml");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let mut deps: Vec<String> = Vec::new();
    let mut group_id = String::new();
    let mut artifact_id = String::new();
    for line in content.lines() {
        let t = line.trim();
        if let Some(v) = tag_value(t, "groupId") { group_id = v; }
        if let Some(v) = tag_value(t, "artifactId") { artifact_id = v; }
        if t == "</dependency>" {
            if !group_id.is_empty() && !artifact_id.is_empty() {
                deps.push(format!("{}:{}", group_id, artifact_id));
            }
            group_id.clear();
            artifact_id.clear();
        }
    }
    deps.sort();
    deps.dedup();
    deps
}

fn tag_value(line: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = line.find(&open)? + open.len();
    let end = line.find(&close)?;
    if start <= end { Some(line[start..end].trim().to_string()) } else { None }
}

fn read_gradle_deps(service_dir: &str) -> Vec<String> {
    // Support both build.gradle (Groovy) and build.gradle.kts (Kotlin DSL)
    let candidates = ["build.gradle.kts", "build.gradle"];
    let content = candidates.iter()
        .find_map(|name| std::fs::read_to_string(std::path::Path::new(service_dir).join(name)).ok())
        .unwrap_or_default();

    let configs = ["implementation", "testImplementation", "api",
                   "compileOnly", "runtimeOnly", "annotationProcessor"];
    let mut deps: Vec<String> = Vec::new();
    for line in content.lines() {
        let t = line.trim();
        let coord = configs.iter().find_map(|cfg| {
            let prefix = format!("{cfg} ");
            if !t.starts_with(&prefix) { return None; }
            // Extract the string inside quotes or parentheses+quotes
            let rest = t[prefix.len()..].trim();
            let inner = rest.trim_start_matches('(').trim_end_matches(')')
                .trim_matches('"').trim_matches('\'');
            // Strip version — keep groupId:artifactId only
            let parts: Vec<&str> = inner.split(':').collect();
            if parts.len() >= 2 { Some(format!("{}:{}", parts[0], parts[1])) } else { None }
        });
        if let Some(c) = coord { deps.push(c); }
    }
    deps.sort();
    deps.dedup();
    deps
}

fn read_package_json_deps(service_dir: &str) -> Vec<String> {
    let path = std::path::Path::new(service_dir).join("package.json");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    let mut pkgs: Vec<String> = Vec::new();
    for key in &["dependencies", "devDependencies"] {
        if let Some(obj) = json.get(key).and_then(|v| v.as_object()) {
            pkgs.extend(obj.keys().cloned());
        }
    }
    pkgs.sort();
    pkgs.dedup();
    pkgs
}
pub(crate) fn scan_service_source_files(service_dir: &str) -> Vec<String> {
    let mut all = Vec::new();
    collect_files(std::path::Path::new(service_dir), &mut all);
    let base = std::path::Path::new(service_dir);
    all.into_iter()
        .filter(|f| f.ends_with(".java") || f.ends_with(".ts") || f.ends_with(".tsx"))
        .map(|f| {
            std::path::Path::new(&f)
                .strip_prefix(base)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or(f)
        })
        .collect()
}
