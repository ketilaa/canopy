//! The whole loop against real Maven, with a scripted model. Skipped unless
//! `CANOPY_IMPLEMENTER_MVN_TEST=1`.
//!
//! The model's first answer has a compile error. The loop must run the real build, undo the
//! attempt, hand the model Maven's actual (digested) error, accept the fixed answer, and report
//! every step through events.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use canopy_implementer::build::CommandBuild;
use canopy_implementer::implementer::{Implementer, StepOutcome, StepRequest};
use canopy_implementer::llm::{Generator, LlmError, LlmResponse, Message, StructuredRequest};
use canopy_implementer::observe::{Observers, RecordingObserver, RunEvent, TraceWriter};

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

struct Scripted {
    replies: Mutex<VecDeque<String>>,
    prompts: Arc<Mutex<Vec<Vec<Message>>>>,
}

impl Generator for Scripted {
    fn complete_structured(&self, request: &StructuredRequest) -> Result<LlmResponse, LlmError> {
        self.prompts.lock().unwrap().push(request.messages.clone());
        let content = self.replies.lock().unwrap().pop_front().expect("script exhausted");
        Ok(LlmResponse {
            call_id: 1,
            content,
            finish_reason: "stop".into(),
            prompt_tokens: 1,
            completion_tokens: 1,
        })
    }
}

fn answer(greeter_body: &str) -> String {
    let greeter = format!("package com.example;\npublic class Greeter {{\n    public String greet(String name) {{ return {greeter_body}; }}\n}}\n");
    let test = "package com.example;\nimport org.junit.jupiter.api.Test;\nimport static org.junit.jupiter.api.Assertions.assertEquals;\nclass GreeterTest {\n    @Test void greetsByName() { assertEquals(\"Hello, Ada!\", new Greeter().greet(\"Ada\")); }\n}\n";
    serde_json::json!({"files": [
        {"path": "src/main/java/com/example/Greeter.java", "content": greeter},
        {"path": "src/test/java/com/example/GreeterTest.java", "content": test},
    ]})
    .to_string()
}

#[test]
fn a_compile_error_from_real_maven_is_fed_back_and_fixed() {
    if std::env::var("CANOPY_IMPLEMENTER_MVN_TEST").as_deref() != Ok("1") {
        eprintln!("skipped: set CANOPY_IMPLEMENTER_MVN_TEST=1 to run against real Maven");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("pom.xml"), POM).unwrap();
    let trace = dir.path().join("trace").join("events.jsonl");

    let recorder = Arc::new(RecordingObserver::new());
    let observer: Arc<Observers> = Arc::new(
        Observers::new()
            .with(recorder.clone())
            .with(Arc::new(TraceWriter::create(&trace, "loop-maven").unwrap())),
    );
    let prompts = Arc::new(Mutex::new(Vec::new()));
    let implementer = Implementer::new(
        Box::new(Scripted {
            replies: Mutex::new(VecDeque::from([
                answer("\"Hello, \" + nme + \"!\""),
                answer("\"Hello, \" + name + \"!\""),
            ])),
            prompts: prompts.clone(),
        }),
        Box::new(CommandBuild::maven(Duration::from_secs(240), observer.clone())),
        observer,
        3,
    );

    let step = StepRequest {
        id: "greeter".into(),
        instruction: "Create Greeter and its test.".into(),
        context: "Root package: com.example".into(),
    };
    let outcome = implementer.run_step(dir.path(), &step);

    assert_eq!(outcome, StepOutcome::Passed { attempts: 2 });
    let greeter = std::fs::read_to_string(dir.path().join("src/main/java/com/example/Greeter.java")).unwrap();
    assert!(greeter.contains("name + "), "{greeter}");

    let retry = &prompts.lock().unwrap()[1][1].content;
    assert!(retry.contains("The build failed to compile"), "{retry}");
    assert!(retry.contains("src/main/java/com/example/Greeter.java:3:"), "{retry}");
    assert!(retry.contains("cannot find symbol"), "{retry}");

    let ran: Vec<_> = recorder
        .events()
        .into_iter()
        .filter_map(|e| match e {
            RunEvent::CommandRan(c) => Some(c),
            _ => None,
        })
        .collect();
    assert_eq!(ran.len(), 3, "compile (fails), compile, test");
    assert!(!ran[0].success && ran[1].success && ran[2].success);
    assert!(ran.iter().all(|c| c.elapsed_ms > 0 && !c.output.is_empty()));

    let lines = std::fs::read_to_string(&trace).unwrap();
    eprintln!("trace: {} lines, {} bytes", lines.lines().count(), lines.len());
    for line in lines.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        let e = &v["event"];
        let detail = match e["type"].as_str().unwrap() {
            "command_ran" => format!("{} success={} {}ms", e["kind"], e["success"], e["elapsed_ms"]),
            "attempt_failed" => format!("stage={}", e["stage"]),
            "step_finished" => format!(
                "passed={} attempts={} {}ms",
                e["passed"], e["attempts"], e["elapsed_ms"]
            ),
            _ => String::new(),
        };
        eprintln!("seq {:>2} {} {}", v["seq"], e["type"].as_str().unwrap(), detail);
    }
}
