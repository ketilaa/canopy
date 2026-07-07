use std::path::{Path, PathBuf};

use roots_core::{Language, Project};
use walkdir::WalkDir;

/// Detect project language from manifest filenames.
fn detect_language(filename: &str) -> Option<Language> {
    match filename {
        "pom.xml" => Some(Language::Java),
        "build.gradle" => Some(Language::Java),
        "settings.gradle" => Some(Language::Java),
        "build.gradle.kts" => Some(Language::Kotlin),
        "settings.gradle.kts" => Some(Language::Kotlin),
        "package.json" => Some(Language::TypeScript),
        "Cargo.toml" => Some(Language::Rust),
        _ => None,
    }
}

const MANIFESTS: &[&str] = &[
    "pom.xml",
    "build.gradle",
    "settings.gradle",
    "build.gradle.kts",
    "settings.gradle.kts",
    "package.json",
    "Cargo.toml",
];

/// Discover projects within a root directory.
/// A project directory that is nested inside another project directory is excluded.
pub fn discover_projects(root: &Path) -> Vec<Project> {
    // First pass: collect all manifest hits as (dir, language) candidates.
    let mut candidates: Vec<(PathBuf, Language)> = Vec::new();

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 { return true; }
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.') && name != "node_modules" && name != "target"
        })
    {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().is_file() {
            continue;
        }

        let file_name = entry.file_name().to_string_lossy().to_string();
        if !MANIFESTS.contains(&file_name.as_str()) {
            continue;
        }

        let dir = match entry.path().parent() {
            Some(d) => d.to_path_buf(),
            None => continue,
        };

        // Skip duplicate manifest in same directory (e.g. both build.gradle and settings.gradle)
        if candidates.iter().any(|(d, _)| d == &dir) {
            continue;
        }

        let Some(language) = detect_language(&file_name) else { continue };

        // For package.json, only claim TypeScript if .ts files exist in the tree
        if language == Language::TypeScript && !has_ts_files(&dir) {
            continue;
        }

        candidates.push((dir, language));
    }

    // Second pass: when same-language projects are nested, prefer the deeper (more specific)
    // one — the shallower one is a container (parent POM, monorepo root) not a real project.
    // Projects of different languages are always kept; they partition different file types
    // and do not conflict.
    candidates.sort_by_key(|(d, _)| d.components().count()); // deterministic order

    let projects: Vec<Project> = candidates
        .iter()
        .filter(|(dir, lang)| {
            // Drop a candidate when a same-language project is nested inside it.
            let has_same_lang_child = candidates.iter().any(|(other, other_lang)| {
                other != dir && other.starts_with(dir) && other_lang == lang
            });
            !has_same_lang_child
        })
        .map(|(dir, language)| {
            let name = dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "root".to_string());
            Project {
                name,
                path:         dir.to_string_lossy().to_string(),
                language:     language.clone(),
                workspace_id: String::new(),
            }
        })
        .collect();

    projects
}

fn has_ts_files(dir: &Path) -> bool {
    WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 { return true; }
            let name = e.file_name().to_string_lossy();
            name != "node_modules" && name != "dist" && name != "build" && !name.starts_with('.')
        })
        .any(|e| {
            e.ok().map_or(false, |e| {
                let name = e.file_name().to_string_lossy().to_string();
                e.file_type().is_file()
                    && (name.ends_with(".ts") || name.ends_with(".tsx"))
                    && !name.ends_with(".d.ts")
            })
        })
}

/// Collect source files for a project.
pub fn collect_source_files(project: &Project) -> Vec<PathBuf> {
    let root = Path::new(&project.path);
    let mut files = Vec::new();

    WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 { return true; }
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.')
                && name != "node_modules"
                && name != "dist"
                && name != "build"
                && name != "target"
        })
        .for_each(|e| {
            let e = match e {
                Ok(e) => e,
                Err(_) => return,
            };
            if !e.file_type().is_file() {
                return;
            }
            let path = e.path();
            let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("");

            let matches = match project.language {
                // Gradle projects use .kts DSL but source can be Java or Kotlin.
                // Collect both so a Java service under settings.gradle.kts isn't skipped.
                Language::Java | Language::Kotlin => {
                    ext == "java" || ext == "kt" || ext == "kts"
                }
                Language::TypeScript => {
                    (ext == "ts" || ext == "tsx")
                        && !path.to_string_lossy().ends_with(".d.ts")
                }
                Language::Rust => ext == "rs",
            };

            if matches {
                files.push(path.to_path_buf());
            }
        });

    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn tmpdir() -> TempDir {
        tempfile::tempdir().unwrap()
    }

    fn touch(dir: &Path, rel: &str) {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::File::create(p).unwrap();
    }

    #[test]
    fn discovers_java_maven_project() {
        let tmp = tmpdir();
        touch(tmp.path(), "myapp/pom.xml");
        touch(tmp.path(), "myapp/src/main/java/Foo.java");

        let projects = discover_projects(tmp.path());
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].language, Language::Java);
        assert_eq!(projects[0].name, "myapp");
    }

    #[test]
    fn discovers_kotlin_gradle_project() {
        let tmp = tmpdir();
        touch(tmp.path(), "svc/build.gradle.kts");
        touch(tmp.path(), "svc/src/main/kotlin/Foo.kt");

        let projects = discover_projects(tmp.path());
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].language, Language::Kotlin);
    }

    #[test]
    fn discovers_typescript_project_only_when_ts_files_present() {
        let tmp = tmpdir();
        touch(tmp.path(), "frontend/package.json");
        // No .ts files yet — should NOT discover
        let projects = discover_projects(tmp.path());
        assert!(projects.is_empty(), "should not discover TS project without .ts files");

        // Add a .ts file
        touch(tmp.path(), "frontend/src/index.ts");
        let projects = discover_projects(tmp.path());
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].language, Language::TypeScript);
    }

    #[test]
    fn prefers_nested_service_over_container_root() {
        let tmp = tmpdir();
        touch(tmp.path(), "monorepo/settings.gradle");
        touch(tmp.path(), "monorepo/service-a/build.gradle");

        let projects = discover_projects(tmp.path());
        // monorepo/ has a same-language Java child → dropped as container
        // service-a/ has no Java child → kept as the real project
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "service-a");
    }

    #[test]
    fn keeps_java_and_typescript_when_root_pom_present() {
        let tmp = tmpdir();
        // Root-level parent POM (common in Spring multi-module / accidental scaffold)
        touch(tmp.path(), "app/pom.xml");
        // Real Java service nested below
        touch(tmp.path(), "app/services/product-service/pom.xml");
        touch(tmp.path(), "app/services/product-service/src/Main.java");
        // TypeScript frontend
        touch(tmp.path(), "app/frontend/admin-portal/package.json");
        touch(tmp.path(), "app/frontend/admin-portal/src/App.tsx");

        let projects = discover_projects(tmp.path());
        // app/ has a same-language Java child → dropped
        // services/product-service/ has no Java child → kept
        // frontend/admin-portal/ has no TypeScript child → kept
        assert_eq!(projects.len(), 2);
        let names: Vec<&str> = projects.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"product-service"), "missing product-service, got: {names:?}");
        assert!(names.contains(&"admin-portal"), "missing admin-portal, got: {names:?}");
    }

    #[test]
    fn collects_java_source_files() {
        let tmp = tmpdir();
        touch(tmp.path(), "app/pom.xml");
        touch(tmp.path(), "app/src/main/java/Foo.java");
        touch(tmp.path(), "app/src/main/java/Bar.java");
        touch(tmp.path(), "app/src/main/java/skip.txt");

        let project = Project {
            name:         "app".into(),
            path:         tmp.path().join("app").to_string_lossy().to_string(),
            language:     Language::Java,
            workspace_id: String::new(),
        };
        let files = collect_source_files(&project);
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn collects_typescript_skips_declaration_files() {
        let tmp = tmpdir();
        touch(tmp.path(), "ui/package.json");
        touch(tmp.path(), "ui/src/app.ts");
        touch(tmp.path(), "ui/src/types.d.ts");
        touch(tmp.path(), "ui/src/component.tsx");

        let project = Project {
            name:         "ui".into(),
            path:         tmp.path().join("ui").to_string_lossy().to_string(),
            language:     Language::TypeScript,
            workspace_id: String::new(),
        };
        let files = collect_source_files(&project);
        // app.ts + component.tsx, NOT types.d.ts
        assert_eq!(files.len(), 2);
    }
}
