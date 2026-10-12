//! The Implementer: one plan step in, files out, checked by the build.
//!
//! Each attempt asks the model for complete files under a schema, writes them, and has the
//! harness compile and test. A failed attempt is undone exactly and the next attempt is told what
//! went wrong. After the last attempt the workspace is as it was before the step began.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use crate::build::Build;
use crate::files::{GeneratedFile, GeneratedFiles, Submission};
use crate::gates;
use crate::hints;
use crate::llm::{ChatMessage, Generator, StructuredRequest, ToolGenerator, ToolRequest};
use crate::observe::{
    AttemptFailed, AttemptStarted, FilesWritten, RunEvent, RunObserver, Stage, StepFinished, StepStarted, ToolCalled,
    ToolOutcome, WorkspaceRestored,
};
use crate::orchestrator::StepRunner;
use crate::prompt;
use crate::snapshot::Snapshot;
use crate::tools::ToolBox;

/// The files a step is allowed to touch, decided by the planner. An empty scope means the step is
/// unscoped and the model finds its own way.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Scope {
    /// Existing files the step changes. Their content is shown to the model.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modify: Vec<String>,
    /// New files the step must create, at exactly these paths.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub create: Vec<String>,
    /// Existing files the step depends on. Shown to the model, not changeable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub read_only: Vec<String>,
}

impl Scope {
    /// A scope that names nothing to change or create does not constrain the step.
    pub fn is_empty(&self) -> bool {
        self.modify.is_empty() && self.create.is_empty()
    }
}

/// One step of a plan: what to do, and the facts about the project it needs.
#[derive(Debug, Clone, PartialEq)]
pub struct StepRequest {
    pub id: String,
    pub instruction: String,
    pub context: String,
    /// Existing files the step reads or changes. Their current content is shown to the model.
    pub files: Vec<String>,
    pub kind: StepKind,
    pub scope: Scope,
}

/// What kind of change a step is, declared by the plan and never by the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    /// New or changed behaviour: production code must come with a test that mentions it.
    #[default]
    BehaviorChange,
    /// Behaviour stays the same: no new test is required, and the existing tests must still pass.
    Refactor,
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

/// Bounds on one tool conversation, so a model that wanders cannot run forever.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToolLimits {
    /// Tool calls the harness will carry out in one attempt, submissions included.
    pub max_tool_calls: u32,
    /// Model turns in one attempt.
    pub max_turns: u32,
    /// Times the model may answer in prose instead of calling a tool before the attempt is abandoned.
    pub max_steers: u32,
    /// Whether `submit_files` must carry an import checklist, written before the files.
    pub declare_imports: bool,
}

impl Default for ToolLimits {
    fn default() -> Self {
        Self {
            max_tool_calls: 12,
            max_turns: 16,
            max_steers: 2,
            declare_imports: false,
        }
    }
}

/// How the model produces its files.
enum Mode {
    /// One schema-constrained call per attempt; the step names the files to show.
    Stateless(Box<dyn Generator>),
    /// A short tool conversation per attempt: the model finds and reads files, then submits them.
    Tools {
        generator: Box<dyn ToolGenerator>,
        limits: ToolLimits,
    },
}

pub struct Implementer {
    mode: Mode,
    build: Box<dyn Build>,
    observer: Arc<dyn RunObserver>,
    max_attempts: u32,
    /// The classes the project can load, when known: a submission whose imports do not fit it is refused.
    classpath: Option<Arc<crate::classpath::ClassIndex>>,
    /// Where the declarations of read-only Java files come from, when known.
    surfaces: Option<Arc<dyn crate::surfaces::Surfaces>>,
}

/// The files a step may create or change (empty when it is unscoped).
fn step_scope_paths(step: &StepRequest) -> Vec<String> {
    step.scope.create.iter().chain(&step.scope.modify).cloned().collect()
}

impl Implementer {
    /// Checks every submission's imports against `index` as well as against the repository and the JDK table.
    pub fn with_classpath(mut self, index: Arc<crate::classpath::ClassIndex>) -> Self {
        self.classpath = Some(index);
        self
    }

    /// Shows a step the declarations of its read-only Java files, from `surfaces`, instead of their source.
    pub fn with_surfaces(mut self, surfaces: Arc<dyn crate::surfaces::Surfaces>) -> Self {
        self.surfaces = Some(surfaces);
        self
    }

    pub fn new(
        generator: Box<dyn Generator>,
        build: Box<dyn Build>,
        observer: Arc<dyn RunObserver>,
        max_attempts: u32,
    ) -> Self {
        Self {
            mode: Mode::Stateless(generator),
            build,
            observer,
            max_attempts,
            classpath: None,
            surfaces: None,
        }
    }

    /// An Implementer whose model finds and reads the files it needs with tools.
    pub fn with_tools(
        generator: Box<dyn ToolGenerator>,
        build: Box<dyn Build>,
        observer: Arc<dyn RunObserver>,
        max_attempts: u32,
        limits: ToolLimits,
    ) -> Self {
        Self {
            mode: Mode::Tools { generator, limits },
            build,
            observer,
            max_attempts,
            classpath: None,
            surfaces: None,
        }
    }

    pub fn run_step(&self, root: &Path, step: &StepRequest) -> StepOutcome {
        let started = Instant::now();
        self.observer.on(&RunEvent::StepStarted(StepStarted {
            step_id: step.id.clone(),
            instruction: step.instruction.clone(),
            context: step.context.clone(),
            scope: step.scope.clone(),
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

    /// One attempt: get files from the model, then write them and let the build judge them. Anything
    /// written is recorded in `snapshot`, so the caller can undo it.
    fn attempt(
        &self,
        root: &Path,
        step: &StepRequest,
        attempt: u32,
        previous: Option<&Failure>,
        snapshot: &mut Snapshot,
    ) -> Result<(), Failure> {
        let files = match &self.mode {
            Mode::Stateless(generator) => self.generate_stateless(generator.as_ref(), root, step, attempt, previous)?,
            Mode::Tools { generator, limits } => {
                self.generate_with_tools(generator.as_ref(), limits, root, step, attempt, previous)?
            }
        };
        self.apply_and_check(root, step, attempt, files, snapshot)
    }

    /// One schema-constrained call; the files the step names are shown in the prompt.
    fn generate_stateless(
        &self,
        generator: &dyn Generator,
        root: &Path,
        step: &StepRequest,
        attempt: u32,
        previous: Option<&Failure>,
    ) -> Result<GeneratedFiles, Failure> {
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
        let response = generator
            .complete_structured(&request)
            .map_err(|e| failure(Stage::Llm, e.to_string(), None))?;
        if response.finish_reason == "length" {
            return Err(failure(
                Stage::Truncated,
                "The answer hit the token limit before it was complete. Return fewer or shorter files.".into(),
                None,
            ));
        }
        GeneratedFiles::parse(&response.content).map_err(|e| failure(Stage::Parse, e.to_string(), None))
    }

    /// A tool conversation: the model finds and reads files, and ends by submitting. Nothing is
    /// written here; a submission that breaks a rule is answered with the reason and the
    /// conversation goes on.
    fn generate_with_tools(
        &self,
        generator: &dyn ToolGenerator,
        limits: &ToolLimits,
        root: &Path,
        step: &StepRequest,
        attempt: u32,
        previous: Option<&Failure>,
    ) -> Result<GeneratedFiles, Failure> {
        let toolbox = ToolBox::new(root);
        let tools = ToolBox::specs_with(limits.declare_imports);
        let shown = read_shown(root, &step.scope, self.surfaces.as_deref());
        let shown_paths: std::collections::HashSet<&str> = step
            .scope
            .modify
            .iter()
            .chain(&step.scope.read_only)
            .map(String::as_str)
            .collect();
        let (system, task) = prompt::tool_messages(step, &shown, previous);
        let mut messages = vec![ChatMessage::System(system), ChatMessage::User(task)];
        let purpose = if attempt == 1 {
            "navigate".to_string()
        } else {
            format!("navigate-retry-{}", attempt - 1)
        };
        let mut turns = 0;
        let mut tool_calls_made = 0;
        let mut steers = 0;
        let mut seen: HashMap<String, u32> = HashMap::new();

        loop {
            if turns >= limits.max_turns {
                return Err(failure(
                    Stage::Budget,
                    format!("You used {turns} turns without submitting."),
                    None,
                ));
            }
            turns += 1;
            let turn = generator
                .complete_with_tools(&ToolRequest {
                    purpose: purpose.clone(),
                    messages: messages.clone(),
                    tools: tools.clone(),
                    thinking: false,
                })
                .map_err(|e| failure(Stage::Llm, e.to_string(), None))?;
            if turn.finish_reason == "length" {
                return Err(failure(
                    Stage::Truncated,
                    "Your answer hit the token limit before it was complete. Submit fewer or shorter files.".into(),
                    None,
                ));
            }
            if turn.tool_calls.is_empty() {
                steers += 1;
                if steers > limits.max_steers {
                    return Err(failure(
                        Stage::Protocol,
                        "You answered in prose instead of calling submit_files.".into(),
                        None,
                    ));
                }
                messages.push(ChatMessage::Assistant {
                    content: turn.content,
                    tool_calls: vec![],
                });
                messages.push(ChatMessage::User(
                    "Do not answer in prose. Use glob and read_file to find what you need, then call submit_files \
                     with the complete content of every file you create or change."
                        .into(),
                ));
                continue;
            }

            messages.push(ChatMessage::Assistant {
                content: turn.content.clone(),
                tool_calls: turn.tool_calls.clone(),
            });
            for call in &turn.tool_calls {
                // Asking whether a file the step itself creates exists costs nothing: the answer is known, and the
                // question is a habit that used up the budget (7 of 10 runs lost an attempt to it).
                let probe = probed_new_file(call, &step.scope.create);
                if probe.is_none() {
                    tool_calls_made += 1;
                }
                if tool_calls_made > limits.max_tool_calls {
                    return Err(failure(
                        Stage::Budget,
                        format!(
                            "You used {} tool calls without a submission that could be built.",
                            limits.max_tool_calls
                        ),
                        None,
                    ));
                }
                let started = Instant::now();
                let repeats = seen.entry(format!("{}|{}", call.name, call.arguments)).or_insert(0);
                *repeats += 1;
                let mut accepted: Option<GeneratedFiles> = None;
                let times = *repeats;
                let stuck = times >= 3;
                let (outcome, text) = if stuck {
                    (
                        ToolOutcome::Refused,
                        "You already made exactly this call. Make a different call, or call submit_files.".to_string(),
                    )
                } else if call.name == "submit_files" {
                    match Submission::parse(&call.arguments) {
                        Err(e) => (
                            ToolOutcome::Error,
                            format!(
                                "The arguments of submit_files are not valid: {e}. Send \
                                 {{\"files\": [{{\"path\": \"...\", \"content\": \"...\"}}]}}."
                            ),
                        ),
                        Ok(Submission { imports, files }) => {
                            let scope_check = if step.scope.is_empty() {
                                Ok(())
                            } else {
                                gates::check_scope(&files, &step.scope, root)
                            };
                            // Files shown up front count as read: the model has their content.
                            let has_read = |p: &str| toolbox.has_read(p) || shown_paths.contains(p);
                            match scope_check
                                .and_then(|_| {
                                    gates::check_declared_imports(&files, imports.as_deref(), limits.declare_imports)
                                })
                                .and_then(|_| {
                                    gates::check_submission(
                                        &files,
                                        root,
                                        &has_read,
                                        step.kind,
                                        self.classpath.as_deref(),
                                    )
                                }) {
                                Err(rejection) => (ToolOutcome::Refused, rejection.message()),
                                Ok(()) => {
                                    accepted = Some(files);
                                    (
                                        ToolOutcome::Ok,
                                        "Submitted. The harness will now write the files, build and test.".to_string(),
                                    )
                                }
                            }
                        }
                    }
                } else if let Some(path) = &probe {
                    (
                        ToolOutcome::Ok,
                        format!(
                            "`{path}` is a file this step creates. It does not exist yet; write it in submit_files."
                        ),
                    )
                } else {
                    let result = toolbox.call(&call.name, &call.arguments);
                    (result.outcome, result.text)
                };
                self.observer.on(&RunEvent::ToolCalled(ToolCalled {
                    step_id: step.id.clone(),
                    attempt,
                    tool: call.name.clone(),
                    arguments: call.arguments.clone(),
                    outcome,
                    result: text.clone(),
                    elapsed_ms: started.elapsed().as_millis() as u64,
                }));
                if stuck {
                    return Err(failure(
                        Stage::Stuck,
                        format!(
                            "You made exactly the same {} call {times} times without making progress.",
                            call.name
                        ),
                        None,
                    ));
                }
                if let Some(files) = accepted {
                    return Ok(files);
                }
                messages.push(ChatMessage::Tool {
                    call_id: call.id.clone(),
                    content: text,
                });
            }
        }
    }

    /// Writes the files and lets the build judge them.
    fn apply_and_check(
        &self,
        root: &Path,
        step: &StepRequest,
        attempt: u32,
        files: GeneratedFiles,
        snapshot: &mut Snapshot,
    ) -> Result<(), Failure> {
        files
            .validate_paths()
            .map_err(|e| failure(Stage::Write, e.to_string(), Some(files.clone())))?;
        // A submission identical to what is on disk is not a step done. The tool conversation refuses it
        // earlier; this covers the single-call mode and is the last line of defence.
        let changes_something = files
            .files
            .iter()
            .any(|f| std::fs::read_to_string(root.join(&f.path)).map_or(true, |on_disk| on_disk != f.content));
        if !changes_something {
            return Err(failure(
                Stage::NoChange,
                "Every file you submitted is identical to what is already there, so nothing changed.".into(),
                Some(files),
            ));
        }
        snapshot.capture(&files.paths());
        files
            .write_into(root)
            .map_err(|e| failure(Stage::Write, e.to_string(), Some(files.clone())))?;
        self.observer.on(&RunEvent::FilesWritten(FilesWritten {
            step_id: step.id.clone(),
            attempt,
            paths: files.paths(),
        }));

        let compiled = self.build.compile(root);
        if !compiled.success {
            // The digest is what the compiler said; the hint is a fact the harness adds after it.
            let feedback = match hints::import_hint(&compiled.digest, root) {
                Some(hint) => format!("{}\n\n{hint}", compiled.digest),
                None => compiled.digest,
            };
            return Err(failure(Stage::Compile, feedback, Some(files)));
        }
        let tested = self.build.test(root);
        if !tested.success {
            // The digest is what the build said; the hint is a fact the harness adds after it.
            let feedback = match hints::subject_hint(&tested.digest, &files.paths(), &step_scope_paths(step), root) {
                Some(hint) => format!("{}\n\n{hint}", tested.digest),
                None => tested.digest,
            };
            return Err(failure(Stage::Test, feedback, Some(files)));
        }
        Ok(())
    }
}

impl StepRunner for Implementer {
    fn run_step(&self, root: &Path, step: &StepRequest) -> StepOutcome {
        Implementer::run_step(self, root, step)
    }
}

/// The path of a file the step creates, when the call is a `glob` or `read_file` of exactly that path.
fn probed_new_file(call: &crate::llm::ToolCall, create: &[String]) -> Option<String> {
    let key = match call.name.as_str() {
        "glob" => "pattern",
        "read_file" => "path",
        _ => return None,
    };
    let arguments: serde_json::Value = serde_json::from_str(&call.arguments).ok()?;
    let target = arguments.get(key)?.as_str()?;
    create.iter().find(|path| path.as_str() == target).cloned()
}

fn failure(stage: Stage, feedback: String, files: Option<GeneratedFiles>) -> Failure {
    Failure { stage, feedback, files }
}

/// The content of the files a scoped step is shown up front. A file that is missing is simply not shown.
fn read_shown(root: &Path, scope: &Scope, surfaces: Option<&dyn crate::surfaces::Surfaces>) -> prompt::ShownFiles {
    let read = |paths: &[String]| -> Vec<GeneratedFile> {
        paths
            .iter()
            .filter_map(|path| {
                let content = std::fs::read_to_string(root.join(path)).ok()?;
                Some(GeneratedFile {
                    path: path.clone(),
                    content,
                })
            })
            .collect()
    };
    let mut declared_text = surfaces
        .map(|s| {
            let java: Vec<String> = scope
                .read_only
                .iter()
                .filter(|p| p.ends_with(".java"))
                .cloned()
                .collect();
            s.surfaces(root, &java)
        })
        .unwrap_or_default();
    let (declared, whole): (Vec<String>, Vec<String>) = scope
        .read_only
        .iter()
        .cloned()
        .partition(|p| declared_text.contains_key(p));
    prompt::ShownFiles {
        modify: read(&scope.modify),
        read_only: read(&whole),
        declared: declared
            .into_iter()
            .filter_map(|path| {
                let content = declared_text.remove(&path)?;
                Some(GeneratedFile { path, content })
            })
            .collect(),
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
            kind: Default::default(),
            scope: Default::default(),
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
    fn the_step_started_event_records_what_the_step_was_told_its_context_and_its_scope() {
        let rig = rig(vec![answer(&[("src/A.java", "class A {}")])], vec![], vec![], 3);
        let mut scoped = step();
        scoped.scope.create = vec!["src/A.java".into()];
        scoped.scope.read_only = vec!["src/B.java".into()];
        rig.implementer.run_step(rig.dir.path(), &scoped);
        let started = rig
            .observer
            .events()
            .into_iter()
            .find_map(|e| match e {
                RunEvent::StepStarted(s) => Some(s),
                _ => None,
            })
            .expect("a step_started event");
        assert_eq!(started.context, "pkg: x");
        assert_eq!(started.scope.create, ["src/A.java"]);
        assert_eq!(started.scope.read_only, ["src/B.java"]);
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
    // ---- tool mode -------------------------------------------------------------------------------

    use crate::llm::{ChatMessage, ToolCall, ToolGenerator, ToolRequest, ToolTurn};
    use crate::observe::ToolOutcome;
    use serde_json::{json, Value};

    const GREETER: &str = "src/main/java/com/example/Greeter.java";
    const GREETER_TEST: &str = "src/test/java/com/example/GreeterTest.java";
    const OLD_GREETER: &str = "package com.example;\npublic class Greeter { }\n";
    const OLD_TEST: &str = "package com.example;\nclass GreeterTest { void t() { new Greeter(); } }\n";
    const NEW_GREETER: &str = "package com.example;\npublic class Greeter { /* trims */ }\n";
    const NEW_TEST: &str =
        "package com.example;\nclass GreeterTest { void t() { new Greeter(); } void trims() { new Greeter(); } }\n";

    struct ScriptedTools {
        turns: Mutex<VecDeque<Result<ToolTurn, LlmError>>>,
        requests: Arc<Mutex<Vec<Vec<ChatMessage>>>>,
        /// The property names of submit_files in each request.
        tools_seen: Arc<Mutex<Vec<Vec<String>>>>,
    }

    impl ToolGenerator for ScriptedTools {
        fn complete_with_tools(&self, request: &ToolRequest) -> Result<ToolTurn, LlmError> {
            self.requests.lock().unwrap().push(request.messages.clone());
            let submit_properties = request
                .tools
                .iter()
                .find(|tool| tool.name == "submit_files")
                .and_then(|tool| {
                    tool.parameters["properties"]
                        .as_object()
                        .map(|p| p.keys().cloned().collect())
                })
                .unwrap_or_default();
            self.tools_seen.lock().unwrap().push(submit_properties);
            self.turns
                .lock()
                .unwrap()
                .pop_front()
                .expect("the script ran out of turns")
        }
    }

    type Call = (&'static str, Value);

    fn calls(list: Vec<Call>) -> Result<ToolTurn, LlmError> {
        let tool_calls = list
            .into_iter()
            .enumerate()
            .map(|(i, (name, args))| ToolCall {
                id: format!("c{i}"),
                name: name.to_string(),
                arguments: args.to_string(),
            })
            .collect();
        Ok(ToolTurn {
            call_id: 1,
            content: String::new(),
            tool_calls,
            finish_reason: "tool_calls".into(),
            prompt_tokens: 10,
            completion_tokens: 10,
        })
    }

    fn prose(text: &str) -> Result<ToolTurn, LlmError> {
        Ok(ToolTurn {
            call_id: 1,
            content: text.into(),
            tool_calls: vec![],
            finish_reason: "stop".into(),
            prompt_tokens: 10,
            completion_tokens: 10,
        })
    }

    fn glob(pattern: &'static str) -> Call {
        ("glob", json!({ "pattern": pattern }))
    }

    fn read_file(path: &'static str) -> Call {
        ("read_file", json!({ "path": path }))
    }

    fn submit(files: &[(&str, &str)]) -> Call {
        let files: Vec<_> = files.iter().map(|(p, c)| json!({ "path": p, "content": c })).collect();
        ("submit_files", json!({ "files": files }))
    }

    struct ToolRig {
        implementer: Implementer,
        observer: Arc<RecordingObserver>,
        requests: Arc<Mutex<Vec<Vec<ChatMessage>>>>,
        tools_seen: Arc<Mutex<Vec<Vec<String>>>>,
        compile_calls: Arc<Mutex<u32>>,
        dir: tempfile::TempDir,
    }

    fn tool_rig(
        turns: Vec<Result<ToolTurn, LlmError>>,
        compiles: Vec<BuildOutcome>,
        max_attempts: u32,
        limits: ToolLimits,
    ) -> ToolRig {
        tool_rig_with_tests(turns, compiles, vec![], max_attempts, limits)
    }

    fn tool_rig_with_tests(
        turns: Vec<Result<ToolTurn, LlmError>>,
        compiles: Vec<BuildOutcome>,
        tests: Vec<BuildOutcome>,
        max_attempts: u32,
        limits: ToolLimits,
    ) -> ToolRig {
        let observer = Arc::new(RecordingObserver::new());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let tools_seen = Arc::new(Mutex::new(Vec::new()));
        let compile_calls = Arc::new(Mutex::new(0));
        let implementer = Implementer::with_tools(
            Box::new(ScriptedTools {
                turns: Mutex::new(turns.into()),
                requests: requests.clone(),
                tools_seen: tools_seen.clone(),
            }),
            Box::new(ScriptedBuild {
                compiles: Mutex::new(compiles.into()),
                tests: Mutex::new(tests.into()),
                compile_calls: compile_calls.clone(),
            }),
            observer.clone(),
            max_attempts,
            limits,
        );
        let dir = tempfile::tempdir().unwrap();
        for (rel, content) in [
            (GREETER, OLD_GREETER),
            (GREETER_TEST, OLD_TEST),
            (
                "src/main/java/com/example/Formatter.java",
                "package com.example;\npublic class Formatter { }\n",
            ),
            ("pom.xml", "<project/>"),
        ] {
            let p = dir.path().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, content).unwrap();
        }
        ToolRig {
            implementer,
            observer,
            requests,
            tools_seen,
            compile_calls,
            dir,
        }
    }

    fn nav_step() -> StepRequest {
        StepRequest {
            id: "nav".into(),
            instruction: "The Greeter should trim.".into(),
            context: "Build: Maven".into(),
            files: vec![],
            kind: StepKind::BehaviorChange,
            scope: Scope::default(),
        }
    }

    fn on_disk(rig: &ToolRig, rel: &str) -> String {
        std::fs::read_to_string(rig.dir.path().join(rel)).unwrap()
    }

    /// (tool, outcome) for every tool call, in order.
    fn tool_log(rig: &ToolRig) -> Vec<(String, ToolOutcome)> {
        rig.observer
            .events()
            .into_iter()
            .filter_map(|e| match e {
                RunEvent::ToolCalled(t) => Some((t.tool, t.outcome)),
                _ => None,
            })
            .collect()
    }

    fn tool_result(rig: &ToolRig, index: usize) -> String {
        rig.observer
            .events()
            .into_iter()
            .filter_map(|e| match e {
                RunEvent::ToolCalled(t) => Some(t.result),
                _ => None,
            })
            .nth(index)
            .unwrap()
    }

    #[test]
    fn the_model_finds_reads_and_submits_and_the_harness_writes_and_builds() {
        let rig = tool_rig(
            vec![
                calls(vec![glob("src/**/*.java")]),
                calls(vec![read_file(GREETER), read_file(GREETER_TEST)]),
                calls(vec![submit(&[(GREETER, NEW_GREETER), (GREETER_TEST, NEW_TEST)])]),
            ],
            vec![],
            3,
            ToolLimits::default(),
        );
        assert_eq!(
            rig.implementer.run_step(rig.dir.path(), &nav_step()),
            StepOutcome::Passed { attempts: 1 }
        );
        assert_eq!(on_disk(&rig, GREETER), NEW_GREETER);
        assert_eq!(on_disk(&rig, GREETER_TEST), NEW_TEST);
        assert_eq!(
            tool_log(&rig),
            [
                ("glob", ToolOutcome::Ok),
                ("read_file", ToolOutcome::Ok),
                ("read_file", ToolOutcome::Ok),
                ("submit_files", ToolOutcome::Ok)
            ]
            .map(|(t, o)| (t.to_string(), o))
        );
        assert!(
            tool_result(&rig, 0).contains("Greeter.java"),
            "the model is told what glob found"
        );
        let requests = rig.requests.lock().unwrap();
        assert_eq!(requests.len(), 3);
        assert_eq!(
            requests[0].len(),
            2,
            "a conversation starts with the rules and the task"
        );
        assert!(matches!(&requests[1][3], ChatMessage::Tool { content, .. } if content.contains("Greeter.java")));
    }

    #[test]
    fn a_submission_that_changes_an_unread_file_is_refused_writes_nothing_and_the_conversation_goes_on() {
        let rig = tool_rig(
            vec![
                calls(vec![submit(&[(GREETER, NEW_GREETER), (GREETER_TEST, NEW_TEST)])]),
                calls(vec![read_file(GREETER), read_file(GREETER_TEST)]),
                calls(vec![submit(&[(GREETER, NEW_GREETER), (GREETER_TEST, NEW_TEST)])]),
            ],
            vec![],
            3,
            ToolLimits::default(),
        );
        assert_eq!(
            rig.implementer.run_step(rig.dir.path(), &nav_step()),
            StepOutcome::Passed { attempts: 1 }
        );
        let log = tool_log(&rig);
        assert_eq!(log[0], ("submit_files".to_string(), ToolOutcome::Refused));
        assert!(
            tool_result(&rig, 0).contains("have not read"),
            "{}",
            tool_result(&rig, 0)
        );
        assert_eq!(
            *rig.compile_calls.lock().unwrap(),
            1,
            "the refused submission never reached the build"
        );
        assert_eq!(
            rig.observer
                .events()
                .iter()
                .filter(|e| matches!(e, RunEvent::AttemptFailed(_)))
                .count(),
            0
        );
    }

    #[test]
    fn a_new_file_with_its_test_needs_no_reading() {
        let new_class = "package com.example;\npublic class Farewell { }\n";
        let new_test = "package com.example;\nclass FarewellTest { void t() { new Farewell(); } }\n";
        let rig = tool_rig(
            vec![calls(vec![submit(&[
                ("src/main/java/com/example/Farewell.java", new_class),
                ("src/test/java/com/example/FarewellTest.java", new_test),
            ])])],
            vec![],
            1,
            ToolLimits::default(),
        );
        assert_eq!(
            rig.implementer.run_step(rig.dir.path(), &nav_step()),
            StepOutcome::Passed { attempts: 1 }
        );
    }

    #[test]
    fn production_code_without_a_test_is_refused_and_a_refactor_is_not_asked_for_one() {
        let rig = tool_rig(
            vec![
                calls(vec![read_file(GREETER)]),
                calls(vec![submit(&[(GREETER, NEW_GREETER)])]),
                calls(vec![read_file(GREETER_TEST)]),
                calls(vec![submit(&[(GREETER, NEW_GREETER), (GREETER_TEST, NEW_TEST)])]),
            ],
            vec![],
            1,
            ToolLimits::default(),
        );
        assert_eq!(
            rig.implementer.run_step(rig.dir.path(), &nav_step()),
            StepOutcome::Passed { attempts: 1 }
        );
        assert!(
            tool_result(&rig, 1).contains("ALWAYS include a test"),
            "{}",
            tool_result(&rig, 1)
        );

        let refactor = tool_rig(
            vec![
                calls(vec![read_file(GREETER)]),
                calls(vec![submit(&[(GREETER, NEW_GREETER)])]),
            ],
            vec![],
            1,
            ToolLimits::default(),
        );
        let step = StepRequest {
            kind: StepKind::Refactor,
            ..nav_step()
        };
        assert_eq!(
            refactor.implementer.run_step(refactor.dir.path(), &step),
            StepOutcome::Passed { attempts: 1 }
        );
    }

    #[test]
    fn a_prose_answer_is_steered_back_to_submitting_and_persistent_prose_fails_the_attempt() {
        let ok_files = [
            ("src/main/java/com/example/F.java", "public class F { }"),
            ("src/test/java/com/example/FTest.java", "class FTest { F f; }"),
        ];
        let rig = tool_rig(
            vec![prose("Sure, here is what I would do."), calls(vec![submit(&ok_files)])],
            vec![],
            1,
            ToolLimits::default(),
        );
        assert_eq!(
            rig.implementer.run_step(rig.dir.path(), &nav_step()),
            StepOutcome::Passed { attempts: 1 }
        );
        let second = &rig.requests.lock().unwrap()[1];
        assert!(
            matches!(second.last(), Some(ChatMessage::User(m)) if m.contains("submit_files")),
            "{second:?}"
        );

        let stubborn = tool_rig(
            vec![prose("a"), prose("b"), prose("c")],
            vec![],
            1,
            ToolLimits {
                max_steers: 2,
                ..ToolLimits::default()
            },
        );
        let result = stubborn.implementer.run_step(stubborn.dir.path(), &nav_step());
        assert!(matches!(result, StepOutcome::Failed { attempts: 1, .. }), "{result:?}");
        assert!(stubborn
            .observer
            .events()
            .iter()
            .any(|e| matches!(e, RunEvent::AttemptFailed(f) if f.stage == Stage::Protocol)));
    }

    fn submit_declared(imports: &[(&str, &str)], files: &[(&str, &str)]) -> Call {
        let imports: Vec<_> = imports.iter().map(|(f, i)| json!({ "file": f, "import": i })).collect();
        let files: Vec<_> = files.iter().map(|(p, c)| json!({ "path": p, "content": c })).collect();
        ("submit_files", json!({ "declared_imports": imports, "files": files }))
    }

    #[test]
    fn with_declared_imports_on_a_submission_without_the_list_is_refused_and_one_with_it_goes_through() {
        let files = [
            (
                "src/main/java/com/example/F.java",
                "package com.example;\npublic class F { }\n",
            ),
            (
                "src/test/java/com/example/FTest.java",
                "package com.example;\nclass FTest { F f; }\n",
            ),
        ];
        let rig = tool_rig(
            vec![calls(vec![submit(&files)]), calls(vec![submit_declared(&[], &files)])],
            vec![],
            1,
            ToolLimits {
                declare_imports: true,
                ..ToolLimits::default()
            },
        );
        let result = rig.implementer.run_step(rig.dir.path(), &nav_step());
        assert!(matches!(result, StepOutcome::Passed { .. }), "{result:?}");
        let log = tool_log(&rig);
        assert_eq!(log[0], ("submit_files".to_string(), ToolOutcome::Refused));
        assert_eq!(log[1], ("submit_files".to_string(), ToolOutcome::Ok));
    }

    #[test]
    fn the_declared_imports_variant_is_what_the_model_is_offered_only_when_switched_on() {
        let offered = |declare: bool| {
            let rig = tool_rig(
                vec![prose("a"), prose("b"), prose("c")],
                vec![],
                1,
                ToolLimits {
                    declare_imports: declare,
                    ..ToolLimits::default()
                },
            );
            let _ = rig.implementer.run_step(rig.dir.path(), &nav_step());
            let seen = rig.tools_seen.lock().unwrap().first().cloned().unwrap_or_default();
            seen
        };
        assert!(offered(true).contains(&"declared_imports".to_string()));
        assert!(!offered(false).contains(&"declared_imports".to_string()));
    }

    #[test]
    fn using_up_the_tool_calls_fails_the_attempt_and_leaves_the_workspace_untouched() {
        let limits = ToolLimits {
            max_tool_calls: 3,
            ..ToolLimits::default()
        };
        let rig = tool_rig(
            vec![
                calls(vec![glob("a*")]),
                calls(vec![glob("b*")]),
                calls(vec![glob("c*")]),
                calls(vec![glob("d*")]),
            ],
            vec![],
            1,
            limits,
        );
        let result = rig.implementer.run_step(rig.dir.path(), &nav_step());
        assert!(matches!(result, StepOutcome::Failed { .. }));
        assert!(rig
            .observer
            .events()
            .iter()
            .any(|e| matches!(e, RunEvent::AttemptFailed(f) if f.stage == Stage::Budget)));
        assert_eq!(on_disk(&rig, GREETER), OLD_GREETER);
        assert_eq!(tool_log(&rig).len(), 3, "the call over the budget is not carried out");
    }

    #[test]
    fn asking_whether_a_file_the_step_creates_exists_does_not_use_up_the_tool_calls() {
        let limits = ToolLimits {
            max_tool_calls: 3,
            ..ToolLimits::default()
        };
        let created = "src/main/java/com/example/Made.java";
        let made_test = "src/test/java/com/example/MadeTest.java";
        let rig = tool_rig(
            vec![
                calls(vec![glob(created), read_file(created), glob("src/test/**")]),
                calls(vec![submit(&[
                    (created, "package com.example;\npublic class Made { }\n"),
                    (
                        made_test,
                        "package com.example;\nclass MadeTest { void t() { new Made(); } }\n",
                    ),
                ])]),
            ],
            vec![],
            1,
            limits,
        );
        let step = scoped(&[], &[created, made_test]);
        let result = rig.implementer.run_step(rig.dir.path(), &step);
        assert_eq!(
            result,
            StepOutcome::Passed { attempts: 1 },
            "{:?}",
            rig.observer.events()
        );
        assert!(!rig
            .observer
            .events()
            .iter()
            .any(|e| matches!(e, RunEvent::AttemptFailed(f) if f.stage == Stage::Budget)));
        assert_eq!(tool_log(&rig).len(), 4, "every call was still carried out and recorded");
    }

    #[test]
    fn the_same_call_a_third_time_ends_the_attempt_and_the_retry_starts_fresh() {
        let rig = tool_rig(
            vec![
                calls(vec![glob("src/**")]),
                calls(vec![glob("src/**")]),
                calls(vec![glob("src/**")]),
                // attempt 2, a fresh conversation
                calls(vec![read_file(GREETER), read_file(GREETER_TEST)]),
                calls(vec![submit(&[(GREETER, NEW_GREETER), (GREETER_TEST, NEW_TEST)])]),
            ],
            vec![],
            2,
            ToolLimits::default(),
        );
        assert_eq!(
            rig.implementer.run_step(rig.dir.path(), &nav_step()),
            StepOutcome::Passed { attempts: 2 }
        );
        let stages: Vec<Stage> = rig
            .observer
            .events()
            .into_iter()
            .filter_map(|e| match e {
                RunEvent::AttemptFailed(f) => Some(f.stage),
                _ => None,
            })
            .collect();
        assert_eq!(stages, [Stage::Stuck]);
        assert_eq!(
            tool_log(&rig)[2],
            ("glob".to_string(), ToolOutcome::Refused),
            "the third call is recorded"
        );
        assert_eq!(
            rig.requests.lock().unwrap().len(),
            5,
            "three turns in attempt 1, not a dozen"
        );
        assert_eq!(
            rig.requests.lock().unwrap()[3].len(),
            2,
            "attempt 2 starts with no history"
        );
    }

    #[test]
    fn a_model_that_keeps_submitting_nothing_is_stopped_after_three_identical_empty_submissions() {
        let rig = tool_rig(
            vec![
                calls(vec![submit(&[])]),
                calls(vec![submit(&[])]),
                calls(vec![submit(&[])]),
            ],
            vec![],
            1,
            ToolLimits::default(),
        );
        let result = rig.implementer.run_step(rig.dir.path(), &nav_step());
        assert!(matches!(result, StepOutcome::Failed { attempts: 1, .. }), "{result:?}");
        assert_eq!(rig.requests.lock().unwrap().len(), 3);
    }

    #[test]
    fn a_stateless_answer_identical_to_the_files_on_disk_is_a_no_change_failure_not_a_pass() {
        let rig = rig(
            vec![
                answer(&[("src/Existing.java", "same")]),
                answer(&[("src/Existing.java", "v2")]),
            ],
            vec![],
            vec![],
            3,
        );
        std::fs::create_dir_all(rig.dir.path().join("src")).unwrap();
        std::fs::write(rig.dir.path().join("src/Existing.java"), "same").unwrap();
        assert_eq!(
            rig.implementer.run_step(rig.dir.path(), &step()),
            StepOutcome::Passed { attempts: 2 }
        );
        assert!(
            tags(&rig.observer).contains(&"attempt_failed:1:nochange".to_string()),
            "{:?}",
            tags(&rig.observer)
        );
        assert_eq!(
            *rig.compile_calls.lock().unwrap(),
            1,
            "the no-op never reached the build"
        );
    }

    #[test]
    fn a_tool_submission_identical_to_disk_is_refused_in_the_conversation() {
        let rig = tool_rig(
            vec![
                calls(vec![read_file(GREETER)]),
                calls(vec![submit(&[(GREETER, OLD_GREETER)])]),
                calls(vec![read_file(GREETER_TEST)]),
                calls(vec![submit(&[(GREETER, NEW_GREETER), (GREETER_TEST, NEW_TEST)])]),
            ],
            vec![],
            1,
            ToolLimits::default(),
        );
        assert_eq!(
            rig.implementer.run_step(rig.dir.path(), &nav_step()),
            StepOutcome::Passed { attempts: 1 }
        );
        assert!(
            tool_result(&rig, 1).contains("nothing would change"),
            "{}",
            tool_result(&rig, 1)
        );
    }

    #[test]
    fn sandbox_violations_and_unknown_tools_are_answered_not_fatal() {
        let rig = tool_rig(
            vec![
                calls(vec![
                    read_file("../outside.txt"),
                    read_file("/etc/passwd"),
                    ("delete_file", json!({})),
                ]),
                calls(vec![read_file(GREETER), read_file(GREETER_TEST)]),
                calls(vec![submit(&[(GREETER, NEW_GREETER), (GREETER_TEST, NEW_TEST)])]),
            ],
            vec![],
            1,
            ToolLimits::default(),
        );
        assert_eq!(
            rig.implementer.run_step(rig.dir.path(), &nav_step()),
            StepOutcome::Passed { attempts: 1 }
        );
        let log = tool_log(&rig);
        assert_eq!(
            &log[..3],
            &[
                ("read_file".to_string(), ToolOutcome::Refused),
                ("read_file".to_string(), ToolOutcome::Refused),
                ("delete_file".to_string(), ToolOutcome::Error)
            ]
        );
    }

    #[test]
    fn a_failed_build_restores_the_workspace_and_the_retry_is_a_fresh_conversation_that_must_read_again() {
        let rig = tool_rig(
            vec![
                calls(vec![read_file(GREETER), read_file(GREETER_TEST)]),
                calls(vec![submit(&[(GREETER, "broken"), (GREETER_TEST, NEW_TEST)])]),
                // second attempt: tries to submit without reading first
                calls(vec![submit(&[(GREETER, NEW_GREETER), (GREETER_TEST, NEW_TEST)])]),
                calls(vec![read_file(GREETER), read_file(GREETER_TEST)]),
                calls(vec![submit(&[(GREETER, NEW_GREETER), (GREETER_TEST, NEW_TEST)])]),
            ],
            vec![outcome(BuildKind::Compile, false, "Greeter.java:1:1: boom")],
            2,
            ToolLimits::default(),
        );
        assert_eq!(
            rig.implementer.run_step(rig.dir.path(), &nav_step()),
            StepOutcome::Passed { attempts: 2 }
        );
        let requests = rig.requests.lock().unwrap();
        // Attempt 1 made two model calls (requests 0 and 1); request 2 starts attempt 2.
        assert_eq!(requests[2].len(), 2, "no history is carried over");
        let ChatMessage::User(task) = &requests[2][1] else {
            panic!()
        };
        assert!(
            task.contains("Your previous answer") && task.contains("boom") && task.contains("broken"),
            "{task}"
        );
        assert!(
            tool_result(&rig, 3).contains("have not read"),
            "the read set starts empty again: {}",
            tool_result(&rig, 3)
        );
    }

    #[test]
    fn a_truncated_turn_and_a_failed_model_call_each_fail_the_attempt_and_are_retried() {
        let mut cut = calls(vec![submit(&[(GREETER, "x")])]).unwrap();
        cut.finish_reason = "length".into();
        let err = LlmError {
            call_id: 1,
            kind: FailureKind::HttpStatus,
            status: Some(500),
            message: "parse".into(),
        };
        let rig = tool_rig(
            vec![
                Ok(cut),
                Err(err),
                calls(vec![submit(&[
                    ("src/main/java/com/example/F.java", "public class F { }"),
                    ("src/test/java/com/example/FTest.java", "class FTest { F f; }"),
                ])]),
            ],
            vec![],
            3,
            ToolLimits::default(),
        );
        assert_eq!(
            rig.implementer.run_step(rig.dir.path(), &nav_step()),
            StepOutcome::Passed { attempts: 3 }
        );
        let stages: Vec<Stage> = rig
            .observer
            .events()
            .into_iter()
            .filter_map(|e| match e {
                RunEvent::AttemptFailed(f) => Some(f.stage),
                _ => None,
            })
            .collect();
        assert_eq!(stages, [Stage::Truncated, Stage::Llm]);
    }
    #[test]
    fn a_test_failure_in_a_class_the_model_did_not_change_is_reported_with_a_hint_to_the_model() {
        let digest = "FormatterTest.trims:5 expected: <Ada> but was: <  ada  >\nTests run: 4, Failures: 1";
        let rig = tool_rig_with_tests(
            vec![
                calls(vec![read_file(GREETER), read_file(GREETER_TEST)]),
                calls(vec![submit(&[(GREETER, NEW_GREETER), (GREETER_TEST, NEW_TEST)])]),
                calls(vec![read_file(GREETER), read_file(GREETER_TEST)]),
                calls(vec![submit(&[(GREETER, NEW_GREETER), (GREETER_TEST, NEW_TEST)])]),
            ],
            vec![],
            vec![outcome(BuildKind::Test, false, digest)],
            2,
            ToolLimits::default(),
        );
        assert_eq!(
            rig.implementer.run_step(rig.dir.path(), &nav_step()),
            StepOutcome::Passed { attempts: 2 }
        );

        let feedback = rig
            .observer
            .events()
            .into_iter()
            .find_map(|e| match e {
                RunEvent::AttemptFailed(f) => Some(f.feedback),
                _ => None,
            })
            .expect("the first attempt failed");
        assert!(
            feedback.starts_with(digest),
            "the digest comes first, unchanged: {feedback}"
        );
        assert!(
            feedback.contains("FormatterTest tests Formatter") && feedback.contains("did not change"),
            "{feedback}"
        );
        let retry = rig.requests.lock().unwrap()[2].clone();
        let ChatMessage::User(task) = &retry[1] else { panic!() };
        assert!(
            task.contains("FormatterTest tests Formatter"),
            "the model is told: {task}"
        );
    }

    #[test]
    fn no_hint_is_added_when_the_failing_tests_class_was_changed() {
        let digest = "GreeterTest.t:5 expected: <1> but was: <2>";
        let rig = tool_rig_with_tests(
            vec![
                calls(vec![read_file(GREETER), read_file(GREETER_TEST)]),
                calls(vec![submit(&[(GREETER, NEW_GREETER), (GREETER_TEST, NEW_TEST)])]),
            ],
            vec![],
            vec![outcome(BuildKind::Test, false, digest)],
            1,
            ToolLimits::default(),
        );
        rig.implementer.run_step(rig.dir.path(), &nav_step());
        let feedback = rig
            .observer
            .events()
            .into_iter()
            .find_map(|e| match e {
                RunEvent::AttemptFailed(f) => Some(f.feedback),
                _ => None,
            })
            .unwrap();
        assert_eq!(feedback, digest);
    }
    fn scoped(modify: &[&str], create: &[&str]) -> StepRequest {
        StepRequest {
            scope: Scope {
                modify: modify.iter().map(|s| s.to_string()).collect(),
                create: create.iter().map(|s| s.to_string()).collect(),
                read_only: vec![],
            },
            ..nav_step()
        }
    }

    #[test]
    fn a_scoped_step_shows_its_files_up_front_and_counts_them_as_read() {
        let rig = tool_rig(
            vec![calls(vec![submit(&[(GREETER, NEW_GREETER), (GREETER_TEST, NEW_TEST)])])],
            vec![],
            1,
            ToolLimits::default(),
        );
        let step = scoped(&[GREETER, GREETER_TEST], &[]);
        assert_eq!(
            rig.implementer.run_step(rig.dir.path(), &step),
            StepOutcome::Passed { attempts: 1 }
        );
        let ChatMessage::User(task) = &rig.requests.lock().unwrap()[0][1] else {
            panic!()
        };
        assert!(task.contains("## Files to change"), "{task}");
        assert!(
            task.contains(OLD_GREETER.trim()),
            "the current content is in the prompt: {task}"
        );
        assert_eq!(tool_log(&rig).len(), 1, "it never needed to call read_file");
    }

    struct FixedSurfaces(HashMap<String, String>);

    impl crate::surfaces::Surfaces for FixedSurfaces {
        fn surfaces(&self, _: &Path, paths: &[String]) -> HashMap<String, String> {
            paths
                .iter()
                .filter_map(|p| self.0.get(p).map(|t| (p.clone(), t.clone())))
                .collect()
        }
    }

    const FORMATTER: &str = "src/main/java/com/example/Formatter.java";

    fn prompt_with_surfaces(surfaces: HashMap<String, String>) -> String {
        let mut rig = tool_rig(
            vec![calls(vec![submit(&[(GREETER, NEW_GREETER), (GREETER_TEST, NEW_TEST)])])],
            vec![],
            1,
            ToolLimits::default(),
        );
        rig.implementer = rig.implementer.with_surfaces(Arc::new(FixedSurfaces(surfaces)));
        let mut step = scoped(&[GREETER, GREETER_TEST], &[]);
        step.scope.read_only = vec![FORMATTER.into()];
        rig.implementer.run_step(rig.dir.path(), &step);
        let ChatMessage::User(task) = &rig.requests.lock().unwrap()[0][1] else {
            panic!()
        };
        task.clone()
    }

    #[test]
    fn a_read_only_java_file_is_shown_as_its_declarations_when_they_are_known_and_as_source_when_not() {
        let known = prompt_with_surfaces(HashMap::from([(
            FORMATTER.to_string(),
            "public class Formatter\n  public String fmt(String s)\n".to_string(),
        )]));
        assert!(known.contains("public String fmt(String s)"), "{known}");
        assert!(
            !known.contains("public class Formatter { }"),
            "the source is not repeated: {known}"
        );
        let unknown = prompt_with_surfaces(HashMap::new());
        assert!(unknown.contains("public class Formatter { }"), "{unknown}");
        assert!(!unknown.contains("Declarations of files for reference"), "{unknown}");
    }

    #[test]
    fn a_scoped_submission_with_a_file_outside_the_scope_is_refused_in_the_conversation() {
        let rig = tool_rig(
            vec![
                calls(vec![submit(&[
                    (GREETER, NEW_GREETER),
                    (GREETER_TEST, NEW_TEST),
                    ("src/main/java/com/invented/Extra.java", "public class Extra {}"),
                ])]),
                calls(vec![submit(&[(GREETER, NEW_GREETER), (GREETER_TEST, NEW_TEST)])]),
            ],
            vec![],
            1,
            ToolLimits::default(),
        );
        let step = scoped(&[GREETER, GREETER_TEST], &[]);
        assert_eq!(
            rig.implementer.run_step(rig.dir.path(), &step),
            StepOutcome::Passed { attempts: 1 }
        );
        assert_eq!(tool_log(&rig)[0], ("submit_files".to_string(), ToolOutcome::Refused));
        assert!(
            tool_result(&rig, 0).contains("not among the files"),
            "{}",
            tool_result(&rig, 0)
        );
        assert!(!rig.dir.path().join("src/main/java/com/invented/Extra.java").exists());
    }

    #[test]
    fn a_scoped_step_that_must_create_files_refuses_a_submission_without_them() {
        let class = "src/main/java/com/example/Farewell.java";
        let test = "src/test/java/com/example/FarewellTest.java";
        let rig = tool_rig(
            vec![
                calls(vec![submit(&[(
                    class,
                    "package com.example;\npublic class Farewell { }\n",
                )])]),
                calls(vec![submit(&[
                    (class, "package com.example;\npublic class Farewell { }\n"),
                    (
                        test,
                        "package com.example;\nclass FarewellTest { void t() { new Farewell(); } }\n",
                    ),
                ])]),
            ],
            vec![],
            1,
            ToolLimits::default(),
        );
        let step = scoped(&[], &[class, test]);
        assert_eq!(
            rig.implementer.run_step(rig.dir.path(), &step),
            StepOutcome::Passed { attempts: 1 }
        );
        assert!(
            tool_result(&rig, 0).contains("did not create"),
            "{}",
            tool_result(&rig, 0)
        );
    }
}
