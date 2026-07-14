//! Stage 3 of docs/design/contract-driven-implementation-experiment.md: first point
//! contract-driven generation is actually compiled and tested for real, not eyeballed.
//!
//! **This is an experimental probe, not production wiring.** Same guarantee as Stages 1 and 2:
//! no import from or call into `canopy-cli`'s `implement` command, `plan.rs`, or `execute.rs`.
//! Standalone binary, not part of the `canopy` CLI or the `canopy-llm` library's public surface.
//! Changes no existing behavior.
//!
//! "Reuse execute.rs's test-run/fix-loop machinery" (the design doc's own Stage 3 description)
//! is satisfied here without modifying or importing execute.rs at all: `canopy_llm::fix_file` is
//! the actual underlying production function execute.rs's fix loop calls — reusing it directly
//! reuses the real mechanism, not a copy of it, while leaving execute.rs itself untouched. The
//! "test-run" half is done for real too, via a real, separate Maven project (this experiment's
//! own scratch harness, not the dogfooding project's real service tree) and `mvn test`, not by
//! reading LLM output and guessing whether it would compile.
//!
//! ## What's reused vs. what's experimental
//!
//! Reused, unmodified: `canopy_storage::load_contracts`, `resolve_implementation_target`,
//! `abstract_layer_for_kind`, `skill_for_technology` (Stage 1/2's functions), plus
//! `canopy_llm::fix_file` (new to this stage — the real fix-loop function, called directly).
//!
//! Experimental, novel to this file: shelling out to a real `mvn test` run and parsing its
//! Surefire summary line, and the one-bounded-fix-attempt loop (generate → compile/test for real
//! → on failure, one `fix_file` call → re-test once — mirrors the shape of the real fix loop
//! without reusing its iteration/retry policy, which lives in execute.rs and isn't touched).
//!
//! ## Inputs allowed vs. disallowed — unchanged from Stage 1/2
//!
//! Allowed: every contract sharing the resolved target, the resolved target itself, the
//! tech-stack skill for that layer, and — new to this stage — the REAL compiler/test error text
//! (an objective fact about the generated code, not a story/spec/ADR input). Disallowed, and
//! absent from this file: story, full scenario list, `entity_schema`, ADRs, OpenAPI, exploratory
//! tool access.
//!
//! ## Run
//!
//! Requires the scratch Maven harness at `<maven-root>/services/manufacturer-service/` (its own
//! `pom.xml`, not part of this repo) and a local llama-server on :8080.
//!
//! ```sh
//! cargo run -p canopy-llm --example contract_driven_stage3_experiment -- <project-root> <maven-root> <story-id>
//! ```

use canopy_core::{AgentLlmConfig, BehaviorKind, Contract, LlmProvider};
use canopy_llm::{abstract_layer_for_kind, fix_file, resolve_implementation_target, skill_for_technology, FixAttempt, LlmClient};

fn target_for(contract: &Contract, tech: &str, pkg: &str, service_name: &str) -> Option<String> {
    let kind = contract.kind.as_ref()?;
    let entity = contract.entity.as_deref()?;
    let layer = abstract_layer_for_kind(kind);
    resolve_implementation_target(tech, pkg, service_name, layer, entity, None)
}

fn render_contract_facts(contracts: &[&Contract]) -> String {
    contracts.iter().enumerate().map(|(i, c)| {
        format!(
            "{}. kind={kind:?}, entity={entity:?}, member={member:?}, mandatory={mandatory:?}\n   required behavior:\n{tests}",
            i + 1,
            kind = c.kind, entity = c.entity, member = c.member, mandatory = c.mandatory,
            tests = c.required_tests.iter().map(|t| format!("     - \"{t}\"")).collect::<Vec<_>>().join("\n"),
        )
    }).collect::<Vec<_>>().join("\n\n")
}

fn test_prompt(contracts: &[&Contract], target_path: &str, tech_rules: &str) -> String {
    format!(
        "You are generating a JUnit 5 test class, using ONLY the information below — do not \
         assume any requirement not explicitly stated here, and do not test anything beyond what \
         these contracts state.\n\
         \n\
         Implementation file (not yet written): {target_path}\n\
         \n\
         The following contracts, TOGETHER, are the complete authorized scope of this ONE file — \
         nothing exists for this file beyond what is listed below:\n\
         \n\
         {facts}\n\
         \n\
         {tech_rules}\n\
         \n\
         Write ONE test class covering every required-behavior line above — one @Test method per \
         line, nothing else, nothing extra.\n",
        target_path = target_path, facts = render_contract_facts(contracts), tech_rules = tech_rules,
    )
}

fn impl_prompt(contracts: &[&Contract], target_path: &str, tech_rules: &str, test_code: &str) -> String {
    format!(
        "You are generating exactly one file, using ONLY the information below — do not assume \
         any requirement not explicitly stated here.\n\
         \n\
         Target file: {target_path}\n\
         \n\
         The following contracts, TOGETHER, are the COMPLETE authorized scope of this file — \
         nothing exists for this file beyond what is listed below. Implement exactly this \
         combined scope: do NOT invent any field, method, or annotation that has no corresponding \
         line among these contracts.\n\
         \n\
         {facts}\n\
         \n\
         {tech_rules}\n\
         \n\
         Test this file must satisfy:\n\
         ```java\n{test_code}\n```\n\
         \n\
         Write the complete file content.\n",
        target_path = target_path, facts = render_contract_facts(contracts), tech_rules = tech_rules, test_code = test_code,
    )
}

/// Strips a single leading/trailing fenced code block, if present — the initial generation
/// prompts above don't request `canopy_summary_contract()`'s summary/deviations footer (kept
/// identical to Stage 1/2's prompts on purpose), so the raw response is just possibly-fenced code.
/// Finds a fenced code block ANYWHERE in the response, not just at the very start — a live bug
/// in an earlier version of this function assumed the fence always opens the response and wrote
/// the model's leading prose (e.g. "Sure, here is the test class...") straight into the .java
/// file, verbatim backticks included, producing a real `javac` "illegal character: `" failure
/// that had nothing to do with the contract or the skill being tested. Falls back to the raw,
/// trimmed text only when no fence is found at all.
fn extract_code(raw: &str) -> String {
    let trimmed = raw.trim();
    let Some(open) = trimmed.find("```") else { return trimmed.to_string() };
    let after_open = &trimmed[open + 3..];
    let after_open = after_open.trim_start_matches(|c: char| c.is_alphanumeric());
    let after_open = after_open.strip_prefix('\n').unwrap_or(after_open);
    match after_open.find("```") {
        Some(close) => after_open[..close].trim().to_string(),
        None => after_open.trim().to_string(),
    }
}

/// Real `mvn -q clean test` — objective compile+test evidence, not eyeballed LLM output.
/// `-q` still lets Surefire's own summary line and any compiler error through on failure.
fn run_maven_test(maven_root: &str) -> (bool, String) {
    let output = std::process::Command::new("mvn")
        .args(["-q", "clean", "test"])
        .current_dir(format!("{maven_root}/services/manufacturer-service"))
        .output()
        .unwrap_or_else(|e| panic!("failed to invoke mvn: {e}"));
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    (output.status.success(), combined)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let project_root = args.next().unwrap_or_else(|| ".".to_string());
    let maven_root = args.next().unwrap_or_else(|| "./stage3-maven".to_string());
    let story_id = args.next().unwrap_or_else(|| "manufacturer-001".to_string());

    let tech = "Spring Boot";
    let pkg = "manufacturer_service";
    let service_name = "manufacturer-service";

    std::env::set_current_dir(&project_root)
        .unwrap_or_else(|e| panic!("cannot cd into '{project_root}': {e}"));
    let contract_set = canopy_storage::load_contracts(&story_id)
        .unwrap_or_else(|e| panic!("failed to load contracts.yaml for '{story_id}': {e}"));

    let name_contract_target = contract_set.contracts.iter()
        .find(|c| c.kind.as_ref() == Some(&BehaviorKind::Validation) && c.member.as_deref() == Some("name"))
        .and_then(|c| target_for(c, tech, pkg, service_name))
        .unwrap_or_else(|| panic!("no ManufacturerNameValidation contract found in {story_id}'s contracts.yaml"));
    let group: Vec<&Contract> = contract_set.contracts.iter()
        .filter(|c| target_for(c, tech, pkg, service_name).as_deref() == Some(name_contract_target.as_str()))
        .collect();

    println!("=== Contracts sharing target '{name_contract_target}' ===\n");
    for c in &group {
        println!("- {} (kind={:?}, member={:?}, mandatory={:?})", c.id, c.kind, c.member, c.mandatory);
    }
    println!();

    // `abstract_layer_for_kind` is the tech-agnostic concept used to RESOLVE the file target
    // (`target_for`, above) — it is NOT necessarily the same string a tech-stack skill's
    // `layer_rules` are keyed under for a given real path (see file_targets.rs's own doc comment:
    // "Not the same string space as detect_layer() in every case"). The skill lookup must use
    // the REAL layer `detect_layer()` computes for the actual resolved path — for Spring Boot,
    // that's "domain", not the abstract "model" — the same distinction the Spring Boot skill fix
    // itself depends on. Using the abstract name here directly (an earlier version of this
    // experiment did) silently fetches an empty/wrong-layer render, invisibly to the model.
    let layer = canopy_llm::detect_layer(&name_contract_target);
    let tech_rules = skill_for_technology(tech, pkg, pkg, service_name, layer);

    let client = LlmClient::from_agent_config(
        &AgentLlmConfig { provider: LlmProvider::Ollama, model: "qwen2.5-coder:14b".to_string(), base_url: Some("http://localhost:8080".to_string()) },
        false,
    );

    let t_prompt = test_prompt(&group, &name_contract_target, &tech_rules);
    let test_code = extract_code(&client.complete_large(&t_prompt).unwrap_or_else(|e| panic!("test generation failed: {e}")));
    println!("=== Generated test ===\n{test_code}\n");

    let i_prompt = impl_prompt(&group, &name_contract_target, &tech_rules, &test_code);
    let mut impl_code = extract_code(&client.complete_large(&i_prompt).unwrap_or_else(|e| panic!("implementation generation failed: {e}")));
    println!("=== Generated implementation (attempt 1) ===\n{impl_code}\n");

    let maven_test_path = format!("{maven_root}/services/manufacturer-service/src/test/java/manufacturer_service/domain/ManufacturerTest.java");
    let maven_impl_path = format!("{maven_root}/services/manufacturer-service/src/main/java/manufacturer_service/domain/Manufacturer.java");
    std::fs::write(&maven_test_path, &test_code).unwrap_or_else(|e| panic!("failed to write test file: {e}"));
    std::fs::write(&maven_impl_path, &impl_code).unwrap_or_else(|e| panic!("failed to write impl file: {e}"));

    println!("=== Real compile + test run (attempt 1) ===\n");
    let (passed, output) = run_maven_test(&maven_root);
    println!("{output}\n");
    println!("attempt 1 result: {}\n", if passed { "PASS" } else { "FAIL" });

    if !passed {
        println!("=== Applying one bounded fix_file attempt (real production function) ===\n");
        let fixed = fix_file(
            &client,
            &maven_impl_path,
            &impl_code,
            &output,
            &[],
            &[],
            &tech_rules,
            "",
            &[FixAttempt { summary: None, resulting_error: Some(output.clone()), is_noop: false }],
            &[],
        ).unwrap_or_else(|e| panic!("fix_file failed: {e}"));

        impl_code = fixed.content;
        println!("=== Generated implementation (after fix) ===\n{impl_code}\n");
        std::fs::write(&maven_impl_path, &impl_code).unwrap_or_else(|e| panic!("failed to write fixed impl file: {e}"));

        println!("=== Real compile + test run (attempt 2, post-fix) ===\n");
        let (passed2, output2) = run_maven_test(&maven_root);
        println!("{output2}\n");
        println!("attempt 2 result: {}\n", if passed2 { "PASS" } else { "FAIL" });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_code_handles_a_fence_with_no_leading_prose() {
        assert_eq!(extract_code("```java\npublic class Foo {}\n```"), "public class Foo {}");
    }

    /// Regression test for the run-2 bug: leading prose before the fence used to be written
    /// verbatim (backticks included) straight into the .java file.
    #[test]
    fn extract_code_strips_leading_prose_before_the_fence() {
        let raw = "Sure, here is the class.\n\n```java\npublic class Foo {}\n```\n\nHope that helps!";
        assert_eq!(extract_code(raw), "public class Foo {}");
    }

    #[test]
    fn extract_code_falls_back_to_raw_text_when_no_fence_exists() {
        assert_eq!(extract_code("public class Foo {}"), "public class Foo {}");
    }
}
