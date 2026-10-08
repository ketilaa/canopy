//! Running the build, and boiling its output down to what a model can act on.
//!
//! The harness runs the checks; the model is never asked whether to. Every run is reported as a
//! [`CommandRan`](crate::observe::CommandRan) event carrying the full output, while the model only
//! ever sees the digest.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::observe::{BuildKind, CommandRan, RunEvent, RunObserver};

#[derive(Debug, Clone, PartialEq)]
pub struct BuildOutcome {
    pub kind: BuildKind,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub elapsed_ms: u64,
    /// Full combined stdout and stderr.
    pub output: String,
    /// The errors worth showing a model: compiler errors with their symbol lines, or the failing
    /// tests. Empty on success.
    pub digest: String,
}

pub trait Build: Send + Sync {
    /// Compiles main and test sources without running any test.
    fn compile(&self, root: &Path) -> BuildOutcome;
    /// Runs the test suite.
    fn test(&self, root: &Path) -> BuildOutcome;
}

/// Runs an external build tool (Maven by default) in the workspace, with a timeout.
pub struct CommandBuild {
    program: String,
    compile_args: Vec<String>,
    test_args: Vec<String>,
    timeout: Duration,
    observer: Arc<dyn RunObserver>,
}

impl CommandBuild {
    pub fn maven(timeout: Duration, observer: Arc<dyn RunObserver>) -> Self {
        Self::custom("mvn", &["-B", "test-compile"], &["-B", "test"], timeout, observer)
    }

    pub fn custom(
        program: &str,
        compile_args: &[&str],
        test_args: &[&str],
        timeout: Duration,
        observer: Arc<dyn RunObserver>,
    ) -> Self {
        Self {
            program: program.into(),
            compile_args: compile_args.iter().map(|s| s.to_string()).collect(),
            test_args: test_args.iter().map(|s| s.to_string()).collect(),
            timeout,
            observer,
        }
    }

    fn run(&self, kind: BuildKind, root: &Path) -> BuildOutcome {
        use std::io::Read;
        use std::os::unix::process::CommandExt;
        use std::process::{Command, Stdio};
        use std::time::Instant;

        let args = match kind {
            BuildKind::Compile => &self.compile_args,
            BuildKind::Test => &self.test_args,
        };
        let command_line = format!("{} {}", self.program, args.join(" "));
        let started = Instant::now();

        let mut command = Command::new(&self.program);
        command
            .args(args)
            .current_dir(root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Own process group, so a timeout can kill the build tool and everything it forked.
            .process_group(0);

        let (exit_code, timed_out, output) = match command.spawn() {
            Err(e) => (None, false, format!("could not start {}: {e}", self.program)),
            Ok(mut child) => {
                // Drain the pipes while waiting: reading only after exit would block on a full
                // pipe, and a blocking read would ignore the timeout.
                let drain = |stream: Option<Box<dyn Read + Send>>| {
                    std::thread::spawn(move || {
                        let mut bytes = Vec::new();
                        if let Some(mut s) = stream {
                            let _ = s.read_to_end(&mut bytes);
                        }
                        String::from_utf8_lossy(&bytes).into_owned()
                    })
                };
                let stdout = drain(child.stdout.take().map(|s| Box::new(s) as Box<dyn Read + Send>));
                let stderr = drain(child.stderr.take().map(|s| Box::new(s) as Box<dyn Read + Send>));

                let (code, timed_out) = loop {
                    match child.try_wait() {
                        Ok(Some(status)) => break (status.code(), false),
                        Ok(None) if started.elapsed() >= self.timeout => {
                            let _ = Command::new("kill")
                                .args(["-KILL", &format!("-{}", child.id())])
                                .status();
                            let _ = child.kill();
                            let _ = child.wait();
                            break (None, true);
                        }
                        Ok(None) => std::thread::sleep(Duration::from_millis(20)),
                        Err(_) => break (None, false),
                    }
                };
                let mut output = stdout.join().unwrap_or_default();
                output.push_str(&stderr.join().unwrap_or_default());
                (code, timed_out, output)
            }
        };

        let success = exit_code == Some(0) && !timed_out;
        let digest = if success {
            String::new()
        } else if timed_out {
            format!(
                "The build did not finish within {} seconds and was stopped.\n{}",
                self.timeout.as_secs().max(1),
                tail(&output)
            )
        } else {
            digest(kind, &output, root)
        };
        let elapsed_ms = started.elapsed().as_millis() as u64;
        self.observer.on(&RunEvent::CommandRan(CommandRan {
            kind,
            command: command_line,
            success,
            exit_code,
            timed_out,
            elapsed_ms,
            output: output.clone(),
        }));
        BuildOutcome {
            kind,
            success,
            exit_code,
            timed_out,
            elapsed_ms,
            output,
            digest,
        }
    }
}

impl Build for CommandBuild {
    fn compile(&self, root: &Path) -> BuildOutcome {
        self.run(BuildKind::Compile, root)
    }

    fn test(&self, root: &Path) -> BuildOutcome {
        self.run(BuildKind::Test, root)
    }
}

/// Reduces raw Maven output to the lines a model can act on. Paths are made relative to `root`.
pub fn digest(kind: BuildKind, output: &str, root: &Path) -> String {
    let compile = compile_errors(output, root);
    let digest = match kind {
        BuildKind::Compile => compile,
        BuildKind::Test => {
            let failures = test_failures(output);
            if failures.is_empty() {
                compile
            } else {
                failures
            }
        }
    };
    if digest.is_empty() {
        tail(output)
    } else {
        digest
    }
}

fn strip_error_prefix(line: &str) -> &str {
    line.strip_prefix("[ERROR] ").unwrap_or(line)
}

/// javac errors as `path:line:col: message` plus their `symbol:`/`location:` lines. Maven prints
/// every error twice (once in the compiler section, once in the failure report), so each block is
/// kept once.
fn compile_errors(output: &str, root: &Path) -> String {
    // Tools print the canonical path, which differs from the given root when it is a symlink.
    let mut root_prefixes = vec![format!("{}/", root.display())];
    if let Ok(canonical) = std::fs::canonicalize(root) {
        root_prefixes.push(format!("{}/", canonical.display()));
    }
    let lines: Vec<&str> = output.lines().map(strip_error_prefix).collect();
    let mut blocks: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if let Some(mut block) = error_head(lines[i], &root_prefixes) {
            while i + 1 < lines.len() && is_detail(lines[i + 1]) {
                i += 1;
                block.push('\n');
                block.push_str(lines[i].trim_end());
            }
            if !blocks.contains(&block) {
                blocks.push(block);
            }
        }
        i += 1;
    }
    blocks.join("\n")
}

fn error_head(line: &str, root_prefixes: &[String]) -> Option<String> {
    let idx = line.find(".java:[")?;
    let path = &line[..idx + ".java".len()];
    let (coords, message) = line[idx + ".java:[".len()..].split_once("] ")?;
    let (row, col) = coords.split_once(',')?;
    let path = root_prefixes
        .iter()
        .find_map(|prefix| path.strip_prefix(prefix.as_str()))
        .unwrap_or(path);
    Some(format!("{path}:{row}:{col}: {message}"))
}

fn is_detail(line: &str) -> bool {
    let line = line.trim_start();
    ["symbol:", "location:", "required:", "found:", "reason:"]
        .iter()
        .any(|k| line.starts_with(k))
}

/// The failing tests listed under Surefire's `Failures:` / `Errors:`, then its summary line.
fn test_failures(output: &str) -> String {
    let mut items: Vec<String> = Vec::new();
    let mut in_list = false;
    let mut summary: Option<String> = None;
    for line in output.lines() {
        let text = strip_error_prefix(line);
        let header = matches!(text.trim(), "Failures:" | "Errors:");
        if header {
            in_list = true;
        } else if in_list && line.starts_with("[ERROR]   ") {
            items.push(text.trim().to_string());
        } else {
            in_list = false;
        }
        if text.starts_with("Tests run:") && !text.contains("Time elapsed") {
            summary = Some(text.trim().to_string());
        }
    }
    if items.is_empty() {
        return String::new();
    }
    if let Some(summary) = summary {
        items.push(summary);
    }
    items.join("\n")
}

fn tail(output: &str) -> String {
    let lines: Vec<&str> = output.lines().collect();
    lines[lines.len().saturating_sub(30)..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observe::{RecordingObserver, RunEvent};
    use std::time::Instant;

    const COMPILE_ERROR: &str = "\
[INFO] Scanning for projects...
[INFO] --- compiler:3.13.0:compile (default-compile) @ demo ---
[INFO] -------------------------------------------------------------
[ERROR] COMPILATION ERROR :
[INFO] -------------------------------------------------------------
[ERROR] /work/demo/src/main/java/com/example/Greeter.java:[5,16] cannot find symbol
  symbol:   variable nme
  location: class com.example.Greeter
[INFO] 1 error
[INFO] -------------------------------------------------------------
[INFO] BUILD FAILURE
[ERROR] Failed to execute goal compiler:3.13.0:compile (default-compile) on project demo: Compilation failure
[ERROR] /work/demo/src/main/java/com/example/Greeter.java:[5,16] cannot find symbol
[ERROR]   symbol:   variable nme
[ERROR]   location: class com.example.Greeter
[ERROR] -> [Help 1]
";

    const TEST_FAILURE: &str = "\
[INFO] Running com.example.GreeterTest
[ERROR] Tests run: 2, Failures: 1, Errors: 0, Skipped: 0, Time elapsed: 0.05 s <<< FAILURE! -- in com.example.GreeterTest
[ERROR] com.example.GreeterTest.greetsByName -- Time elapsed: 0.01 s <<< FAILURE!
org.opentest4j.AssertionFailedError: expected: <Hello, Ada!> but was: <Hello Ada>
\tat org.junit.jupiter.api.AssertionFailureBuilder.build(AssertionFailureBuilder.java:151)
\tat com.example.GreeterTest.greetsByName(GreeterTest.java:11)
[INFO]
[INFO] Results:
[INFO]
[ERROR] Failures:
[ERROR]   GreeterTest.greetsByName:11 expected: <Hello, Ada!> but was: <Hello Ada>
[INFO]
[ERROR] Tests run: 2, Failures: 1, Errors: 0, Skipped: 0
[INFO] BUILD FAILURE
[ERROR] -> [Help 1]
";

    fn root() -> &'static Path {
        Path::new("/work/demo")
    }

    #[test]
    fn compile_digest_keeps_each_error_once_with_its_symbol_lines_and_a_relative_path() {
        let d = digest(BuildKind::Compile, COMPILE_ERROR, root());
        assert_eq!(
            d,
            "src/main/java/com/example/Greeter.java:5:16: cannot find symbol\n  symbol:   variable nme\n  location: class com.example.Greeter"
        );
    }

    #[test]
    fn test_digest_lists_the_failing_tests_and_the_summary() {
        let d = digest(BuildKind::Test, TEST_FAILURE, root());
        assert_eq!(
            d,
            "GreeterTest.greetsByName:11 expected: <Hello, Ada!> but was: <Hello Ada>\nTests run: 2, Failures: 1, Errors: 0, Skipped: 0"
        );
    }

    #[test]
    fn a_test_run_that_fails_to_compile_digests_the_compile_errors() {
        let d = digest(BuildKind::Test, COMPILE_ERROR, root());
        assert!(
            d.starts_with("src/main/java/com/example/Greeter.java:5:16: cannot find symbol"),
            "{d}"
        );
    }

    #[test]
    fn unrecognised_output_falls_back_to_its_tail() {
        let output = (1..=100).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        let d = digest(BuildKind::Compile, &output, root());
        assert!(d.ends_with("line 100"));
        assert!(!d.contains("line 10\n"));
        assert!(d.lines().count() <= 30);
    }

    #[test]
    fn paths_are_relative_even_when_the_tool_prints_the_canonical_path_of_a_symlinked_root() {
        let dir = tempfile::tempdir().unwrap();
        // On macOS the temp dir is a symlink (/var -> /private/var) and Maven prints the real path.
        let canonical = std::fs::canonicalize(dir.path()).unwrap();
        let output = format!(
            "[ERROR] {}/src/main/java/A.java:[1,2] cannot find symbol
",
            canonical.display()
        );
        assert_eq!(
            digest(BuildKind::Compile, &output, dir.path()),
            "src/main/java/A.java:1:2: cannot find symbol"
        );
    }

    fn build(
        program: &str,
        compile: &[&str],
        test: &[&str],
        timeout_ms: u64,
    ) -> (CommandBuild, Arc<RecordingObserver>) {
        let observer = Arc::new(RecordingObserver::new());
        (
            CommandBuild::custom(
                program,
                compile,
                test,
                Duration::from_millis(timeout_ms),
                observer.clone(),
            ),
            observer,
        )
    }

    #[test]
    fn a_passing_command_succeeds_and_captures_both_streams() {
        let dir = tempfile::tempdir().unwrap();
        let (b, _) = build("sh", &["-c", "echo out; echo err 1>&2"], &["-c", "true"], 5000);
        let outcome = b.compile(dir.path());
        assert!(outcome.success);
        assert_eq!(outcome.exit_code, Some(0));
        assert!(!outcome.timed_out);
        assert!(
            outcome.output.contains("out") && outcome.output.contains("err"),
            "{}",
            outcome.output
        );
        assert_eq!(outcome.digest, "");
    }

    #[test]
    fn a_failing_command_reports_its_exit_code_and_a_digest() {
        let dir = tempfile::tempdir().unwrap();
        let script = format!("cat <<'EOF'\n{TEST_FAILURE}EOF\nexit 1");
        let (b, _) = build("sh", &["-c", "true"], &["-c", &script], 5000);
        let outcome = b.test(dir.path());
        assert!(!outcome.success);
        assert_eq!(outcome.exit_code, Some(1));
        assert!(
            outcome.digest.contains("GreeterTest.greetsByName:11"),
            "{}",
            outcome.digest
        );
    }

    #[test]
    fn the_command_runs_in_the_workspace() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("marker.txt"), "x").unwrap();
        let (b, _) = build("sh", &["-c", "test -f marker.txt"], &["-c", "true"], 5000);
        assert!(b.compile(dir.path()).success);
    }

    #[test]
    fn a_hung_command_is_killed_at_the_timeout_children_included() {
        let dir = tempfile::tempdir().unwrap();
        let (b, _) = build("sh", &["-c", "sleep 30"], &["-c", "true"], 300);
        let started = Instant::now();
        let outcome = b.compile(dir.path());
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "took {:?}",
            started.elapsed()
        );
        assert!(outcome.timed_out);
        assert!(!outcome.success);
    }

    #[test]
    fn every_run_is_reported_with_its_full_output_and_timing() {
        let dir = tempfile::tempdir().unwrap();
        let (b, observer) = build("sh", &["-c", "echo hello; sleep 0.05"], &["-c", "true"], 5000);
        b.compile(dir.path());
        let events = observer.events();
        assert_eq!(events.len(), 1);
        let RunEvent::CommandRan(ran) = &events[0] else {
            panic!("{:?}", events[0])
        };
        assert_eq!(ran.kind, BuildKind::Compile);
        assert!(ran.success && !ran.timed_out);
        assert_eq!(ran.exit_code, Some(0));
        assert!(ran.output.contains("hello"));
        assert!(ran.elapsed_ms >= 40, "elapsed {}", ran.elapsed_ms);
        assert!(ran.command.starts_with("sh "), "{}", ran.command);
    }
}
