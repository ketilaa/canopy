//! Pre-Behavior Planning Reproducibility Sweep
//! (`docs/design/pre-behavior-planning-reproducibility-sweep.md`): does
//! `identify_architectural_questions` — the one LLM call performing service discovery, service-
//! ownership assignment, technology recommendation, and infrastructure recommendation, all at
//! once — produce reproducible output across repeated calls against the exact same frozen input?
//!
//! **Standalone experiment, same discipline as Stage 5/6.** Calls production's real, unmodified
//! `identify_architectural_questions` (`canopy-llm/src/prompts/spec.rs:258`) directly, N=5 times,
//! with frozen inputs loaded/constructed once and never mutated. No modification to `canopy-cli`'s
//! `spec.rs` or any other production call site.
//!
//! **Why not just run `canopy spec` five times against the real CLI:** `canopy spec` persists
//! `services.yaml`/`decisions/adr-*.yaml` after every run, and the prompt itself tells the model to
//! skip proposing a service that's already in Known Services — a second real run would see the
//! first run's own output as context and mostly collapse to "skip," not measure reproducibility.
//!
//! **Frozen inputs (see design doc §1 "Which frozen inputs to use"):**
//! - `story`: the real `manufacturer-001` `UserStory`, loaded via `load_user_stories`.
//! - `domain`: the real, current `domain_registry.yaml`, loaded via `load_domain_registry` — this
//!   file is populated by `init`/`intent`, not by `spec`, so today's content already reflects the
//!   pre-spec state.
//! - `existing_adrs`: an EMPTY `Vec<Adr>` — reconstructing the real state immediately before
//!   `manufacturer-001`'s first (and, per the Contract Readiness Assessment, only) `spec` run,
//!   which is the source of all 8 ADRs present in the project today.
//! - `services`: an EMPTY `ServicesRegistry { services: vec![] }` — same reasoning; today's
//!   `services.yaml` already has both services fully decided, which would mostly reproduce the
//!   "already decided, skip" branch instead of exercising discovery/recommendation.
//!
//! **Strictly read-only against the dogfooding project.** Only `stories.yaml` and
//! `domain_registry.yaml` are read from disk; every one of the 5 `ProposedAdrs` results is printed
//! and discarded, never written anywhere. No `services.yaml`/`decisions/` load, no
//! `with_log_path` (which would write into the dogfooding project's `.canopy/logs/`), no save call
//! of any kind.
//!
//! ## Run
//!
//! ```sh
//! cargo run -p canopy-llm --example pre_behavior_reproducibility_sweep -- <project-root> <story-id>
//! ```

use canopy_core::{Adr, ServicesRegistry};
use canopy_llm::{identify_architectural_questions, LlmClient};

const RUNS: usize = 5;

fn main() {
    let mut args = std::env::args().skip(1);
    let project_root = args
        .next()
        .unwrap_or_else(|| "/Users/ketil/code/ketilaa/canopy-e-commerce".to_string());
    let story_id = args
        .next()
        .unwrap_or_else(|| "manufacturer-001".to_string());

    std::env::set_current_dir(&project_root)
        .unwrap_or_else(|e| panic!("failed to cd into {project_root}: {e}"));

    // ── Load frozen inputs, once, read-only ─────────────────────────────────────────────
    let stories = canopy_storage::load_user_stories().expect("failed to load stories.yaml");
    let story = stories
        .stories
        .iter()
        .find(|s| s.id == story_id)
        .unwrap_or_else(|| panic!("story '{story_id}' not found in stories.yaml"))
        .clone();
    let domain =
        canopy_storage::load_domain_registry().expect("failed to load domain_registry.yaml");

    // Reconstructed pre-spec state, NOT loaded from today's already-populated files.
    let existing_adrs: Vec<Adr> = Vec::new();
    let services = ServicesRegistry { services: vec![] };

    // ── Build the "architect" agent's LlmClient exactly as `canopy spec` does ───────────
    // (mirrors canopy-cli/src/util.rs's `build_client`, which is `pub(crate)` and so cannot be
    // called directly from this standalone example) — but WITHOUT `.with_log_path(...)`, since
    // that would write a log file into the dogfooding project's `.canopy/logs/` directory and
    // this sweep must stay strictly read-only against it.
    let debug = false;
    let client = match canopy_storage::load_config().expect("failed to read .canopy/config.yaml") {
        Some(cfg) => {
            let agent_cfg = cfg.for_agent("architect").unwrap_or_else(|| {
                panic!("no LLM config for agent 'architect' and no default in .canopy/config.yaml")
            });
            LlmClient::from_agent_config(&agent_cfg, debug)
        }
        None => LlmClient::default_local(debug),
    };

    println!(
        "=== Pre-Behavior Planning Reproducibility Sweep: '{story_id}', {RUNS} runs ===\n\
         Frozen inputs: story='{}' (real), domain_registry (real, {} entities / {} events), \
         existing_adrs=[] (reconstructed pre-spec state), services=[] (reconstructed pre-spec state)\n",
        story.id,
        domain.entities.len(),
        domain.events.len(),
    );

    let mut all_runs: Vec<Vec<canopy_core::ProposedAdr>> = Vec::new();

    for run in 1..=RUNS {
        println!("--- Run {run} ---");
        match identify_architectural_questions(&client, &story, &existing_adrs, &services, &domain)
        {
            Ok(proposed) => {
                println!("proposals.len() = {}\n", proposed.proposals.len());
                for (i, p) in proposed.proposals.iter().enumerate() {
                    println!("  [{}] question:                 {}", i + 1, p.question);
                    println!("      title:                    {}", p.title);
                    println!("      decision:                 {}", p.decision);
                    println!("      reason:                   {}", p.reason);
                    println!("      alternatives:             {:?}", p.alternatives);
                    println!("      service:                  {:?}", p.service);
                    println!(
                        "      service_responsibilities: {:?}",
                        p.service_responsibilities
                    );
                    println!("      technology:               {:?}", p.technology);
                    println!("      component_type:           {:?}", p.component_type);
                    println!();
                }
                all_runs.push(proposed.proposals);
            }
            Err(e) => {
                println!("  identify_architectural_questions returned Err: {e}\n");
                all_runs.push(Vec::new());
            }
        }
    }

    // ── Raw, objective per-run summary (no equivalence/variance judgment here) ──────────
    println!("=== Summary (objective counts only — classification is a separate, human step) ===");
    for (i, proposals) in all_runs.iter().enumerate() {
        let services_named: Vec<&str> = proposals
            .iter()
            .filter_map(|p| p.service.as_deref())
            .collect();
        let backend_tech: Vec<&str> = proposals
            .iter()
            .filter(|p| p.component_type.as_deref() == Some("service"))
            .filter_map(|p| p.technology.as_deref())
            .collect();
        let frontend_tech: Vec<&str> = proposals
            .iter()
            .filter(|p| p.component_type.as_deref() == Some("frontend"))
            .filter_map(|p| p.technology.as_deref())
            .collect();
        println!(
            "Run {}: {} proposal(s); services={:?}; backend_tech={:?}; frontend_tech={:?}",
            i + 1,
            proposals.len(),
            services_named,
            backend_tech,
            frontend_tech,
        );
    }
}
