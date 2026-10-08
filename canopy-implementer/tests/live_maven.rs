//! Runs real Maven. Skipped unless `CANOPY_IMPLEMENTER_MVN_TEST=1`.
//!
//!   CANOPY_IMPLEMENTER_MVN_TEST=1 cargo test -p canopy-implementer --test live_maven -- --nocapture
//!
//! Checks that the digest holds up against real Maven/javac/Surefire output, not just the samples
//! the unit tests were written from.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use canopy_implementer::build::{Build, CommandBuild};
use canopy_implementer::observe::NoopObserver;

const POM: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
  <modelVersion>4.0.0</modelVersion>
  <parent>
    <groupId>org.springframework.boot</groupId>
    <artifactId>spring-boot-starter-parent</artifactId>
    <version>3.4.5</version>
    <relativePath/>
  </parent>
  <groupId>com.example</groupId>
  <artifactId>demo</artifactId>
  <version>0.0.1</version>
  <properties><java.version>21</java.version></properties>
  <dependencies>
    <dependency>
      <groupId>org.springframework.boot</groupId>
      <artifactId>spring-boot-starter-test</artifactId>
      <scope>test</scope>
    </dependency>
  </dependencies>
</project>
"#;

const GREETER: &str = "package com.example;\npublic class Greeter {\n    public String greet(String name) { return \"Hello, \" + name + \"!\"; }\n}\n";
const GREETER_BROKEN: &str = "package com.example;\npublic class Greeter {\n    public String greet(String name) { return \"Hello, \" + nme + \"!\"; }\n}\n";
const TEST: &str = "package com.example;\nimport org.junit.jupiter.api.Test;\nimport static org.junit.jupiter.api.Assertions.assertEquals;\nclass GreeterTest {\n    @Test void greetsByName() { assertEquals(\"Hello, Ada!\", new Greeter().greet(\"Ada\")); }\n}\n";
const TEST_WRONG: &str = "package com.example;\nimport org.junit.jupiter.api.Test;\nimport static org.junit.jupiter.api.Assertions.assertEquals;\nclass GreeterTest {\n    @Test void greetsByName() { assertEquals(\"Hello Ada\", new Greeter().greet(\"Ada\")); }\n}\n";

fn project(greeter: &str, test: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let write = |rel: &str, content: &str| {
        let path = dir.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    };
    write("pom.xml", POM);
    write("src/main/java/com/example/Greeter.java", greeter);
    write("src/test/java/com/example/GreeterTest.java", test);
    dir
}

fn enabled() -> bool {
    let on = std::env::var("CANOPY_IMPLEMENTER_MVN_TEST").as_deref() == Ok("1");
    if !on {
        eprintln!("skipped: set CANOPY_IMPLEMENTER_MVN_TEST=1 to run against real Maven");
    }
    on
}

fn maven() -> CommandBuild {
    CommandBuild::maven(Duration::from_secs(240), Arc::new(NoopObserver))
}

fn show(root: &Path, label: &str, digest: &str) {
    eprintln!("--- {label} digest ({}) ---\n{digest}", root.display());
}

#[test]
fn a_correct_project_compiles_and_passes() {
    if !enabled() {
        return;
    }
    let dir = project(GREETER, TEST);
    let compile = maven().compile(dir.path());
    assert!(compile.success, "{}", compile.output);
    let test = maven().test(dir.path());
    assert!(test.success, "{}", test.output);
    assert_eq!(test.digest, "");
}

#[test]
fn a_compile_error_is_digested_to_file_line_message_and_symbol() {
    if !enabled() {
        return;
    }
    let dir = project(GREETER_BROKEN, TEST);
    let outcome = maven().compile(dir.path());
    assert!(!outcome.success);
    show(dir.path(), "compile", &outcome.digest);
    assert!(
        outcome.digest.starts_with("src/main/java/com/example/Greeter.java:3:"),
        "{}",
        outcome.digest
    );
    assert!(outcome.digest.contains("cannot find symbol"), "{}", outcome.digest);
    assert!(outcome.digest.contains("symbol:"), "{}", outcome.digest);
    assert_eq!(
        outcome.digest.matches("cannot find symbol").count(),
        1,
        "errors must not be repeated"
    );
    assert!(
        outcome.digest.lines().count() <= 6,
        "digest should be short: {}",
        outcome.digest
    );
}

#[test]
fn a_failing_test_is_digested_to_the_test_and_the_summary() {
    if !enabled() {
        return;
    }
    let dir = project(GREETER, TEST_WRONG);
    assert!(maven().compile(dir.path()).success);
    let outcome = maven().test(dir.path());
    assert!(!outcome.success);
    show(dir.path(), "test", &outcome.digest);
    assert!(
        outcome.digest.contains("GreeterTest.greetsByName"),
        "{}",
        outcome.digest
    );
    assert!(outcome.digest.contains("Tests run:"), "{}", outcome.digest);
    assert!(
        outcome.digest.lines().count() <= 6,
        "digest should be short: {}",
        outcome.digest
    );
}
