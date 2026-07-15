//! Stage 6 (composition) validation step: after fixing the two blockers named in
//! docs/design/contract-composition-assessment.md §8 (JVM event/infrastructure file-target
//! convention, entity-based Construction-dependency matching) and re-running the ADR-fix
//! experiment against the real dogfooding project, this confirms
//! `generate_story_plan_from_contracts` — the real, unmodified, production function `canopy
//! implement` calls — turns the regenerated `contracts.yaml` (now 8 contracts, 2 with a real
//! non-empty dependency) into an actual multi-file, dependency-aware plan.
//!
//! Read-only against the dogfooding project: loads `contracts.yaml`/`services.yaml` from disk,
//! calls the real mechanical planner, and prints the result. Never calls `canopy implement`
//! itself, never writes anything, never risks triggering real execution.
//!
//! ## Run
//!
//! ```sh
//! cargo run -p canopy-llm --example contract_driven_stage6_composition_check -- <project-root> <story-id>
//! ```

use canopy_llm::generate_story_plan_from_contracts;
use std::collections::HashMap;

fn main() {
    let mut args = std::env::args().skip(1);
    let project_root = args.next().expect("usage: <project-root> <story-id>");
    let story_id = args.next().expect("usage: <project-root> <story-id>");

    std::env::set_current_dir(&project_root)
        .unwrap_or_else(|e| panic!("failed to cd into {project_root}: {e}"));

    let contracts = canopy_storage::load_contracts(&story_id).expect("failed to load contracts.yaml");
    let services = canopy_storage::load_services_registry().expect("failed to load services.yaml");
    let service_packages: HashMap<String, String> = HashMap::new();
    let existing_files: Vec<String> = Vec::new();

    println!("=== Stage 6: contract-driven plan for '{story_id}' ({} contracts loaded) ===\n", contracts.contracts.len());

    match generate_story_plan_from_contracts(&story_id, &contracts, &services, &service_packages, &existing_files) {
        Ok(plan) => {
            println!("Plan generated mechanically — {} step(s):\n", plan.steps.len());
            for step in &plan.steps {
                println!("--- step {} ({}) ---", step.id, step.operation);
                println!("  service: {}", step.service);
                println!("  file:    {}", step.file);
                println!("  desc:    {}", step.description);
                if step.depends_on.is_empty() {
                    println!("  depends_on: (none)");
                } else {
                    println!("  depends_on:");
                    for d in &step.depends_on {
                        println!("    - {d}");
                    }
                }
                println!();
            }
        }
        Err(e) => {
            println!("generate_story_plan_from_contracts returned Err — falls back to legacy planner in production:");
            println!("  {e}");
        }
    }
}
