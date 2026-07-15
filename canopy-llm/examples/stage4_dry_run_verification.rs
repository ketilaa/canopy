//! Stage 4 sanity check: run `generate_story_plan_from_contracts` against a real, already-
//! generated `contracts.yaml` and `services.yaml` (not synthetic test fixtures) and print the
//! resulting plan. Read-only — no scaffolding, no LLM calls, no file writes. This is the same
//! mechanical function `canopy implement` now calls automatically when a story's contracts.yaml
//! exists; running it standalone here avoids needing the target project scaffolded first (a
//! separate, larger, and more invasive action `canopy implement`'s own `ensure_services_scaffolded`
//! gate would otherwise require before the plan-generation code is even reached).
//!
//! ```sh
//! cargo run -p canopy-llm --example stage4_dry_run_verification -- <project-root> <story-id>
//! ```

fn main() {
    let mut args = std::env::args().skip(1);
    let project_root = args.next().unwrap_or_else(|| ".".to_string());
    let story_id = args.next().unwrap_or_else(|| "manufacturer-001".to_string());

    std::env::set_current_dir(&project_root)
        .unwrap_or_else(|e| panic!("cannot cd into '{project_root}': {e}"));

    let contracts = canopy_storage::load_contracts(&story_id)
        .unwrap_or_else(|e| panic!("failed to load contracts.yaml for '{story_id}': {e}"));
    let services = canopy_storage::load_services_registry()
        .unwrap_or_else(|e| panic!("failed to load services.yaml: {e}"));

    match canopy_llm::generate_story_plan_from_contracts(
        &story_id, &contracts, &services, &std::collections::HashMap::new(), &[],
    ) {
        Ok(plan) => {
            println!("=== Contract-driven plan for '{story_id}' ({} step(s)) ===\n", plan.steps.len());
            for step in &plan.steps {
                let op = if step.operation == "modify" { "modify" } else { "create" };
                println!("[{}] {op} {} ({})", step.id, step.file, step.service);
                println!("      {}", step.description);
                if !step.depends_on.is_empty() {
                    println!("      depends_on: {:?}", step.depends_on);
                }
            }
        }
        Err(reason) => println!("Contract-driven enumeration declined: {reason}"),
    }
}
