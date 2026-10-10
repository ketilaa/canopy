//! What the harness knows about Java source: where a class lives, how to import it, which JDK classes a
//! model tends to forget. This is the one place that knows Java's rules, so a language registry can take
//! its place later without touching the callers.

use std::path::Path;

/// Classes the JDK ships that a model forgets to import. Only consulted when the repository has no class
/// of that name.
const JDK_CLASSES: &[(&str, &str)] = &[
    ("UUID", "java.util.UUID"),
    ("Optional", "java.util.Optional"),
    ("List", "java.util.List"),
    ("ArrayList", "java.util.ArrayList"),
    ("Map", "java.util.Map"),
    ("HashMap", "java.util.HashMap"),
    ("Set", "java.util.Set"),
    ("HashSet", "java.util.HashSet"),
    ("Objects", "java.util.Objects"),
    ("Collectors", "java.util.stream.Collectors"),
    ("ConcurrentHashMap", "java.util.concurrent.ConcurrentHashMap"),
    ("Instant", "java.time.Instant"),
    ("LocalDate", "java.time.LocalDate"),
    ("BigDecimal", "java.math.BigDecimal"),
];

/// The fully qualified name of a well-known JDK class, if `class` is one.
pub fn jdk_class(class: &str) -> Option<&'static str> {
    JDK_CLASSES.iter().find(|(name, _)| *name == class).map(|(_, full)| *full)
}

/// `src/main/java/com/example/greeting/Greeter.java` gives `com.example.greeting`.
pub fn package_of(path: &str) -> Option<String> {
    let inside = path.strip_prefix("src/main/java/")?;
    let (directory, _) = inside.rsplit_once('/')?;
    Some(directory.replace('/', "."))
}

/// `src/main/java/com/example/greeting/Greeter.java` gives `com.example.greeting.Greeter`.
pub fn qualified_name(path: &str) -> Option<String> {
    let package = package_of(path)?;
    let class = Path::new(path).file_stem()?.to_string_lossy().into_owned();
    Some(format!("{package}.{class}"))
}

/// The import lines for the classes of the given production paths, then for the JDK classes whose name
/// appears as a whole word in `text`. Each line once, in order of first appearance.
pub fn import_lines(paths: &[String], text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut add = |line: String| {
        if !lines.contains(&line) {
            lines.push(line);
        }
    };
    for path in paths {
        if let Some(name) = qualified_name(path) {
            add(format!("import {name};"));
        }
    }
    for (class, full) in JDK_CLASSES {
        if crate::gates::mentions_word(text, class) {
            add(format!("import {full};"));
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn the_package_comes_from_the_path_under_src_main_java() {
        assert_eq!(
            package_of("src/main/java/com/example/widgets/domain/Widget.java").as_deref(),
            Some("com.example.widgets.domain")
        );
        assert_eq!(package_of("src/test/java/com/example/WidgetTest.java"), None);
        assert_eq!(package_of("src/main/java/Loose.java"), None);
    }

    #[test]
    fn the_qualified_name_is_the_package_and_the_file_stem() {
        assert_eq!(
            qualified_name("src/main/java/com/example/widgets/domain/Widget.java").as_deref(),
            Some("com.example.widgets.domain.Widget")
        );
    }

    #[test]
    fn production_paths_give_import_lines_and_test_paths_none() {
        let lines = import_lines(
            &paths(&[
                "src/main/java/com/example/widgets/domain/Widget.java",
                "src/test/java/com/example/widgets/domain/WidgetTest.java",
            ]),
            "",
        );
        assert_eq!(lines, vec!["import com.example.widgets.domain.Widget;".to_string()]);
    }

    #[test]
    fn a_jdk_class_named_in_the_text_gets_its_import_line() {
        let lines = import_lines(&[], "Optional<Widget> findById(UUID id)");
        assert!(lines.contains(&"import java.util.Optional;".to_string()), "{lines:?}");
        assert!(lines.contains(&"import java.util.UUID;".to_string()), "{lines:?}");
        assert!(!lines.iter().any(|l| l.contains("List")), "{lines:?}");
    }

    #[test]
    fn a_jdk_name_inside_a_longer_word_does_not_count() {
        assert!(import_lines(&[], "OptionalThing and MyUUIDs").is_empty());
    }

    #[test]
    fn a_line_is_given_once() {
        let lines = import_lines(
            &paths(&["src/main/java/a/B.java", "src/main/java/a/B.java"]),
            "UUID UUID",
        );
        assert_eq!(lines.len(), 2, "{lines:?}");
    }
}
