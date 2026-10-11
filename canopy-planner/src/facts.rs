//! Facts about the repository the Planner computes instead of asking the model for them: the build, the
//! root package, the source roots and what is already there. Rendered as the standing context every step
//! of the plan carries.

use std::path::Path;

use canopy_implementer::java;

use std::collections::BTreeSet;

use crate::contract::{Consumer, Contract};

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
    /// The qualified names of the production classes (`src/main/java`), sorted.
    pub classes: Vec<String>,
    /// The qualified names of the test classes (`src/test/java`), sorted.
    pub test_classes: Vec<String>,
    /// The decisions the repository carries about itself (`.canopy/contract.yaml`), when it has them.
    pub contract: Option<Contract>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum FactsError {
    #[error("{0}")]
    Contract(String),
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
        let classes_under = |prefix: &str| -> Vec<String> {
            files
                .iter()
                .filter(|f| f.starts_with(prefix))
                .filter_map(|f| java::qualified_name_of_source(f))
                .collect()
        };
        let contract = Contract::read(root).map_err(FactsError::Contract)?;
        let classes = classes_under("src/main/java/");
        let test_classes = classes_under("src/test/java/");
        Ok(Self {
            build_tool: BuildTool::Maven,
            java_version: element(&pom, "java.version").map(str::to_string),
            spring_boot_version: spring_boot_version(&pom),
            dependencies: dependencies(&pom),
            root_package,
            files,
            classes,
            test_classes,
            contract,
        })
    }

    /// Whether a database or persistence library is on the classpath.
    pub fn has_database_library(&self) -> bool {
        const PREFIXES: &[&str] = &[
            "spring-boot-starter-data-",
            "spring-boot-starter-jdbc",
            "spring-boot-starter-orm",
        ];
        const NAMES: &[&str] = &[
            "jpa",
            "hibernate",
            "h2",
            "postgresql",
            "mysql",
            "mariadb",
            "sqlite",
            "mongodb",
            "jdbc",
        ];
        self.dependencies
            .iter()
            .any(|d| PREFIXES.iter().any(|p| d.artifact.starts_with(p)) || NAMES.iter().any(|n| d.artifact.contains(n)))
    }

    /// Whether a bean validation library (`javax.validation` or `jakarta.validation`) is on the classpath.
    pub fn has_validation_library(&self) -> bool {
        self.dependencies
            .iter()
            .any(|d| d.artifact.contains("validation") || d.artifact.contains("hibernate-validator"))
    }

    /// Where the repository disagrees with its own contract (empty without one). `root` is where it is.
    pub fn drift(&self, root: &Path) -> Vec<String> {
        self.contract
            .as_ref()
            .map_or_else(Vec::new, |c| c.audit(root, &self.files))
    }

    /// The facts as the standing context of every step. Facts that are not known are left out.
    pub fn render(&self) -> String {
        self.render_with(|contract| contract.render())
    }

    /// The facts for the planner's request: the derived facts and the contract entries meant for the planner.
    pub fn render_for_planner(&self) -> String {
        self.render_with(|contract| contract.render_for(Consumer::Planner, &BTreeSet::new()))
    }

    /// The facts for one implementation step: the derived facts and the contract entries for the step's layers.
    pub fn render_for_step(&self, layers: &BTreeSet<String>) -> String {
        self.render_with(|contract| contract.render_for(Consumer::Implementer, layers))
    }

    /// The layers of the given classes (qualified names): by name, and by annotation for a class whose source is
    /// already in the repository at `root`. Empty without a contract or when nothing says.
    pub fn layers_of_classes(&self, root: &Path, classes: &[String]) -> BTreeSet<String> {
        let Some(contract) = &self.contract else {
            return BTreeSet::new();
        };
        classes
            .iter()
            .flat_map(|class| {
                let source = std::fs::read_to_string(root.join(java::main_source_path(class))).ok();
                contract.layers_of(class, source.as_deref())
            })
            .collect()
    }

    fn render_with(&self, contract_lines: impl Fn(&Contract) -> Vec<String>) -> String {
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
        let boot_major = self
            .spring_boot_version
            .as_deref()
            .and_then(|v| v.split('.').next())
            .and_then(|major| major.parse::<u32>().ok());
        if boot_major.is_some_and(|major| major >= 3) {
            lines.push("Namespace: Spring Boot 3 uses jakarta.* packages, never javax.*.".to_string());
        }
        if !self.has_validation_library() {
            lines.push(
                "Validation: no bean validation library is available (neither javax.validation nor jakarta.validation)."
                    .to_string(),
            );
        }
        if !self.has_database_library() {
            lines.push("Storage: no database or JPA library is available.".to_string());
        }
        if let Some(contract) = &self.contract {
            lines.extend(contract_lines(contract));
        }
        for (label, classes) in [
            ("Existing classes", &self.classes),
            ("Existing test classes", &self.test_classes),
        ] {
            if !classes.is_empty() {
                let mut listed = classes
                    .iter()
                    .take(MAX_LISTED_FILES)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ");
                if classes.len() > MAX_LISTED_FILES {
                    listed.push_str(&format!(", and {} more", classes.len() - MAX_LISTED_FILES));
                }
                lines.push(format!("{label}: {listed}"));
            }
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
            text.contains("Existing classes: com.example.widgets.WidgetsApplication"),
            "{text}"
        );
    }

    #[test]
    fn without_a_database_library_the_facts_say_data_stays_in_memory() {
        let facts = RepoFacts::collect(scaffold().path()).unwrap();
        assert!(!facts.has_database_library());
        let text = facts.render();
        assert!(
            text.contains("Storage: no database or JPA library is available."),
            "{text}"
        );
    }

    #[test]
    fn a_database_or_jpa_dependency_removes_that_line() {
        for artifact in [
            "spring-boot-starter-data-jpa",
            "spring-boot-starter-jdbc",
            "h2",
            "postgresql",
            "spring-boot-starter-data-mongodb",
        ] {
            let dir = scaffold();
            let pom = POM.replace(
                "</dependencies>",
                &format!("<dependency><artifactId>{artifact}</artifactId></dependency></dependencies>"),
            );
            write(dir.path(), "pom.xml", &pom);
            let facts = RepoFacts::collect(dir.path()).unwrap();
            assert!(facts.has_database_library(), "{artifact}");
            assert!(!facts.render().contains("Storage:"), "{artifact}");
        }
    }

    #[test]
    fn existing_classes_are_listed_by_qualified_name_production_and_test_apart_and_without_paths() {
        let dir = scaffold();
        write(
            dir.path(),
            "src/main/java/com/example/widgets/domain/Widget.java",
            "package x;",
        );
        write(
            dir.path(),
            "src/test/java/com/example/widgets/domain/WidgetTest.java",
            "package x;",
        );
        write(dir.path(), "src/main/resources/application.properties", "a=b");
        let facts = RepoFacts::collect(dir.path()).unwrap();
        assert_eq!(
            facts.classes,
            [
                "com.example.widgets.WidgetsApplication",
                "com.example.widgets.domain.Widget"
            ]
        );
        assert_eq!(facts.test_classes, ["com.example.widgets.domain.WidgetTest"]);
        let text = facts.render();
        assert!(
            text.contains(
                "Existing classes: com.example.widgets.WidgetsApplication, com.example.widgets.domain.Widget"
            ),
            "{text}"
        );
        assert!(
            text.contains("Existing test classes: com.example.widgets.domain.WidgetTest"),
            "{text}"
        );
        assert!(!text.contains("Existing files"), "{text}");
        assert!(!text.contains(".java"), "symbols, not paths: {text}");
    }

    const CONTRACT: &str = "
stereotypes:
  - {suffix: Controller, annotation: RestController}
  - {suffix: Service, annotation: Service}
conventions:
  - {topic: storage, rule: keep data in memory}
";

    #[test]
    fn without_a_contract_the_facts_carry_no_stereotype_or_convention_lines() {
        let facts = RepoFacts::collect(scaffold().path()).unwrap();
        assert_eq!(facts.contract, None);
        let text = facts.render();
        assert!(!text.contains("Stereotypes") && !text.contains("Convention"), "{text}");
    }

    #[test]
    fn a_contract_in_the_repository_is_rendered_into_the_facts_after_the_derived_ones() {
        let dir = scaffold();
        write(dir.path(), ".canopy/contract.yaml", CONTRACT);
        let facts = RepoFacts::collect(dir.path()).unwrap();
        assert!(facts.contract.is_some());
        let text = facts.render();
        assert!(
            text.contains("Stereotypes by class name: *Controller is @RestController, *Service is @Service."),
            "{text}"
        );
        assert!(text.contains("Convention (storage): keep data in memory."), "{text}");
        assert!(
            text.find("Dependencies:").unwrap() < text.find("Stereotypes").unwrap()
                && text.find("Convention").unwrap() < text.find("Existing classes").unwrap(),
            "{text}"
        );
    }

    #[test]
    fn a_contract_that_is_not_valid_stops_the_facts_with_the_file_named() {
        let dir = scaffold();
        write(dir.path(), ".canopy/contract.yaml", "stereotypes: [broken");
        let e = RepoFacts::collect(dir.path()).unwrap_err();
        assert!(
            matches!(&e, FactsError::Contract(m) if m.contains(".canopy/contract.yaml")),
            "{e}"
        );
    }

    #[test]
    fn the_drift_of_the_repository_from_its_contract_is_reported_and_empty_without_one() {
        let dir = scaffold();
        write(
            dir.path(),
            "src/main/java/com/example/widgets/GadgetService.java",
            "package x;\nclass GadgetService {}\n",
        );
        assert!(RepoFacts::collect(dir.path()).unwrap().drift(dir.path()).is_empty());
        write(dir.path(), ".canopy/contract.yaml", CONTRACT);
        let drift = RepoFacts::collect(dir.path()).unwrap().drift(dir.path());
        assert_eq!(
            drift,
            ["com.example.widgets.GadgetService is named *Service but is not annotated @Service"]
        );
    }

    #[test]
    fn a_project_without_spring_boot_states_no_spring_convention() {
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
        let text = RepoFacts::collect(dir.path()).unwrap().render();
        assert!(!text.contains("Stereotypes"), "{text}");
    }

    #[test]
    fn without_a_validation_library_the_facts_say_to_check_values_in_code() {
        let facts = RepoFacts::collect(scaffold().path()).unwrap();
        assert!(!facts.has_validation_library());
        assert!(
            facts.render().contains(
                "Validation: no bean validation library is available (neither javax.validation nor jakarta.validation)."
            ),
            "{}",
            facts.render()
        );
    }

    #[test]
    fn a_validation_dependency_removes_that_line() {
        for artifact in [
            "spring-boot-starter-validation",
            "hibernate-validator",
            "jakarta.validation-api",
        ] {
            let dir = scaffold();
            let pom = POM.replace(
                "</dependencies>",
                &format!("<dependency><artifactId>{artifact}</artifactId></dependency></dependencies>"),
            );
            write(dir.path(), "pom.xml", &pom);
            let facts = RepoFacts::collect(dir.path()).unwrap();
            assert!(facts.has_validation_library(), "{artifact}");
            assert!(!facts.render().contains("Validation:"), "{artifact}");
        }
    }

    #[test]
    fn spring_boot_3_and_later_state_the_jakarta_namespace_and_earlier_versions_do_not() {
        let text = RepoFacts::collect(scaffold().path()).unwrap().render();
        assert!(
            text.contains("Namespace: Spring Boot 3 uses jakarta.* packages, never javax.*."),
            "{text}"
        );
        for (version, expected) in [("2.7.18", false), ("3.0.0", true), ("3.4.5", true), ("4.0.0", true)] {
            let dir = scaffold();
            write(dir.path(), "pom.xml", &POM.replace("3.4.5", version));
            let rendered = RepoFacts::collect(dir.path()).unwrap().render();
            assert_eq!(rendered.contains("Namespace:"), expected, "{version}: {rendered}");
        }
    }

    const SCOPED: &str = "
layers:
  - {name: controller, suffix: Controller, annotation: RestController}
  - {name: service, suffix: Service, annotation: Service}
rules:
  - id: controller-style
    applies_to: {consumer: implementer, layers: [controller]}
    content: Use constructor injection.
  - id: naming
    applies_to: {consumer: planner}
    content: Name a class after its layer.
  - id: everywhere
    content: Records have no getters.
";

    fn scoped_facts() -> (tempfile::TempDir, RepoFacts) {
        let dir = scaffold();
        write(dir.path(), ".canopy/contract.yaml", SCOPED);
        write(
            dir.path(),
            "src/main/java/com/example/widgets/WidgetManager.java",
            "package x;\n@Service\nclass WidgetManager {}\n",
        );
        let facts = RepoFacts::collect(dir.path()).unwrap();
        (dir, facts)
    }

    #[test]
    fn the_planner_view_has_the_derived_facts_and_only_the_contract_rules_meant_for_the_planner() {
        let (_dir, facts) = scoped_facts();
        let text = facts.render_for_planner();
        assert!(text.contains("Root package: com.example.widgets"), "{text}");
        assert!(
            text.contains("Stereotypes by class name: *Controller is @RestController, *Service is @Service."),
            "{text}"
        );
        assert!(
            text.contains("Convention (naming): Name a class after its layer."),
            "{text}"
        );
        assert!(
            text.contains("Convention (everywhere): Records have no getters."),
            "{text}"
        );
        assert!(!text.contains("controller-style"), "{text}");
    }

    #[test]
    fn a_step_view_has_the_derived_facts_and_only_the_rules_for_the_steps_layers() {
        let (_dir, facts) = scoped_facts();
        let layers: std::collections::BTreeSet<String> = ["service".to_string()].into();
        let text = facts.render_for_step(&layers);
        assert!(text.contains("Root package: com.example.widgets"), "{text}");
        assert!(
            text.contains("Stereotype: a class named *Service is @Service."),
            "{text}"
        );
        assert!(text.contains("Convention (everywhere):"), "{text}");
        assert!(
            !text.contains("controller-style") && !text.contains("RestController") && !text.contains("naming"),
            "{text}"
        );
        let controller: std::collections::BTreeSet<String> = ["controller".to_string()].into();
        assert!(facts
            .render_for_step(&controller)
            .contains("Convention (controller-style): Use constructor injection."));
        assert!(
            facts.render().contains("controller-style"),
            "the unscoped render still has everything"
        );
    }

    #[test]
    fn the_layers_of_planned_classes_come_from_names_and_those_of_existing_classes_also_from_annotations() {
        let (dir, facts) = scoped_facts();
        let layers = facts.layers_of_classes(
            dir.path(),
            &[
                "com.example.widgets.WidgetController".to_string(),
                "com.example.widgets.WidgetManager".to_string(),
                "com.example.widgets.NewManager".to_string(),
                "com.example.widgets.WidgetControllerTest".to_string(),
            ],
        );
        assert_eq!(
            layers.into_iter().collect::<Vec<_>>(),
            ["controller", "service", "test"],
            "WidgetManager is a service by its annotation; the not yet written NewManager is unclassified"
        );
        assert!(RepoFacts::collect(scaffold().path())
            .unwrap()
            .layers_of_classes(dir.path(), &["a.B".to_string()])
            .is_empty());
    }

    /// The contract baselines are hand-written files; this keeps them valid. Skipped without the dogfooding repo.
    #[test]
    fn the_widgets_contract_baselines_parse_render_and_differ_only_in_what_they_decide() {
        let (Some(memory), Some(jpa)) = (
            crate::testing::baseline("java-widgets", "baseline-02-widgets-contract-memory"),
            crate::testing::baseline("java-widgets", "baseline-03-widgets-contract-jpa"),
        ) else {
            return;
        };
        let memory = RepoFacts::collect(memory.path()).unwrap();
        let jpa = RepoFacts::collect(jpa.path()).unwrap();
        for facts in [&memory, &jpa] {
            let text = facts.render();
            assert!(
                text.contains("Stereotypes by class name: *Controller is @RestController"),
                "{text}"
            );
            assert!(text.contains("Convention (validation): check values in code"), "{text}");
            assert!(
                facts.drift(std::path::Path::new("/nonexistent")).is_empty(),
                "no classes to drift yet"
            );
        }
        assert!(memory.render().contains("Convention (storage): keep data in memory"));
        assert!(jpa
            .render()
            .contains("Convention (storage): persist with Spring Data JPA"));
        assert!(memory
            .render()
            .contains("Storage: no database or JPA library is available."));
        assert!(
            !jpa.render().contains("Storage:"),
            "JPA is on the classpath, so no absence line"
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
