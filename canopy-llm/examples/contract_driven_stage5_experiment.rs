//! Stage 5 of docs/design/contract-driven-implementation-experiment.md: A/B test — does
//! contract-scoped generation actually beat production's own real prompt, or only match a
//! hand-built substitute no one ships? Full design in that document's "Stage 5 Design" section;
//! this file implements it literally.
//!
//! **Still a standalone experiment.** No import from or call into `canopy-cli`'s `implement`
//! command, `plan.rs`, or `execute.rs`. Calls production's real, unmodified public functions
//! directly (`generate_unit_test_stub`, `execute_implementation_with_test`) — reusing the actual
//! mechanism, not a copy of it.
//!
//! ## Path A (production) vs Path B (contract-scoped)
//!
//! Both target the same real `manufacturer-001` case (the same six-contract
//! `Manufacturer.java` group Stages 2-4 all used), written into two SIBLING sub-packages of the
//! same real Maven harness (`<maven-root>/services/manufacturer-service/`) so both paths'
//! output coexists for inspection rather than overwriting each other:
//! - Path A: `manufacturer_service.domain.production.Manufacturer{,Test}`
//! - Path B: `manufacturer_service.domain.contractscoped.Manufacturer{,Test}`
//!
//! Path A calls `generate_unit_test_stub`/`execute_implementation_with_test` with REAL data
//! loaded from disk (story, spec — full entity_schema + all 12 real scenarios, openapi, ADRs,
//! services) — exactly what `canopy implement` would supply today. Path B reuses Stage 2/3's own
//! prompts, unchanged, against the same six real contracts. The `ImplementationStep.description`
//! is held IDENTICAL between the two paths on purpose, so the only experimental variable is
//! prompt context, not incidental wording.
//!
//! Runs path A's 3 iterations to completion, deletes its sub-package entirely (not just
//! overwrites), THEN runs path B's 3 iterations — so a broken final path-A file can never
//! contaminate path B's compile step by sitting in the same `mvn clean test` source tree.
//!
//! ## Run
//!
//! ```sh
//! cargo run -p canopy-llm --example contract_driven_stage5_experiment -- <project-root> <maven-root> <story-id>
//! ```

use canopy_core::{
    AgentLlmConfig, BehaviorKind, Contract, ImplementationStep, LlmProvider, StepStatus,
};
use canopy_llm::{
    abstract_layer_for_kind, execute_implementation_with_test, generate_unit_test_stub,
    resolve_implementation_target, skill_for_technology, skills_for_architecture, LlmClient,
};

const TECH: &str = "Spring Boot";
const PKG: &str = "manufacturer_service";
const SERVICE_NAME: &str = "manufacturer-service";
const DESCRIPTION: &str = "Constructs and validates Manufacturer.";

// ── Contract-scoped path (B) — reused verbatim from Stage 2/3 ──────────────────────────────

fn target_for(contract: &Contract) -> Option<String> {
    let kind = contract.kind.as_ref()?;
    let entity = contract.entity.as_deref()?;
    let layer = abstract_layer_for_kind(kind);
    resolve_implementation_target(TECH, PKG, SERVICE_NAME, layer, entity, None)
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

fn contract_test_prompt(contracts: &[&Contract], target_path: &str, tech_rules: &str) -> String {
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

fn contract_impl_prompt(contracts: &[&Contract], target_path: &str, tech_rules: &str, test_code: &str) -> String {
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

/// Finds a fenced code block anywhere in the response (fixed in Stage 3 after a live bug: leading
/// prose before the fence used to be written verbatim, backticks included, into the .java file).
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

fn parse_surefire_summary(output: &str) -> String {
    output.lines()
        .find(|l| l.contains("Tests run:"))
        .unwrap_or("(no \"Tests run:\" summary line found — likely a compile failure; see full output above)")
        .trim()
        .to_string()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let project_root = args.next().unwrap_or_else(|| ".".to_string());
    let maven_root = args.next().unwrap_or_else(|| "./stage3-maven".to_string());
    let story_id = args.next().unwrap_or_else(|| "manufacturer-001".to_string());

    let service_root = format!("{maven_root}/services/manufacturer-service");
    let prod_main_dir = format!("{service_root}/src/main/java/manufacturer_service/domain/production");
    let prod_test_dir = format!("{service_root}/src/test/java/manufacturer_service/domain/production");
    let cs_main_dir = format!("{service_root}/src/main/java/manufacturer_service/domain/contractscoped");
    let cs_test_dir = format!("{service_root}/src/test/java/manufacturer_service/domain/contractscoped");
    for d in [&prod_main_dir, &prod_test_dir, &cs_main_dir, &cs_test_dir] {
        std::fs::create_dir_all(d).unwrap_or_else(|e| panic!("failed to create {d}: {e}"));
    }
    let prod_impl_path = format!("{prod_main_dir}/Manufacturer.java");
    let prod_test_path = format!("{prod_test_dir}/ManufacturerTest.java");
    let cs_impl_path = format!("{cs_main_dir}/Manufacturer.java");
    let cs_test_path = format!("{cs_test_dir}/ManufacturerTest.java");

    std::env::set_current_dir(&project_root)
        .unwrap_or_else(|e| panic!("cannot cd into '{project_root}': {e}"));

    // ── Load real data, once, shared by both paths ──────────────────────────────────────
    let stories = canopy_storage::load_user_stories().expect("failed to load stories.yaml");
    let story = stories.stories.iter().find(|s| s.id == story_id)
        .unwrap_or_else(|| panic!("story '{story_id}' not found")).clone();
    let spec = canopy_storage::load_story_spec(&story_id).expect("failed to load spec.yaml");
    let openapi_yaml = canopy_storage::load_story_openapi(&story_id).expect("failed to load openapi.yaml")
        .unwrap_or_else(|| panic!("no openapi.yaml saved for '{story_id}'"));
    let adrs = canopy_storage::load_all_adrs().unwrap_or_default();
    let services = canopy_storage::load_services_registry().expect("failed to load services.yaml");
    let contract_set = canopy_storage::load_contracts(&story_id).expect("failed to load contracts.yaml");
    let service_packages: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    let name_target = contract_set.contracts.iter()
        .find(|c| c.kind.as_ref() == Some(&BehaviorKind::Validation) && c.member.as_deref() == Some("name"))
        .and_then(target_for)
        .unwrap_or_else(|| panic!("no ManufacturerNameValidation contract found in {story_id}'s contracts.yaml"));
    let group: Vec<&Contract> = contract_set.contracts.iter()
        .filter(|c| target_for(c).as_deref() == Some(name_target.as_str()))
        .collect();

    println!("=== Stage 5: {} contracts sharing '{}' ===\n", group.len(), name_target);

    let arch_skills = skills_for_architecture(&adrs, TECH);
    let tech_rules = skill_for_technology(TECH, PKG, PKG, SERVICE_NAME, "domain");

    // Prompt-size proxy (§ Stage 5 Design, metric 4) — production's actual prompt-building
    // functions are private, so this measures the same underlying data volume rather than the
    // exact final prompt byte count. Both sides share the identical tech_rules text, so it
    // cancels out in the comparison; what differs is entity_schema+scenarios+arch_skills (A)
    // versus the six contracts' own rendered facts (B).
    let entity_schema_yaml = serde_yaml::to_string(&spec.entity_schema).unwrap_or_default();
    let scenarios_yaml = serde_yaml::to_string(&spec.scenarios).unwrap_or_default();
    let production_content_size = entity_schema_yaml.len() + scenarios_yaml.len() + arch_skills.len() + tech_rules.len();
    let contract_content_size = render_contract_facts(&group).len() + tech_rules.len();
    println!(
        "Prompt-content-size proxy: production ~{production_content_size} chars \
         (entity_schema {} + {} scenarios {} + arch_skills {} + tech_rules {}), \
         contract-scoped ~{contract_content_size} chars (6 contracts' facts {} + tech_rules {}).\n",
        entity_schema_yaml.len(), spec.scenarios.len(), scenarios_yaml.len(), arch_skills.len(), tech_rules.len(),
        render_contract_facts(&group).len(), tech_rules.len(),
    );

    let client = LlmClient::from_agent_config(
        &AgentLlmConfig { provider: LlmProvider::Ollama, model: "qwen2.5-coder:14b".to_string(), base_url: Some("http://localhost:8080".to_string()) },
        false,
    );

    // ── Path A: production, 3 runs ──────────────────────────────────────────────────────
    println!("########## PATH A: PRODUCTION (story + spec + scenarios + ADRs) ##########\n");
    for run in 1..=3 {
        println!("--- Path A, run {run} ---\n");
        let step = ImplementationStep {
            id: "1".to_string(), service: SERVICE_NAME.to_string(), file: prod_impl_path.clone(),
            operation: "create".to_string(), description: DESCRIPTION.to_string(),
            depends_on: vec![], status: StepStatus::Pending,
        };
        let test_result = generate_unit_test_stub(
            &client, &story, &spec, &openapi_yaml, &step, &prod_test_path,
            &service_packages, &services, &adrs, "", &arch_skills,
        ).unwrap_or_else(|e| panic!("path A run {run}: test generation failed: {e}"));
        std::fs::write(&prod_test_path, &test_result.content).expect("failed to write production test file");
        println!("Generated test:\n{}\n", test_result.content);

        let impl_result = execute_implementation_with_test(
            &client, &story, &spec, &openapi_yaml, &step, None, None,
            &service_packages, &services, "", &arch_skills,
            &prod_test_path, &test_result.content, None, None,
        ).unwrap_or_else(|e| panic!("path A run {run}: implementation generation failed: {e}"));
        std::fs::write(&prod_impl_path, &impl_result.content).expect("failed to write production impl file");
        println!("Generated implementation:\n{}\n", impl_result.content);

        let (passed, output) = run_maven_test(&maven_root);
        println!("Path A run {run} — {}", parse_surefire_summary(&output));
        println!("Path A run {run} result: {}\n", if passed { "PASS" } else { "FAIL" });
        if !passed { println!("Full mvn output:\n{output}\n"); }
    }

    // Remove path A's sub-package entirely before path B starts, so a broken final run can
    // never contaminate path B's compile step by sitting in the same source tree.
    let _ = std::fs::remove_dir_all(&prod_main_dir);
    let _ = std::fs::remove_dir_all(&prod_test_dir);

    // ── Path B: contract-scoped, 3 runs ─────────────────────────────────────────────────
    println!("########## PATH B: CONTRACT-SCOPED (six contracts, no story/spec/scenarios/ADRs) ##########\n");
    for run in 1..=3 {
        println!("--- Path B, run {run} ---\n");
        let t_prompt = contract_test_prompt(&group, &cs_impl_path, &tech_rules);
        let test_code = extract_code(&client.complete_large(&t_prompt).unwrap_or_else(|e| panic!("path B run {run}: test generation failed: {e}")));
        std::fs::write(&cs_test_path, &test_code).expect("failed to write contract-scoped test file");
        println!("Generated test:\n{test_code}\n");

        let i_prompt = contract_impl_prompt(&group, &cs_impl_path, &tech_rules, &test_code);
        let impl_code = extract_code(&client.complete_large(&i_prompt).unwrap_or_else(|e| panic!("path B run {run}: implementation generation failed: {e}")));
        std::fs::write(&cs_impl_path, &impl_code).expect("failed to write contract-scoped impl file");
        println!("Generated implementation:\n{impl_code}\n");

        let (passed, output) = run_maven_test(&maven_root);
        println!("Path B run {run} — {}", parse_surefire_summary(&output));
        println!("Path B run {run} result: {}\n", if passed { "PASS" } else { "FAIL" });
        if !passed { println!("Full mvn output:\n{output}\n"); }
    }
}
