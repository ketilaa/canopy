//! Fixtures shared by the tests: a scaffold repository on disk and a real baseline taken from git.

use std::fs;
use std::path::Path;

pub const POM: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
  <parent>
<groupId>org.springframework.boot</groupId>
<artifactId>spring-boot-starter-parent</artifactId>
<version>3.4.5</version>
  </parent>
  <properties>
<java.version>21</java.version>
  </properties>
  <dependencies>
<dependency>
  <groupId>org.springframework.boot</groupId>
  <artifactId>spring-boot-starter-web</artifactId>
</dependency>
<!-- <dependency><artifactId>commented-out</artifactId></dependency> -->
<dependency>
  <groupId>org.springframework.boot</groupId>
  <artifactId>spring-boot-starter-test</artifactId>
  <scope>test</scope>
</dependency>
  </dependencies>
</project>
"#;

pub const APPLICATION: &str = "package com.example.widgets;\n\nimport org.springframework.boot.autoconfigure.SpringBootApplication;\n\n@SpringBootApplication\npublic class WidgetsApplication {}\n";

pub fn write(root: &std::path::Path, path: &str, text: &str) {
    let p = root.join(path);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

pub fn scaffold() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "pom.xml", POM);
    write(
        dir.path(),
        "src/main/java/com/example/widgets/WidgetsApplication.java",
        APPLICATION,
    );
    write(dir.path(), "src/test/java/com/example/widgets/.gitkeep", "");
    dir
}

/// A repository at a tag of the dogfooding repository, exported with `git archive` so that repository is
/// never touched. `None` when that repository is not on this machine (it is git-ignored and personal).
pub fn baseline(repo_name: &str, tag: &str) -> Option<tempfile::TempDir> {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dogfooding")
        .join(repo_name);
    if !repo.join(".git").exists() {
        return None;
    }
    let dir = tempfile::tempdir().unwrap();
    let archive = std::process::Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["archive", "--format=tar", tag])
        .output()
        .unwrap();
    if !archive.status.success() {
        return None;
    }
    let mut tar = std::process::Command::new("tar")
        .args(["-x", "-C"])
        .arg(dir.path())
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    std::io::Write::write_all(&mut tar.stdin.take().unwrap(), &archive.stdout).unwrap();
    assert!(tar.wait().unwrap().success());
    Some(dir)
}
