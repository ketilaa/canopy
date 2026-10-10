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

#[cfg(test)]
mod tests {
    use super::*;

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
}
