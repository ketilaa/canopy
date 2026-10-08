//! What the model is told. Built from scratch for every call: no conversation is kept between
//! calls, so a retry carries exactly what it needs and nothing stale.

use crate::implementer::{Failure, StepRequest};
use crate::llm::Message;
use crate::observe::Stage;

const RULES: &str = "\
You write source files for a Java 21 Spring Boot 3.4 Maven project, one plan step at a time.
ALWAYS return every file the step needs, each one complete.
ALWAYS use paths relative to the project root, such as src/main/java/<package path>/<Class>.java.
ALWAYS put one public type in each file, in a file named after that type.
ALWAYS write tests with JUnit 5 (org.junit.jupiter.api) under src/test/java, in the same package as the class under test.
NEVER change pom.xml or any file the step does not name or require.";

pub fn messages(step: &StepRequest, previous: Option<&Failure>) -> Vec<Message> {
    let mut user = format!("## Step\n{}\n\n## Project\n{}\n", step.instruction, step.context);
    if let Some(failure) = previous {
        if let Some(files) = &failure.files {
            user.push_str("\n## Your previous answer\n");
            for file in &files.files {
                user.push_str(&format!("### {}\n```\n{}\n```\n", file.path, file.content));
            }
        }
        user.push_str(&format!("\n## {}\n{}\n", heading(failure.stage), failure.feedback));
        user.push_str("\nReturn the complete corrected files.\n");
    }
    vec![Message::system(RULES), Message::user(&user)]
}

fn heading(stage: Stage) -> &'static str {
    match stage {
        Stage::Compile => "The build failed to compile",
        Stage::Test => "The tests failed",
        Stage::Parse => "Your answer could not be used",
        Stage::Write => "Your answer could not be written",
        Stage::Truncated => "Your answer was cut off",
        Stage::Llm => "The model call failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::{GeneratedFile, GeneratedFiles};

    fn step() -> StepRequest {
        StepRequest {
            id: "s1".into(),
            instruction: "Create a Greeter class with greet(String).".into(),
            context: "Root package: com.example.demo".into(),
        }
    }

    fn failure(stage: Stage, feedback: &str, with_files: bool) -> Failure {
        Failure {
            stage,
            feedback: feedback.into(),
            files: with_files.then(|| GeneratedFiles {
                files: vec![GeneratedFile {
                    path: "src/main/java/G.java".into(),
                    content: "class G {}".into(),
                }],
            }),
        }
    }

    #[test]
    fn the_first_prompt_has_the_rules_the_step_and_the_project_and_nothing_about_failures() {
        let m = messages(&step(), None);
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].role, "system");
        assert!(m[0].content.contains("ALWAYS") && m[0].content.contains("JUnit 5"));
        assert_eq!(m[1].role, "user");
        assert!(m[1].content.contains("Create a Greeter class with greet(String)."));
        assert!(m[1].content.contains("Root package: com.example.demo"));
        assert!(!m[1].content.contains("previous answer"));
    }

    #[test]
    fn a_retry_after_a_compile_failure_shows_the_previous_files_and_the_errors() {
        let m = messages(&step(), Some(&failure(Stage::Compile, "G.java:1:1: boom", true)));
        assert_eq!(
            m.len(),
            2,
            "no conversation is kept: still one system and one user message"
        );
        let user = &m[1].content;
        assert!(user.contains("Create a Greeter class"), "the step must still be there");
        assert!(user.contains("src/main/java/G.java") && user.contains("class G {}"));
        assert!(user.contains("The build failed to compile"));
        assert!(user.contains("G.java:1:1: boom"));
    }

    #[test]
    fn each_failure_stage_has_its_own_heading() {
        let heading = |stage| messages(&step(), Some(&failure(stage, "x", false)))[1].content.clone();
        assert!(heading(Stage::Test).contains("The tests failed"));
        assert!(heading(Stage::Parse).contains("could not be used"));
        assert!(heading(Stage::Truncated).contains("cut off"));
        assert!(heading(Stage::Write).contains("could not be written"));
        assert!(heading(Stage::Llm).contains("model call failed"));
    }

    #[test]
    fn a_retry_without_files_does_not_show_a_previous_answer() {
        let m = messages(&step(), Some(&failure(Stage::Parse, "not json", false)));
        assert!(!m[1].content.contains("previous answer"));
        assert!(m[1].content.contains("not json"));
    }
}
