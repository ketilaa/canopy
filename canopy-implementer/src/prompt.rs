//! What the model is told. Built from scratch for every call: no conversation is kept between
//! calls, so a retry carries exactly what it needs and nothing stale.

use crate::files::GeneratedFile;
use crate::implementer::{Failure, StepRequest};
use crate::llm::Message;
use crate::observe::Stage;

const RULES: &str = "\
You write source files for a Java 21 Spring Boot 3.4 Maven project, one plan step at a time.
ALWAYS return every file the step needs, each one complete.
ALWAYS keep the existing code and tests of a file you change, except what the step asks you to change.
ALWAYS use paths relative to the project root, such as src/main/java/<package path>/<Class>.java.
ALWAYS put one public type in each file, in a file named after that type.
ALWAYS write tests with JUnit 5 (org.junit.jupiter.api) under src/test/java, in the same package as the class under test.
NEVER change pom.xml or any file the step does not name or require.";

/// `current` is what the step's existing files contain right now (a file that does not exist yet is simply absent).
const TOOL_RULES: &str = "\
You change files in a Java 21 Spring Boot 3.4 Maven project, one task at a time.
ALWAYS use glob and read_file to find and read the files you need.
ALWAYS read an existing file with read_file before you change it.
ALWAYS finish by calling submit_files with the complete content of every file you create or change.
ALWAYS keep the existing code and tests of a file you change, except what the task asks you to change.
ALWAYS include a test for every behaviour you add or change.
ALWAYS put one public type in each file, in a file named after that type.
ALWAYS write tests with JUnit 5 (org.junit.jupiter.api) under src/test/java, in the same package as the class under test.
NEVER change pom.xml or any file the task does not require.";

pub fn messages(step: &StepRequest, current: &[GeneratedFile], previous: Option<&Failure>) -> Vec<Message> {
    let mut user = format!("## Step\n{}\n\n## Project\n{}\n", step.instruction, step.context);
    if !current.is_empty() {
        user.push_str("\n## Current files\n");
        for file in current {
            user.push_str(&format!("### {}\n```\n{}\n```\n", file.path, file.content));
        }
    }
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

/// The rules and the task for a tool conversation. The model is not shown the files: it has to find
/// and read them.
pub fn tool_messages(step: &StepRequest, previous: Option<&Failure>) -> (String, String) {
    let mut user = format!("## Task\n{}\n\n## Project\n{}\n", step.instruction, step.context);
    if let Some(failure) = previous {
        if let Some(files) = &failure.files {
            user.push_str("\n## Your previous answer\n");
            for file in &files.files {
                user.push_str(&format!("### {}\n```\n{}\n```\n", file.path, file.content));
            }
        }
        user.push_str(&format!("\n## {}\n{}\n", heading(failure.stage), failure.feedback));
        user.push_str(match failure.stage {
            Stage::Budget => {
                "\nYou spent your tool calls reading. Read each file you need at most once, then call submit_files \
                 with your change.\n"
            }
            Stage::Protocol => {
                "\nDo not answer in prose. Read what you need, then call submit_files with the complete files.\n"
            }
            _ => "\nStart again: read the files again before you change them, then call submit_files.\n",
        });
    }
    (TOOL_RULES.to_string(), user)
}

fn heading(stage: Stage) -> &'static str {
    match stage {
        Stage::Compile => "The build failed to compile",
        Stage::Test => "The tests failed",
        Stage::Parse => "Your answer could not be used",
        Stage::Write => "Your answer could not be written",
        Stage::Truncated => "Your answer was cut off",
        Stage::Llm => "The model call failed",
        Stage::Budget => "You used up your tool calls without submitting",
        Stage::Protocol => "You did not call submit_files",
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
            files: vec![],
            kind: Default::default(),
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
        let m = messages(&step(), &[], None);
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
        let m = messages(&step(), &[], Some(&failure(Stage::Compile, "G.java:1:1: boom", true)));
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
        let heading = |stage| {
            messages(&step(), &[], Some(&failure(stage, "x", false)))[1]
                .content
                .clone()
        };
        assert!(heading(Stage::Test).contains("The tests failed"));
        assert!(heading(Stage::Parse).contains("could not be used"));
        assert!(heading(Stage::Truncated).contains("cut off"));
        assert!(heading(Stage::Write).contains("could not be written"));
        assert!(heading(Stage::Llm).contains("model call failed"));
    }

    #[test]
    fn a_retry_without_files_does_not_show_a_previous_answer() {
        let m = messages(&step(), &[], Some(&failure(Stage::Parse, "not json", false)));
        assert!(!m[1].content.contains("previous answer"));
        assert!(m[1].content.contains("not json"));
    }
    fn greeter() -> GeneratedFile {
        GeneratedFile {
            path: "src/main/java/Greeter.java".into(),
            content: "class Greeter { /* original */ }".into(),
        }
    }

    #[test]
    fn existing_files_are_shown_with_path_and_content() {
        let m = messages(&step(), &[greeter()], None);
        let user = &m[1].content;
        assert!(user.contains("## Current files"));
        assert!(user.contains("### src/main/java/Greeter.java"));
        assert!(user.contains("class Greeter { /* original */ }"));
    }

    #[test]
    fn there_is_no_current_files_section_when_the_step_changes_nothing_existing() {
        assert!(!messages(&step(), &[], None)[1].content.contains("Current files"));
    }

    #[test]
    fn a_retry_shows_the_current_files_and_the_previous_answer_as_different_things() {
        let m = messages(&step(), &[greeter()], Some(&failure(Stage::Compile, "boom", true)));
        let user = &m[1].content;
        let current = user.find("## Current files").unwrap();
        let previous = user.find("## Your previous answer").unwrap();
        assert!(
            current < previous,
            "the state to change comes before the failed attempt at changing it"
        );
        assert!(user.contains("/* original */") && user.contains("class G {}"));
    }

    #[test]
    fn the_rules_tell_the_model_to_keep_what_the_step_does_not_change() {
        let rules = &messages(&step(), &[], None)[0].content;
        assert!(rules.contains("ALWAYS keep"), "{rules}");
    }
    #[test]
    fn the_tool_prompt_states_the_rules_the_task_and_the_project_and_does_not_show_files() {
        let (system, user) = tool_messages(&step(), None);
        for rule in [
            "ALWAYS read an existing file with read_file before you change it",
            "ALWAYS finish by calling submit_files",
            "ALWAYS include a test for every behaviour you add or change",
            "ALWAYS keep the existing code and tests",
        ] {
            assert!(system.contains(rule), "missing: {rule}\n{system}");
        }
        assert!(
            user.contains("Create a Greeter class with greet(String).")
                && user.contains("Root package: com.example.demo")
        );
        assert!(!user.contains("Current files") && !user.contains("previous answer"));
    }

    #[test]
    fn a_tool_retry_shows_the_previous_answer_and_what_failed_and_says_to_read_again() {
        let (_, user) = tool_messages(&step(), Some(&failure(Stage::Compile, "G.java:1:1: boom", true)));
        assert!(user.contains("Your previous answer") && user.contains("class G {}"));
        assert!(user.contains("The build failed to compile") && user.contains("G.java:1:1: boom"));
        assert!(user.contains("read the files again"), "{user}");
    }
    #[test]
    fn a_budget_retry_says_to_read_each_file_once_and_then_submit_not_to_read_again() {
        let (_, user) = tool_messages(&step(), Some(&failure(Stage::Budget, "You used 12 tool calls.", false)));
        assert!(user.contains("at most once") && user.contains("submit_files"), "{user}");
        assert!(!user.contains("read the files again"), "{user}");
    }

    #[test]
    fn a_protocol_retry_says_not_to_answer_in_prose() {
        let (_, user) = tool_messages(
            &step(),
            Some(&failure(Stage::Protocol, "You answered in prose.", false)),
        );
        assert!(
            user.contains("Do not answer in prose") && user.contains("submit_files"),
            "{user}"
        );
    }
}
