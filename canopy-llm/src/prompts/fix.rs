use crate::client::{LlmClient, LlmError};
use super::summary::{canopy_summary_contract, split_step_response, StepResult};

/// A record of one prior fix attempt on a file — what the model reported doing, and what
/// actually happened. Deliberately does NOT store the attempt's file content: embedding the
/// full source of every prior attempt made the prompt grow by a whole file per iteration, and
/// the model's own summary is not always trustworthy (it has claimed a fix when the returned
/// content was byte-identical to the input) — but the RESULTING ERROR is real, verifiable
/// evidence of the outcome regardless of what the model said it did.
#[derive(Clone)]
pub struct FixAttempt {
    pub summary: Option<String>,
    pub resulting_error: Option<String>,
    /// True when this attempt's output was byte-identical to what was fed in — the model
    /// made no change at all, whatever it claimed in its summary.
    pub is_noop: bool,
}

fn fix_prompt(
    file_path: &str,
    content: &str,
    errors: &str,
    existing_files: &[String],
    referenced_files: &[(String, String)],
    skill: &str,
    arch_skills: &str,
    prior_attempts: &[FixAttempt],
) -> String {
    let ext = std::path::Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let lang = match ext {
        "java" => "Java",
        "ts" | "tsx" => "TypeScript",
        "xml" => "XML",
        _ => "source",
    };
    // Detect TDD context from file path and content. Suffix-based for TS/TSX — tests are
    // co-located next to their implementation file, not under a separate directory.
    let is_test_file = file_path.contains("/src/test/java/")
        || file_path.ends_with(".test.ts")
        || file_path.ends_with(".test.tsx")
        || file_path.ends_with("Test.java")
        || file_path.ends_with("IT.java");
    let is_stub = !is_test_file
        && (content.contains("throw new Error('not implemented')")
            || content.contains("throw new Error(\"not implemented\")")
            || (ext == "java" && content.contains("return null;") && !content.contains("if ") && !content.contains("for ")));

    let tdd_rules = if is_test_file {
        "\n## TDD — this is a test file\n\
         - Do NOT change, weaken, or remove any assertions\n\
         - Do NOT remove or rename test cases\n\
         - Only fix: import paths, missing type annotations, syntax errors\n\
         - If a test imports a symbol that does not exist yet, add a minimal import/mock — do NOT delete the test"
    } else if is_stub {
        "\n## TDD — this is a Red-phase stub\n\
         - Do NOT implement any business logic\n\
         - Preserve all `throw new Error('not implemented')` or `return null;` bodies\n\
         - Only fix: missing exports, import paths, type signatures, constructor declarations\n\
         - The stub must compile and satisfy the test's type contract — nothing more"
    } else {
        ""
    };

    let extra_rules = if ext == "java" {
        "\n- A Java source file contains exactly one top-level type declaration\n\
         - Nothing may appear after the final closing brace of the top-level class/interface/enum/record\n\
         - Remove any stray import statements, package declarations, or class bodies that appear after that brace\n\
         - The file must begin with the package declaration\n\
         - Constructor mismatch: look at the referenced file to find the available constructor(s).\n\
           If only a no-args constructor is present, use: Foo f = new Foo(); f.setField(value); ...\n\
           Do NOT call a multi-arg constructor that is not declared in the referenced file.\n\
           Do NOT add a new constructor to a class that lives in a referenced file — only fix THIS file."
    } else if file_path.ends_with("pom.xml") {
        "\n- Only use dependencies from well-known Maven Central groupIds:\n\
           org.springframework.boot, com.h2database, org.projectlombok, com.fasterxml.jackson.*,\n\
           org.junit.*, org.assertj.*, org.mockito.*\n\
         - Remove any dependency whose groupId is derived from this project — those are not published artifacts\n\
         - Domain event classes (e.g. ProductCreated) are in the service's own domain/ package; they are NOT\n\
           a separate JAR — remove any such dependency\n\
         - ApplicationEventPublisher is in spring-context (via spring-boot-starter); no extra dep needed\n\
         - For javax.validation use spring-boot-starter-validation\n\
         - For javax.persistence use spring-boot-starter-data-jpa\n\
         - Do not remove existing valid dependencies\n\
         - Keep the XML well-formed and end with </project>"
    } else {
        ""
    };
    let files_section = if !existing_files.is_empty() {
        format!(
            "\nExisting files in the project (use for correct import paths):\n{}\n",
            existing_files.iter().map(|f| format!("  {f}")).collect::<Vec<_>>().join("\n")
        )
    } else {
        String::new()
    };
    // For TypeScript errors: include the content of related files so the model can fix
    // cross-file type mismatches (e.g. a missing props interface in an imported component).
    let referenced_section = if !referenced_files.is_empty() {
        let parts: Vec<String> = referenced_files.iter()
            .map(|(path, c)| format!("--- {} ---\n{}", path, c))
            .collect();
        let label = if ext == "java" {
            "Referenced files — check these for available constructors, setter methods, and field types \
             before writing any new() calls or method invocations:"
        } else {
            "Referenced files — check these for the correct component signatures and exported types:"
        };
        format!(
            "\n{label}\n\n{}\n",
            parts.join("\n\n")
        )
    } else {
        String::new()
    };
    let skill_section = if skill.is_empty() {
        String::new()
    } else {
        format!("\n{skill}\n")
    };
    let arch_section = if arch_skills.is_empty() {
        String::new()
    } else {
        format!("\n{arch_skills}\n")
    };
    let attempts_section = if prior_attempts.is_empty() {
        String::new()
    } else {
        let entries = prior_attempts.iter().enumerate()
            .map(|(i, a)| {
                let what = if a.is_noop {
                    "Made NO changes to the file at all.".to_string()
                } else {
                    a.summary.clone().unwrap_or_else(|| "(no summary reported)".to_string())
                };
                let outcome = a.resulting_error.as_deref()
                    .map(|e| format!("Still failed with:\n{e}"))
                    .unwrap_or_else(|| "Outcome unknown.".to_string());
                format!("### Attempt {}\n{what}\n{outcome}", i + 1)
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        format!(
            "\n## Previous attempts that did NOT fix the errors\n\
             Do NOT repeat any of these — each one was tried and failed. If an attempt made NO\n\
             changes, you MUST make a concrete, different code change this time:\n\n{entries}\n"
        )
    };
    format!(
        "Fix the {lang} file below so that all listed errors are resolved.\n\
         \n\
         File: {file_path}\n\
         {files_section}\
         {referenced_section}\
         {skill_section}\
         {arch_section}\
         {attempts_section}\n\
         Errors:\n\
         {errors}\n\
         \n\
         Current content:\n\
         {content}\n\
         \n\
         {tdd_rules}\
         Rules:\n\
         - Write ONLY the corrected file content first — no prose, no markdown fences, no explanations\
         {extra_rules}\n\
         - Preserve all correct logic; only fix what the errors report\n\
         - Only import from modules that exist in the project files listed above\n\
         \n\
         {contract}",
        contract = canopy_summary_contract(),
    )
}

pub fn fix_file(
    client: &LlmClient,
    file_path: &str,
    content: &str,
    errors: &str,
    existing_files: &[String],
    referenced_files: &[(String, String)],
    skill: &str,
    arch_skills: &str,
    prior_attempts: &[FixAttempt],
) -> Result<StepResult, LlmError> {
    let raw = client.complete_large(&fix_prompt(file_path, content, errors, existing_files, referenced_files, skill, arch_skills, prior_attempts))?;
    Ok(split_step_response(&raw))
}

