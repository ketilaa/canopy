//! Human-Insight Process Experiment, Phase 2 (`docs/design/human-insight-process-experiment-
//! design.md`): does each Product Owner persona's distinguishing business instinct, injected the
//! same validated way the Role Meaning Value Experiment injected a role fact, produce divergent
//! output from the real, unmodified `generate_story_spec` for `order-001` ("Customers must be
//! able to request a return for a product they previously purchased")?
//!
//! **Why fact-injection, not live ADR Accept/Modify/Reject**: confirmed directly in
//! `canopy-cli/src/commands/spec.rs` — the business-policy checklist (`resolved_policies`/
//! `open_questions`) is produced entirely inside `generate_story_spec` with no interactive human
//! review step at all; only the ADR proposals from `identify_architectural_questions` have a real
//! Accept/Modify/Reject gate, and Product Owners have no legitimate standing over the
//! architecture-flavored ones (service naming, tech stack) per this project's own established
//! norm. The one real lever a persona has over policy-shaped output is exactly the channel the
//! Value Experiment already validated: an ADR-shaped fact present in `existing_adrs`, the same way
//! a real accepted ADR already is.
//!
//! **Standalone experiment, same discipline as the Value Experiment.** Calls
//! `generate_story_spec` directly, unmodified, five times against the same frozen input — the
//! real `order-001` story and domain registry, empty `services` — varying only which persona's
//! one-fact ADR is present.
//!
//! **Strictly read-only.** Only `stories.yaml` and `domain_registry.yaml` are read from disk;
//! every result is printed and discarded, never saved.
//!
//! ## Run
//!
//! ```sh
//! cargo run -p canopy-llm --example return_request_persona_experiment -- <project-root> <story-id>
//! ```

use canopy_core::{Adr, ServicesRegistry};
use canopy_llm::{generate_story_spec, LlmClient};

fn persona_fact(persona: &str) -> Adr {
    let (decision, reason): (&str, &str) = match persona {
        "risk_averse" => (
            "A return request must include the original order/purchase confirmation number, verified against our records, before it can be accepted",
            "Unverified requests are rejected outright to prevent fraudulent return claims — loss prevention takes priority over convenience.",
        ),
        "customer_experience" => (
            "A return request is accepted based on the customer's own account order history, with no additional proof required from the customer",
            "Minimizing friction for a returning customer is more valuable than strict verification for a typical, low-value return.",
        ),
        "operational" => (
            "Return eligibility is determined automatically by matching the order date against a fixed return window, with no manual verification step",
            "Verification should never require a human review step — the lowest-overhead, fully automatic resolution is preferred.",
        ),
        "compliance" => (
            "Per consumer protection requirements, a return request must be honored within a legally mandated minimum window regardless of the item's condition, using the original payment method for any refund",
            "Verification exists only to confirm the purchase occurred, not to create a barrier to a legally guaranteed right — driven by external obligation, not internal preference.",
        ),
        "growth_retention" => (
            "A return request should trigger an offer to exchange for a replacement or store credit before a refund is processed, and the stated reason for the return must always be captured",
            "Verification should stay lightweight so it doesn't discourage a returning customer from remaining engaged — the relationship matters more than the single transaction.",
        ),
        other => panic!("unknown persona: {other}"),
    };
    Adr {
        title: "Return Eligibility and Verification Policy".to_string(),
        decision: decision.to_string(),
        reason: reason.to_string(),
        alternatives: vec![],
    }
}

const PERSONAS: [&str; 5] = [
    "risk_averse",
    "customer_experience",
    "operational",
    "compliance",
    "growth_retention",
];

fn main() {
    let mut args = std::env::args().skip(1);
    let project_root = args
        .next()
        .unwrap_or_else(|| "/Users/ketil/code/ketilaa/canopy-e-commerce".to_string());
    let story_id = args.next().unwrap_or_else(|| "order-001".to_string());

    std::env::set_current_dir(&project_root)
        .unwrap_or_else(|e| panic!("failed to cd into {project_root}: {e}"));

    let stories = canopy_storage::load_user_stories().expect("failed to load stories.yaml");
    let story = stories
        .stories
        .iter()
        .find(|s| s.id == story_id)
        .unwrap_or_else(|| panic!("story '{story_id}' not found in stories.yaml"))
        .clone();
    let domain =
        canopy_storage::load_domain_registry().expect("failed to load domain_registry.yaml");
    let services = ServicesRegistry { services: vec![] };

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
        "=== Return Request Persona Experiment: '{story_id}' ===\n\
         Frozen inputs: story='{}' (real), domain_registry (real, {} entities / {} events), \
         services=[] (reconstructed pre-spec state)\n",
        story.id,
        domain.entities.len(),
        domain.events.len(),
    );

    for persona in PERSONAS {
        let existing_adrs: Vec<Adr> = vec![persona_fact(persona)];

        println!("\n=================================================================");
        println!("=== Persona: {persona} ===");
        println!("=================================================================\n");

        match generate_story_spec(&client, &story, &existing_adrs, &services, &domain) {
            Ok(spec) => {
                if let Some(schema) = &spec.entity_schema {
                    println!("entity_schema.entity: {}", schema.entity);
                    println!(
                        "  mandatory: {:?}",
                        schema.mandatory.iter().map(|f| &f.name).collect::<Vec<_>>()
                    );
                    println!(
                        "  optional:  {:?}",
                        schema.optional.iter().map(|f| &f.name).collect::<Vec<_>>()
                    );
                } else {
                    println!("entity_schema: <none>");
                }

                println!("\nresolved_policies ({}):", spec.resolved_policies.len());
                for p in &spec.resolved_policies {
                    println!("  [{}]", p.area);
                    println!("    resolution: {}", p.resolution);
                    println!("    evidence:   {}", p.evidence);
                }

                println!("\nopen_questions ({}):", spec.open_questions.len());
                for q in &spec.open_questions {
                    println!("  - {q}");
                }

                println!("\nscenarios ({}):", spec.scenarios.len());
                for s in &spec.scenarios {
                    println!("  [{}] {}", s.id, s.name);
                    for g in &s.given {
                        println!("      given: {g}");
                    }
                    println!("      when:  {}", s.when);
                    for t in &s.then {
                        println!("      then:  {t}");
                    }
                }

                println!("\nout_of_scope: {:?}", spec.out_of_scope);
            }
            Err(e) => {
                println!("generate_story_spec returned Err: {e}");
            }
        }
    }
}
