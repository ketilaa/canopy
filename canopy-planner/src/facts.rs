//! Facts about the repository the Planner computes instead of asking the model for them: the build, the
//! root package, the source roots and what is already there. Rendered as the standing context every step
//! of the plan carries.

use std::path::Path;

use canopy_implementer::java;

/// How many existing files the rendered facts list before they are cut.
pub const MAX_LISTED_FILES: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildTool {
    Maven,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Dependency {
    pub artifact: String,
    pub test_only: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RepoFacts {
    pub build_tool: BuildTool,
    pub java_version: Option<String>,
    pub spring_boot_version: Option<String>,
    pub dependencies: Vec<Dependency>,
    pub root_package: String,
    /// Every file under `src`, relative to the repository root, sorted. Placeholders are left out.
    pub files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum FactsError {
    #[error("the repository has no pom.xml; only Maven repositories are supported")]
    NoBuildFile,
    #[error("no class annotated @SpringBootApplication under src/main/java, so there is no root package")]
    NoApplicationClass,
}

impl RepoFacts {
    pub fn collect(root: &Path) -> Result<Self, FactsError> {
        let pom = std::fs::read_to_string(root.join("pom.xml")).map_err(|_| FactsError::NoBuildFile)?;
        let pom = strip_comments(&pom);
        let mut files = Vec::new();
        walk(root, &root.join("src"), &mut files);
        files.sort();
        let root_package = application_package(root, &files).ok_or(FactsError::NoApplicationClass)?;
        Ok(Self {
            build_tool: BuildTool::Maven,
            java_version: element(&pom, "java.version").map(str::to_string),
            spring_boot_version: spring_boot_version(&pom),
            dependencies: dependencies(&pom),
            root_package,
            files,
        })
    }

    /// The facts as the standing context of every step. Facts that are not known are left out.
    pub fn render(&self) -> String {
        let directory = self.root_package.replace('.', "/");
        let mut build = match self.build_tool {
            BuildTool::Maven => "Maven".to_string(),
        };
        if let Some(java) = &self.java_version {
            build.push_str(&format!(", Java {java}"));
        }
        if let Some(boot) = &self.spring_boot_version {
            build.push_str(&format!("; Spring Boot {boot}"));
        }
        let mut lines = vec![
            format!("Root package: {}", self.root_package),
            format!("Main sources: src/main/java/{directory}/"),
            format!("Test sources: src/test/java/{directory}/"),
            format!("Build: {build}"),
        ];
        if !self.dependencies.is_empty() {
            let listed: Vec<String> = self
                .dependencies
                .iter()
                .map(|d| match d.test_only {
                    true => format!("{} (test)", d.artifact),
                    false => d.artifact.clone(),
                })
                .collect();
            lines.push(format!("Dependencies: {}", listed.join(", ")));
        }
        if !self.files.is_empty() {
            let shown = self.files.iter().take(MAX_LISTED_FILES).cloned().collect::<Vec<_>>();
            let mut listed = shown.join(", ");
            if self.files.len() > MAX_LISTED_FILES {
                listed.push_str(&format!(", and {} more", self.files.len() - MAX_LISTED_FILES));
            }
            lines.push(format!("Existing files: {listed}"));
        }
        lines.join("\n")
    }
}

fn walk(root: &Path, dir: &Path, files: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_symlink() {
            continue;
        }
        let path = entry.path();
        if kind.is_dir() {
            walk(root, &path, files);
        } else if entry.file_name() != ".gitkeep" {
            if let Ok(relative) = path.strip_prefix(root) {
                files.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
}

/// The package of the first (by path) production class annotated `@SpringBootApplication`.
fn application_package(root: &Path, files: &[String]) -> Option<String> {
    files
        .iter()
        .filter(|f| f.starts_with("src/main/java/") && f.ends_with(".java"))
        .find(|f| std::fs::read_to_string(root.join(f)).is_ok_and(|text| text.contains("@SpringBootApplication")))
        .and_then(|f| java::package_of(f))
}

fn strip_comments(xml: &str) -> String {
    let mut out = String::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        match rest[start..].find("-->") {
            Some(end) => rest = &rest[start + end + 3..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// The text of the first `<tag>…</tag>` in `xml`.
fn element<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&format!("</{tag}>"))? + start;
    Some(xml[start..end].trim())
}

/// `xml` without the first `<tag>…</tag>` block, whole.
fn without(xml: &str, tag: &str) -> String {
    let (open, close) = (format!("<{tag}>"), format!("</{tag}>"));
    match (xml.find(&open), xml.find(&close)) {
        (Some(start), Some(end)) if end > start => format!("{}{}", &xml[..start], &xml[end + close.len()..]),
        _ => xml.to_string(),
    }
}

fn spring_boot_version(pom: &str) -> Option<String> {
    let parent = element(pom, "parent")?;
    (element(parent, "artifactId")? == "spring-boot-starter-parent")
        .then(|| element(parent, "version").map(str::to_string))
        .flatten()
}

fn dependencies(pom: &str) -> Vec<Dependency> {
    let pom = without(&without(pom, "parent"), "dependencyManagement");
    let Some(block) = element(&pom, "dependencies") else {
        return vec![];
    };
    block
        .split("<dependency>")
        .skip(1)
        .filter_map(|chunk| {
            let chunk = chunk.split("</dependency>").next()?;
            Some(Dependency {
                artifact: element(chunk, "artifactId")?.to_string(),
                test_only: element(chunk, "scope") == Some("test"),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{scaffold, write, POM};

    #[test]
    fn the_build_is_read_from_the_pom() {
        let facts = RepoFacts::collect(scaffold().path()).unwrap();
        assert_eq!(facts.build_tool, BuildTool::Maven);
        assert_eq!(facts.java_version.as_deref(), Some("21"));
        assert_eq!(facts.spring_boot_version.as_deref(), Some("3.4.5"));
    }

    #[test]
    fn dependencies_keep_their_scope_and_commented_out_ones_are_ignored() {
        let facts = RepoFacts::collect(scaffold().path()).unwrap();
        assert_eq!(
            facts.dependencies,
            [
                Dependency {
                    artifact: "spring-boot-starter-web".into(),
                    test_only: false
                },
                Dependency {
                    artifact: "spring-boot-starter-test".into(),
                    test_only: true
                },
            ]
        );
    }

    #[test]
    fn the_root_package_is_the_package_of_the_spring_boot_application_class() {
        let facts = RepoFacts::collect(scaffold().path()).unwrap();
        assert_eq!(facts.root_package, "com.example.widgets");
    }

    #[test]
    fn the_file_tree_lists_files_under_src_sorted_without_placeholders() {
        let dir = scaffold();
        write(
            dir.path(),
            "src/main/java/com/example/widgets/domain/Widget.java",
            "package x;",
        );
        let facts = RepoFacts::collect(dir.path()).unwrap();
        assert_eq!(
            facts.files,
            [
                "src/main/java/com/example/widgets/WidgetsApplication.java",
                "src/main/java/com/example/widgets/domain/Widget.java",
            ]
        );
    }

    #[test]
    fn a_repository_without_a_pom_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(RepoFacts::collect(dir.path()), Err(FactsError::NoBuildFile));
    }

    #[test]
    fn a_repository_without_a_spring_boot_application_class_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "pom.xml", POM);
        write(
            dir.path(),
            "src/main/java/com/example/Plain.java",
            "package com.example;\nclass Plain {}\n",
        );
        assert_eq!(RepoFacts::collect(dir.path()), Err(FactsError::NoApplicationClass));
    }

    #[test]
    fn a_pom_without_a_java_version_or_parent_leaves_those_facts_out() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "pom.xml",
            "<project><dependencies></dependencies></project>",
        );
        write(
            dir.path(),
            "src/main/java/a/App.java",
            "package a;\n@SpringBootApplication\nclass App {}\n",
        );
        let facts = RepoFacts::collect(dir.path()).unwrap();
        assert_eq!(facts.java_version, None);
        assert_eq!(facts.spring_boot_version, None);
        assert!(facts.dependencies.is_empty());
        assert!(
            !facts.render().contains("None"),
            "an unknown fact is left out, not printed"
        );
    }

    #[test]
    fn the_rendered_facts_state_what_the_hand_written_context_stated() {
        let text = RepoFacts::collect(scaffold().path()).unwrap().render();
        assert!(text.contains("Root package: com.example.widgets"), "{text}");
        assert!(
            text.contains("Main sources: src/main/java/com/example/widgets/"),
            "{text}"
        );
        assert!(
            text.contains("Test sources: src/test/java/com/example/widgets/"),
            "{text}"
        );
        assert!(text.contains("Maven, Java 21"), "{text}");
        assert!(text.contains("Spring Boot 3.4.5"), "{text}");
        assert!(text.contains("spring-boot-starter-web"), "{text}");
        assert!(text.contains("spring-boot-starter-test (test)"), "{text}");
        assert!(
            text.contains("src/main/java/com/example/widgets/WidgetsApplication.java"),
            "{text}"
        );
    }

    #[test]
    fn a_long_file_list_is_cut_and_says_how_many_were_left_out() {
        let dir = scaffold();
        for i in 0..(MAX_LISTED_FILES + 5) {
            write(
                dir.path(),
                &format!("src/main/java/com/example/widgets/C{i:03}.java"),
                "package x;",
            );
        }
        let text = RepoFacts::collect(dir.path()).unwrap().render();
        assert!(text.contains("and 6 more"), "{text}");
    }

    /// The real baseline, taken from git without touching the dogfooding repository. Skipped when that
    /// repository is not on this machine (it is git-ignored and personal).
    #[test]
    fn the_widgets_baseline_gives_the_facts_the_hand_written_context_gave() {
        let Some(dir) = crate::testing::baseline("java-widgets", "baseline-00-widgets") else {
            return;
        };
        let facts = RepoFacts::collect(dir.path()).unwrap();
        assert_eq!(facts.root_package, "com.example.widgets");
        assert_eq!(facts.java_version.as_deref(), Some("21"));
        assert_eq!(facts.spring_boot_version.as_deref(), Some("3.4.5"));
        assert!(facts
            .dependencies
            .iter()
            .any(|d| d.artifact == "spring-boot-starter-web" && !d.test_only));
        assert!(facts.files.iter().any(|f| f.ends_with("WidgetsApplication.java")));
    }
}
