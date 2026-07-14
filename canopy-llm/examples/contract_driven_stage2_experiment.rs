//! Stage 2 of docs/design/contract-driven-implementation-experiment.md: does showing a model
//! *every* contract that targets a shared file — not just one — stop the unauthorized
//! `@Entity`/`@Id`/`@GeneratedValue` invention Stage 1 found in 2 of 3 runs?
//!
//! **This is an experimental probe, not production wiring.** Same guarantee as Stage 1's example:
//! no import from or call into `canopy-cli`'s `implement` command, `plan.rs`, `execute.rs`, or
//! `step.rs`. Standalone binary, not part of the `canopy` CLI or the `canopy-llm` library's
//! public surface. Changes no existing behavior.
//!
//! ## What's reused vs. what's experimental
//!
//! Reused, unmodified: `canopy_storage::load_contracts`, `canopy_llm::resolve_implementation_target`
//! / `abstract_layer_for_kind`, `canopy_llm::skill_for_technology` — identical to Stage 1's
//! example, and to what `canopy behaviors`/a future contract-driven `canopy implement` would
//! actually call.
//!
//! Experimental, novel to this file: grouping contracts by their resolved target (Stage 1 always
//! had exactly one contract per run; this groups every contract sharing one file and generates
//! from all of them in a single call — the one variable this experiment changes relative to
//! Stage 1, per the design doc's explicit isolation requirement).
//!
//! ## Inputs allowed vs. disallowed — unchanged from Stage 1
//!
//! Allowed: every contract sharing the resolved target (`kind`/`entity`/`member`/`mandatory`/
//! `required_tests`/`dependencies`), the resolved target itself, the tech-stack skill for that
//! layer. Disallowed, and absent from this file (grep for `load_story_spec`, `load_all_adrs`,
//! `load_story_openapi`, `load_user_stories`, `ToolSpec` — none appear): story, full scenario
//! list, `entity_schema`, ADRs, OpenAPI, exploratory tool access.
//!
//! ## Run
//!
//! ```sh
//! cargo run -p canopy-llm --example contract_driven_stage2_experiment -- <project-root> <story-id>
//! ```

use canopy_core::{AgentLlmConfig, BehaviorKind, Contract, LlmProvider};
use canopy_llm::{abstract_layer_for_kind, resolve_implementation_target, skill_for_technology, LlmClient};

/// Mechanically resolves the file target for one contract — mirrors exactly what a future
/// contract-driven generation step would do per contract, before grouping. `None` for a contract
/// with no `kind`/`entity` (e.g. an integration contract) — such contracts don't target a single
/// implementation file and are excluded from grouping entirely.
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
            kind = c.kind,
            entity = c.entity,
            member = c.member,
            mandatory = c.mandatory,
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
        target_path = target_path,
        facts = render_contract_facts(contracts),
        tech_rules = tech_rules,
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
        target_path = target_path,
        facts = render_contract_facts(contracts),
        tech_rules = tech_rules,
        test_code = test_code,
    )
}

fn main() {
    let mut args = std::env::args().skip(1);
    let project_root = args.next().unwrap_or_else(|| ".".to_string());
    let story_id = args.next().unwrap_or_else(|| "manufacturer-001".to_string());

    std::env::set_current_dir(&project_root)
        .unwrap_or_else(|e| panic!("cannot cd into '{project_root}': {e}"));

    let contract_set = canopy_storage::load_contracts(&story_id)
        .unwrap_or_else(|e| panic!("failed to load contracts.yaml for '{story_id}': {e}"));

    let tech = "Spring Boot";
    let pkg = "manufacturer_service";
    let service_name = "manufacturer-service";

    // Group every contract by its resolved target — the one thing this experiment changes
    // relative to Stage 1 (which always had exactly one contract in this group).
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
    println!("\n{} contracts, {} required_tests total.\n", group.len(), group.iter().map(|c| c.required_tests.len()).sum::<usize>());

    let layer = abstract_layer_for_kind(group[0].kind.as_ref().unwrap());
    let tech_rules = skill_for_technology(tech, pkg, pkg, service_name, layer);

    let client = LlmClient::from_agent_config(
        &AgentLlmConfig {
            provider: LlmProvider::Ollama,
            model: "qwen2.5-coder:14b".to_string(),
            base_url: Some("http://localhost:8080".to_string()),
        },
        false,
    );

    println!("=== Generated test ===\n");
    let t_prompt = test_prompt(&group, &name_contract_target, &tech_rules);
    let test_code = client.complete_large(&t_prompt).unwrap_or_else(|e| panic!("test generation failed: {e}"));
    println!("{test_code}\n");

    println!("=== Generated implementation ===\n");
    let i_prompt = impl_prompt(&group, &name_contract_target, &tech_rules, &test_code);
    let impl_code = client.complete_large(&i_prompt).unwrap_or_else(|e| panic!("implementation generation failed: {e}"));
    println!("{impl_code}\n");
}
