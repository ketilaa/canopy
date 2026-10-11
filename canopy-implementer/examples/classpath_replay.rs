//! Replays the classpath import findings over the stored runs.
//!
//! For every recorded compile failure of the kind "cannot find symbol: class X" or "package P does not exist",
//! asks what the index would have said about the submission that failed; for every file of a step that passed,
//! asks whether it would have refused it (a false positive: that file compiled).
//!
//! The index of each run is built from the repository tag its batch used, exported with `git archive`, so the
//! dogfooding repository is never touched. Pre-registered in docs/design/classpath-class-index.md: the exact fix in
//! at least 80% of the failures, and no false positive on a passed step.
//!
//! usage: classpath_replay <runs dir> [<runs dir>...]   (a runs dir holds batch directories)
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use canopy_implementer::classpath::{import_findings, ClassIndex, Finding};
use canopy_implementer::java;
use serde_json::Value;

type Files = Vec<(String, String)>;

fn export_tag(repo: &Path, tag: &str) -> Option<PathBuf> {
    let dest = std::env::temp_dir().join(format!("canopy-replay-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dest).ok()?;
    let archive = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["archive", "--format=tar", tag])
        .output()
        .ok()?;
    if !archive.status.success() {
        return None;
    }
    let mut tar = Command::new("tar")
        .args(["-x", "-C"])
        .arg(&dest)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .ok()?;
    std::io::Write::write_all(&mut tar.stdin.take()?, &archive.stdout).ok()?;
    tar.wait().ok()?.success().then_some(dest)
}

fn project_of(created: &HashMap<String, Vec<String>>, extra: &Files) -> HashMap<String, Vec<String>> {
    let mut project = created.clone();
    for (path, _) in extra {
        if let (Some(q), Some(stem)) = (java::qualified_name(path), Path::new(path).file_stem()) {
            let entry = project.entry(stem.to_string_lossy().into_owned()).or_default();
            if !entry.contains(&q) {
                entry.push(q);
            }
        }
    }
    project
}

fn main() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../dogfooding/java-widgets");
    let mut indexes: HashMap<String, ClassIndex> = HashMap::new();
    let (mut runs, mut failures, mut eligible, mut named, mut exact) = (0, 0, 0, 0, 0);
    // The same, for the failures that involve a class the project does not own (a library or JDK class).
    let (mut lib_eligible, mut lib_named, mut lib_exact) = (0, 0, 0);
    let (mut lib_no_fix, mut lib_ambiguous) = (0, 0);
    let (mut files_checked, mut flagged) = (0, Vec::<String>::new());
    let mut unexplained: Vec<String> = Vec::new();
    let mut kinds: HashMap<&'static str, u32> = HashMap::new();

    for dir in std::env::args().skip(1) {
        let mut batches: Vec<PathBuf> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .collect();
        batches.sort();
        for batch in batches {
            let meta: Value = std::fs::read_to_string(batch.join("batch.json"))
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or(Value::Null);
            let (Some(tag), Some("widgets-planned")) = (meta["repository_tag"].as_str(), meta["task"].as_str()) else {
                continue;
            };
            if !indexes.contains_key(tag) {
                let Some(root) = export_tag(&repo, tag) else { continue };
                eprintln!("indexing {tag} ...");
                let index = ClassIndex::for_repository(&root);
                eprintln!("  resolved: {}", index.resolved());
                // REPLAY_DEBUG=Test,NotNull prints what the index holds for those simple names.
                for name in std::env::var("REPLAY_DEBUG")
                    .unwrap_or_default()
                    .split(',')
                    .filter(|n| !n.is_empty())
                {
                    eprintln!("    {name}: {:?}", index.candidates(name));
                }
                indexes.insert(tag.to_string(), index);
            }
            let index = &indexes[tag];
            if !index.resolved() {
                continue;
            }
            for run in std::fs::read_dir(&batch)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
            {
                let Ok(text) = std::fs::read_to_string(run.join("events.jsonl")) else {
                    continue;
                };
                runs += 1;
                let mut step = String::new();
                let mut last_submit: HashMap<String, Files> = HashMap::new();
                let mut created: HashMap<String, Vec<String>> = HashMap::new();
                for line in text.lines() {
                    let Ok(v) = serde_json::from_str::<Value>(line) else {
                        continue;
                    };
                    let ev = &v["event"];
                    match ev["type"].as_str() {
                        Some("step_started") => step = ev["step_id"].as_str().unwrap_or("").to_string(),
                        Some("tool_called") if ev["tool"] == "submit_files" && ev["outcome"] == "ok" => {
                            let args: Value =
                                serde_json::from_str(ev["arguments"].as_str().unwrap_or("")).unwrap_or(Value::Null);
                            let files = args["files"]
                                .as_array()
                                .map(|a| {
                                    a.iter()
                                        .map(|f| {
                                            (
                                                f["path"].as_str().unwrap_or("").to_string(),
                                                f["content"].as_str().unwrap_or("").to_string(),
                                            )
                                        })
                                        .collect()
                                })
                                .unwrap_or_default();
                            last_submit.insert(step.clone(), files);
                        }
                        Some("attempt_failed") if ev["stage"] == "compile" => {
                            failures += 1;
                            let feedback = ev["feedback"].as_str().unwrap_or("");
                            let classes: Vec<&str> = feedback
                                .lines()
                                .flat_map(|l| l.split(" | "))
                                .filter_map(|l| {
                                    l.trim()
                                        .strip_prefix("symbol:")
                                        .and_then(|s| s.trim().strip_prefix("class "))
                                })
                                .map(str::trim)
                                .collect();
                            let packages: Vec<&str> = feedback
                                .lines()
                                .flat_map(|l| l.split(" | "))
                                .filter_map(|l| {
                                    let (before, _) = l.split_once(" does not exist")?;
                                    before.rsplit_once("package ").map(|(_, p)| p.trim())
                                })
                                .collect();
                            if classes.is_empty() && packages.is_empty() {
                                *kinds.entry("compile failure of another kind").or_insert(0) += 1;
                                continue;
                            }
                            eligible += 1;
                            let files = last_submit.get(&step).cloned().unwrap_or_default();
                            let project = project_of(&created, &files);
                            let findings: Vec<Finding> = files
                                .iter()
                                .filter(|(p, _)| p.ends_with(".java"))
                                .flat_map(|(p, c)| import_findings(p, c, &project, index))
                                .collect();
                            let simple = |q: &str| q.rsplit('.').next().unwrap_or(q).to_string();
                            // What the existing audit (the project's classes and a short JDK table) already names.
                            let existing: Vec<String> = files
                                .iter()
                                .filter(|(p, _)| p.ends_with(".java"))
                                .flat_map(|(p, c)| java::missing_imports(p, c, &project))
                                .map(|q| simple(&q))
                                .collect();
                            let covers_class = |w: &str| {
                                existing.iter().any(|e| e == w)
                                    || findings.iter().any(|f| match f {
                                        Finding::Missing { class, .. } => class == w,
                                        Finding::WrongImport { imported, .. }
                                        | Finding::Absent { imported }
                                        | Finding::TestScope { imported } => simple(imported) == w,
                                    })
                            };
                            let covers_package = |p: &str| {
                                findings.iter().any(|f| match f {
                                    Finding::WrongImport { imported, .. }
                                    | Finding::Absent { imported }
                                    | Finding::TestScope { imported } => imported.starts_with(&format!("{p}.")),
                                    Finding::Missing { .. } => false,
                                })
                            };
                            let explained =
                                classes.iter().all(|w| covers_class(w)) && packages.iter().all(|p| covers_package(p));
                            let exact_all = findings.iter().all(|f| {
                                !matches!(f, Finding::Missing { .. } | Finding::WrongImport { .. })
                                    || f.exact_fix().is_some()
                            });
                            // A failure involves a library or JDK class when a wanted class is not the project's.
                            let library = !packages.is_empty() || classes.iter().any(|w| !project.contains_key(*w));
                            if library {
                                lib_eligible += 1;
                            }
                            if explained {
                                named += 1;
                                let no_fix = findings
                                    .iter()
                                    .any(|f| matches!(f, Finding::Absent { .. } | Finding::TestScope { .. }));
                                if exact_all && !no_fix {
                                    exact += 1;
                                }
                                if library {
                                    lib_named += 1;
                                    if no_fix {
                                        lib_no_fix += 1;
                                    } else if exact_all {
                                        lib_exact += 1;
                                    } else {
                                        lib_ambiguous += 1;
                                    }
                                }
                            } else {
                                unexplained.push(format!(
                                    "{} step {step}: classes {classes:?} packages {packages:?}; files {:?}; findings {:?}",
                                    run.display(),
                                    files.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>(),
                                    findings.iter().map(Finding::message).collect::<Vec<_>>()
                                ));
                            }
                        }
                        Some("step_finished") if ev["passed"] == true => {
                            let id = ev["step_id"].as_str().unwrap_or("");
                            for (path, content) in last_submit.get(id).into_iter().flatten() {
                                if !path.ends_with(".java") {
                                    continue;
                                }
                                files_checked += 1;
                                let project = project_of(&created, &last_submit.get(id).cloned().unwrap_or_default());
                                for finding in import_findings(path, content, &project, index) {
                                    flagged.push(format!("{}: {path}: {}", run.display(), finding.message()));
                                }
                            }
                            for (path, _) in last_submit.get(id).into_iter().flatten() {
                                if let (Some(q), Some(stem)) = (java::qualified_name(path), Path::new(path).file_stem())
                                {
                                    created.entry(stem.to_string_lossy().into_owned()).or_default().push(q);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    println!("runs: {runs}; compile failures: {failures}; of those about a missing class or package: {eligible}");
    for (kind, n) in &kinds {
        println!("  other: {kind}: {n}");
    }
    let pct = |n: i32| {
        if eligible == 0 {
            0.0
        } else {
            100.0 * f64::from(n) / f64::from(eligible)
        }
    };
    println!(
        "  the index explains every missing class or package in {named} ({:.0}%)",
        pct(named)
    );
    println!(
        "  and gives the exact import line for all of them in {exact} ({:.0}%)",
        pct(exact)
    );
    let lib_pct = |n: i32| {
        if lib_eligible == 0 {
            0.0
        } else {
            100.0 * f64::from(n) / f64::from(lib_eligible)
        }
    };
    println!("of those, failures that involve a library or JDK class: {lib_eligible}");
    println!(
        "  explained (by the index or the existing audit): {lib_named} ({:.0}%)",
        lib_pct(lib_named)
    );
    println!(
        "    with the exact import line: {lib_exact} ({:.0}%)",
        lib_pct(lib_exact)
    );
    println!(
        "    no import fixes it (not on the classpath, or test scope): {lib_no_fix} ({:.0}%)",
        lib_pct(lib_no_fix)
    );
    println!(
        "    several candidates, none chosen: {lib_ambiguous} ({:.0}%)",
        lib_pct(lib_ambiguous)
    );
    println!(
        "files of passed steps checked: {files_checked}; flagged (false positives): {}",
        flagged.len()
    );
    for f in &flagged {
        println!("  {f}");
    }
    for u in unexplained.iter().take(20) {
        println!("not explained: {u}");
    }
}
