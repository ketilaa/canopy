//! The Implementer: one plan step in, files out, checked by the build.
//!
//! Each attempt asks the model for complete files under a schema, writes them, and has the
//! harness compile and test. A failed attempt is undone exactly and the next attempt is told what
//! went wrong. After the last attempt the workspace is as it was before the step began.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use crate::build::Build;
use crate::files::{GeneratedFile, GeneratedFiles};
use crate::llm::{Generator, StructuredRequest};
use crate::observe::{
    AttemptFailed, AttemptStarted, FilesWritten, RunEvent, RunObserver, Stage, StepFinished, StepStarted,
    WorkspaceRestored,
};
use crate::prompt;
use crate::snapshot::Snapshot;

/// One step of a plan: what to do, and the facts about the project it needs.
#[derive(Debug, Clone, PartialEq)]
pub struct StepRequest {
    pub id: String,
    pub instruction: String,
    pub context: String,
    /// Existing files the step reads or changes. Their current content is shown to the model.
    pub files: Vec<String>,
}

/// Why an attempt failed, and what the next attempt is shown.
#[derive(Debug, Clone, PartialEq)]
pub struct Failure {
    pub stage: Stage,
    pub feedback: String,
    /// The files the attempt wrote, when it got that far.
    pub files: Option<GeneratedFiles>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StepOutcome {
    Passed { attempts: u32 },
    Failed { attempts: u32, last_failure: String },
}

pub struct Implementer {
    generator: Box<dyn Generator>,
    build: Box<dyn Build>,
    observer: Arc<dyn RunObserver>,
    max_attempts: u32,
}

impl Implementer {
    pub fn new(
        generator: Box<dyn Generator>,
        build: Box<dyn Build>,
        observer: Arc<dyn RunObserver>,
        max_attempts: u32,
    ) -> Self {
        Self {
            generator,
            build,
            observer,
            max_attempts,
        }
    }

    pub fn run_step(&self, root: &Path, step: &StepRequest) -> StepOutcome {
        let started = Instant::now();
        self.observer.on(&RunEvent::StepStarted(StepStarted {
            step_id: step.id.clone(),
            instruction: step.instruction.clone(),
        }));

        let mut previous: Option<Failure> = None;
        for attempt in 1..=self.max_attempts {
            self.observer.on(&RunEvent::AttemptStarted(AttemptStarted {
                step_id: step.id.clone(),
                attempt,
            }));
            let mut snapshot = Snapshot::new(root);
            match self.attempt(root, step, attempt, previous.as_ref(), &mut snapshot) {
                Ok(()) => {
                    self.finish(step, true, attempt, started);
                    return StepOutcome::Passed { attempts: attempt };
                }
                Err(failure) => {
                    self.observer.on(&RunEvent::AttemptFailed(AttemptFailed {
                        step_id: step.id.clone(),
                        attempt,
                        stage: failure.stage,
                        feedback: failure.feedback.clone(),
                    }));
                    let restored = snapshot.restore();
                    if !restored.is_empty() {
                        self.observer.on(&RunEvent::WorkspaceRestored(WorkspaceRestored {
                            step_id: step.id.clone(),
                            attempt,
                            paths: restored,
                        }));
                    }
                    previous = Some(failure);
                }
            }
        }
        self.finish(step, false, self.max_attempts, started);
        StepOutcome::Failed {
            attempts: self.max_attempts,
            last_failure: previous.map(|f| f.feedback).unwrap_or_default(),
        }
    }

    fn finish(&self, step: &StepRequest, passed: bool, attempts: u32, started: Instant) {
        self.observer.on(&RunEvent::StepFinished(StepFinished {
            step_id: step.id.clone(),
            passed,
            attempts,
            elapsed_ms: started.elapsed().as_millis() as u64,
        }));
    }

    /// One attempt. Anything written is recorded in `snapshot`, so the caller can undo it.
    fn attempt(
        &self,
        root: &Path,
        step: &StepRequest,
        attempt: u32,
        previous: Option<&Failure>,
        snapshot: &mut Snapshot,
    ) -> Result<(), Failure> {
        let fail = |stage: Stage, feedback: String, files: Option<GeneratedFiles>| Failure { stage, feedback, files };

        let purpose = if attempt == 1 {
            "generate-files".to_string()
        } else {
            format!("retry-{}", attempt - 1)
        };
        let request = StructuredRequest {
            purpose,
            messages: prompt::messages(step, &read_current(root, &step.files), previous),
            schema_name: "files".into(),
            schema: GeneratedFiles::schema(),
            thinking: false,
        };
        let response = self
            .generator
            .complete_structured(&request)
            .map_err(|e| fail(Stage::Llm, e.to_string(), None))?;
        if response.finish_reason == "length" {
            return Err(fail(
                Stage::Truncated,
                "The answer hit the token limit before it was complete. Return fewer or shorter files.".into(),
                None,
            ));
        }
        let files = GeneratedFiles::parse(&response.content).map_err(|e| fail(Stage::Parse, e.to_string(), None))?;
        files
            .validate_paths()
            .map_err(|e| fail(Stage::Write, e.to_string(), Some(files.clone())))?;

        snapshot.capture(&files.paths());
        files
            .write_into(root)
            .map_err(|e| fail(Stage::Write, e.to_string(), Some(files.clone())))?;
        self.observer.on(&RunEvent::FilesWritten(FilesWritten {
            step_id: step.id.clone(),
            attempt,
            paths: files.paths(),
        }));

        let compiled = self.build.compile(root);
        if !compiled.success {
            return Err(fail(Stage::Compile, compiled.digest, Some(files)));
        }
        let tested = self.build.test(root);
        if !tested.success {
            return Err(fail(Stage::Test, tested.digest, Some(files)));
        }
        Ok(())
    }
}

/// The content of the step's existing files, right now. A file that does not exist yet is skipped, and so is
/// any path that would leave the workspace.
fn read_current(root: &Path, paths: &[String]) -> Vec<GeneratedFile> {
    paths
        .iter()
        .filter(|path| {
            Path::new(path.as_str())
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_)))
        })
        .filter_map(|path| {
            let content = std::fs::read_to_string(root.join(path)).ok()?;
            Some(GeneratedFile {
                path: path.clone(),
                content,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::BuildOutcome;
    use crate::llm::{LlmError, LlmResponse, Message, StructuredRequest};
    use crate::observe::{BuildKind, FailureKind, RecordingObserver, RunEvent};
    use std::collections::VecDeque;
    use std::sync::Mutex;

    struct ScriptedGenerator {
        replies: Mutex<VecDeque<Result<LlmResponse, LlmError>>>,
        prompts: Arc<Mutex<Vec<Vec<Message>>>>,
    }

    impl Generator for ScriptedGenerator {
        fn complete_structured(&self, request: &StructuredRequest) -> Result<LlmResponse, LlmError> {
            self.prompts.lock().unwrap().push(request.messages.clone());
            self.replies
                .lock()
                .unwrap()
                .pop_front()
                .expect("the script ran out of replies")
        }
    }

    struct ScriptedBuild {
        compiles: Mutex<VecDeque<BuildOutcome>>,
        tests: Mutex<VecDeque<BuildOutcome>>,
        compile_calls: Arc<Mutex<u32>>,
    }

    impl Build for ScriptedBuild {
        fn compile(&self, _root: &Path) -> BuildOutcome {
            *self.compile_calls.lock().unwrap() += 1;
            self.compiles
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| outcome(BuildKind::Compile, true, ""))
        }

        fn test(&self, _root: &Path) -> BuildOutcome {
            self.tests
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| outcome(BuildKind::Test, true, ""))
        }
    }

    fn outcome(kind: BuildKind, success: bool, digest: &str) -> BuildOutcome {
        BuildOutcome {
            kind,
            success,
            exit_code: Some(if success { 0 } else { 1 }),
            timed_out: false,
            elapsed_ms: 1,
            output: digest.into(),
            digest: digest.into(),
        }
    }

    fn answer(files: &[(&str, &str)]) -> Result<LlmResponse, LlmError> {
        let files: Vec<_> = files
            .iter()
            .map(|(p, c)| serde_json::json!({"path": p, "content": c}))
            .collect();
        reply(&serde_json::json!({ "files": files }).to_string(), "stop")
    }

    fn reply(content: &str, finish: &str) -> Result<LlmResponse, LlmError> {
        Ok(LlmResponse {
            call_id: 1,
            content: content.into(),
            finish_reason: finish.into(),
            prompt_tokens: 10,
            completion_tokens: 10,
        })
    }

    struct Rig {
        implementer: Implementer,
        observer: Arc<RecordingObserver>,
        prompts: Arc<Mutex<Vec<Vec<Message>>>>,
        compile_calls: Arc<Mutex<u32>>,
        dir: tempfile::TempDir,
    }

    fn rig(
        replies: Vec<Result<LlmResponse, LlmError>>,
        compiles: Vec<BuildOutcome>,
        tests: Vec<BuildOutcome>,
        max_attempts: u32,
    ) -> Rig {
        let observer = Arc::new(RecordingObserver::new());
        let prompts = Arc::new(Mutex::new(Vec::new()));
        let compile_calls = Arc::new(Mutex::new(0));
        let implementer = Implementer::new(
            Box::new(ScriptedGenerator {
                replies: Mutex::new(replies.into()),
                prompts: prompts.clone(),
            }),
            Box::new(ScriptedBuild {
                compiles: Mutex::new(compiles.into()),
                tests: Mutex::new(tests.into()),
                compile_calls: compile_calls.clone(),
            }),
            observer.clone(),
            max_attempts,
        );
        Rig {
            implementer,
            observer,
            prompts,
            compile_calls,
            dir: tempfile::tempdir().unwrap(),
        }
    }

    fn step() -> StepRequest {
        StepRequest {
            id: "s1".into(),
            instruction: "Create A.".into(),
            context: "pkg: x".into(),
            files: vec![],
        }
    }

    fn read(rig: &Rig, rel: &str) -> Option<String> {
        std::fs::read_to_string(rig.dir.path().join(rel)).ok()
    }

    fn tags(observer: &RecordingObserver) -> Vec<String> {
        observer
            .events()
            .iter()
            .filter_map(|e| match e {
                RunEvent::StepStarted(_) => Some("step_started".to_string()),
                RunEvent::AttemptStarted(a) => Some(format!("attempt_started:{}", a.attempt)),
                RunEvent::FilesWritten(f) => Some(format!("files_written:{}", f.attempt)),
                RunEvent::AttemptFailed(f) => {
                    Some(format!("attempt_failed:{}:{:?}", f.attempt, f.stage).to_lowercase())
                }
                RunEvent::WorkspaceRestored(r) => Some(format!("workspace_restored:{}", r.attempt)),
                RunEvent::StepFinished(f) => Some(format!(
                    "step_finished:{}:{}",
                    if f.passed { "passed" } else { "failed" },
                    f.attempts
                )),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_first_attempt_that_compiles_and_passes_finishes_the_step() {
        let rig = rig(vec![answer(&[("src/A.java", "class A {}")])], vec![], vec![], 3);
        let result = rig.implementer.run_step(rig.dir.path(), &step());
        assert_eq!(result, StepOutcome::Passed { attempts: 1 });
        assert_eq!(read(&rig, "src/A.java").as_deref(), Some("class A {}"));
        assert_eq!(
            tags(&rig.observer),
            [
                "step_started",
                "attempt_started:1",
                "files_written:1",
                "step_finished:passed:1"
            ]
        );
        let first_prompt = &rig.prompts.lock().unwrap()[0];
        assert!(first_prompt[1].content.contains("Create A.") && first_prompt[1].content.contains("pkg: x"));
    }

    #[test]
    fn a_compile_failure_is_undone_and_the_retry_is_told_the_errors_and_shown_its_previous_files() {
        let rig = rig(
            vec![
                answer(&[
                    ("src/A.java", "class A { broken }"),
                    ("src/Stale.java", "class Stale {}"),
                ]),
                answer(&[("src/A.java", "class A {}")]),
            ],
            vec![outcome(BuildKind::Compile, false, "src/A.java:1:11: boom")],
            vec![],
            3,
        );
        let result = rig.implementer.run_step(rig.dir.path(), &step());
        assert_eq!(result, StepOutcome::Passed { attempts: 2 });
        assert_eq!(read(&rig, "src/A.java").as_deref(), Some("class A {}"));
        assert_eq!(
            read(&rig, "src/Stale.java"),
            None,
            "a file only the failed attempt wrote must not linger"
        );
        let retry = &rig.prompts.lock().unwrap()[1];
        assert!(retry[1].content.contains("src/A.java:1:11: boom"));
        assert!(retry[1].content.contains("class A { broken }"));
        assert_eq!(
            tags(&rig.observer),
            [
                "step_started",
                "attempt_started:1",
                "files_written:1",
                "attempt_failed:1:compile",
                "workspace_restored:1",
                "attempt_started:2",
                "files_written:2",
                "step_finished:passed:2"
            ]
        );
    }

    #[test]
    fn a_test_failure_is_retried_with_the_failing_tests() {
        let rig = rig(
            vec![answer(&[("src/A.java", "v1")]), answer(&[("src/A.java", "v2")])],
            vec![],
            vec![outcome(BuildKind::Test, false, "ATest.x:5 expected 1 but was 2")],
            3,
        );
        assert_eq!(
            rig.implementer.run_step(rig.dir.path(), &step()),
            StepOutcome::Passed { attempts: 2 }
        );
        let retry = &rig.prompts.lock().unwrap()[1];
        assert!(retry[1].content.contains("The tests failed") && retry[1].content.contains("ATest.x:5"));
    }

    #[test]
    fn running_out_of_attempts_leaves_the_workspace_as_it_was_before_the_step() {
        let rig = rig(
            vec![
                answer(&[("src/Existing.java", "bad1"), ("src/New.java", "n1")]),
                answer(&[("src/Existing.java", "bad2")]),
            ],
            vec![
                outcome(BuildKind::Compile, false, "e1"),
                outcome(BuildKind::Compile, false, "e2"),
            ],
            vec![],
            2,
        );
        std::fs::create_dir_all(rig.dir.path().join("src")).unwrap();
        std::fs::write(rig.dir.path().join("src/Existing.java"), "original").unwrap();

        let result = rig.implementer.run_step(rig.dir.path(), &step());
        assert_eq!(
            result,
            StepOutcome::Failed {
                attempts: 2,
                last_failure: "e2".into()
            }
        );
        assert_eq!(read(&rig, "src/Existing.java").as_deref(), Some("original"));
        assert_eq!(read(&rig, "src/New.java"), None);
        let t = tags(&rig.observer);
        assert_eq!(t.iter().filter(|x| x.starts_with("workspace_restored")).count(), 2);
        assert_eq!(t.last().unwrap(), "step_finished:failed:2");
    }

    #[test]
    fn an_answer_cut_off_at_the_token_limit_is_a_truncation_failure_not_a_parse_failure() {
        let rig = rig(
            vec![reply("{\"files\":[{\"pa", "length"), answer(&[("src/A.java", "ok")])],
            vec![],
            vec![],
            3,
        );
        assert_eq!(
            rig.implementer.run_step(rig.dir.path(), &step()),
            StepOutcome::Passed { attempts: 2 }
        );
        assert!(tags(&rig.observer).contains(&"attempt_failed:1:truncated".to_string()));
        assert!(rig.prompts.lock().unwrap()[1][1].content.contains("cut off"));
    }

    #[test]
    fn an_answer_that_is_not_valid_json_is_a_parse_failure_and_is_retried() {
        let rig = rig(
            vec![reply("sure! here you go", "stop"), answer(&[("src/A.java", "ok")])],
            vec![],
            vec![],
            3,
        );
        assert_eq!(
            rig.implementer.run_step(rig.dir.path(), &step()),
            StepOutcome::Passed { attempts: 2 }
        );
        assert!(tags(&rig.observer).contains(&"attempt_failed:1:parse".to_string()));
    }

    #[test]
    fn an_unsafe_path_writes_nothing_and_never_reaches_the_build() {
        let rig = rig(
            vec![
                answer(&[("src/ok.txt", "x"), ("../escape.txt", "y")]),
                answer(&[("src/A.java", "ok")]),
            ],
            vec![],
            vec![],
            3,
        );
        assert_eq!(
            rig.implementer.run_step(rig.dir.path(), &step()),
            StepOutcome::Passed { attempts: 2 }
        );
        assert!(tags(&rig.observer).contains(&"attempt_failed:1:write".to_string()));
        assert_eq!(read(&rig, "src/ok.txt"), None);
        assert_eq!(
            *rig.compile_calls.lock().unwrap(),
            1,
            "only the second attempt reached the build"
        );
    }

    #[test]
    fn a_failed_model_call_counts_as_an_attempt_and_is_retried() {
        let err = LlmError {
            call_id: 1,
            kind: FailureKind::Timeout,
            status: None,
            message: "timed out".into(),
        };
        let rig = rig(vec![Err(err), answer(&[("src/A.java", "ok")])], vec![], vec![], 3);
        assert_eq!(
            rig.implementer.run_step(rig.dir.path(), &step()),
            StepOutcome::Passed { attempts: 2 }
        );
        assert!(tags(&rig.observer).contains(&"attempt_failed:1:llm".to_string()));
    }

    #[test]
    fn the_step_finished_event_carries_the_time_spent() {
        let rig = rig(vec![answer(&[("src/A.java", "ok")])], vec![], vec![], 3);
        rig.implementer.run_step(rig.dir.path(), &step());
        let finished = rig.observer.events().into_iter().find_map(|e| match e {
            RunEvent::StepFinished(f) => Some(f),
            _ => None,
        });
        let finished = finished.unwrap();
        assert!(finished.passed && finished.attempts == 1 && finished.step_id == "s1");
    }
    fn step_changing(paths: &[&str]) -> StepRequest {
        StepRequest {
            files: paths.iter().map(|p| p.to_string()).collect(),
            ..step()
        }
    }

    #[test]
    fn files_the_step_names_are_shown_to_the_model_as_they_are_on_disk_and_missing_ones_are_skipped() {
        let rig = rig(vec![answer(&[("src/Existing.java", "v2")])], vec![], vec![], 3);
        std::fs::create_dir_all(rig.dir.path().join("src")).unwrap();
        std::fs::write(rig.dir.path().join("src/Existing.java"), "class Existing { /* v1 */ }").unwrap();

        let step = step_changing(&["src/Existing.java", "src/NotYet.java"]);
        assert_eq!(
            rig.implementer.run_step(rig.dir.path(), &step),
            StepOutcome::Passed { attempts: 1 }
        );
        let prompt = &rig.prompts.lock().unwrap()[0][1].content;
        assert!(
            prompt.contains("### src/Existing.java") && prompt.contains("/* v1 */"),
            "{prompt}"
        );
        assert!(
            !prompt.contains("NotYet"),
            "a file that does not exist yet has nothing to show"
        );
    }

    #[test]
    fn a_retry_shows_the_original_files_not_the_failed_attempts_version_of_them() {
        let rig = rig(
            vec![
                answer(&[("src/Existing.java", "class Existing { /* broken v2 */ }")]),
                answer(&[("src/Existing.java", "v3")]),
            ],
            vec![outcome(BuildKind::Compile, false, "boom")],
            vec![],
            3,
        );
        std::fs::create_dir_all(rig.dir.path().join("src")).unwrap();
        std::fs::write(rig.dir.path().join("src/Existing.java"), "class Existing { /* v1 */ }").unwrap();

        rig.implementer
            .run_step(rig.dir.path(), &step_changing(&["src/Existing.java"]));
        let retry = &rig.prompts.lock().unwrap()[1][1].content;
        let current = &retry[retry.find("## Current files").unwrap()..retry.find("## Your previous answer").unwrap()];
        assert!(
            current.contains("/* v1 */") && !current.contains("broken v2"),
            "{current}"
        );
        assert!(
            retry.contains("broken v2"),
            "the failed attempt is still shown, as the previous answer"
        );
    }
}
