//! Human Insight Process Experiment, Phase 3 materialization step
//! (`docs/design/human-insight-process-experiment-phase2-results.md`): re-runs one selected
//! persona's fact-injected `generate_story_spec` + `generate_story_openapi` call for real, with
//! FULL entity-schema detail (type, validation), and SAVES the result into the current project's
//! `.canopy/` via the real, unmodified `save_story_spec`/`save_story_openapi` — no hand-authored
//! YAML, no risk of a transcription mismatch with what the model actually produced.
//!
//! **Disclosed methodological note**: Phase 2's own logging only printed entity-schema field
//! *names*, not full type/validation detail, so this is not guaranteed to be byte-identical to
//! what Phase 2 reported for the same persona — it is a fresh sample under the same injected
//! fact, not a replay. This is stated explicitly in the Phase 3 results, not glossed over.
//!
//! **Not read-only, by design** — unlike Phase 2's standalone comparison, this step's entire job
//! is to materialize real project state for `canopy behaviors`/`scaffold`/`implement` to consume,
//! so it saves for real into whatever directory is the current working directory.
//!
//! ## Run
//!
//! ```sh
//! cd <persona-branch-project-root>
//! cargo run -p canopy-llm --example return_request_materialize_persona -- <story-id> <persona>
//! ```

use canopy_core::Adr;
use canopy_llm::{generate_story_openapi, generate_story_spec, LlmClient};

fn persona_fact(persona: &str) -> Adr {
    let (decision, reason): (&str, &str) = match persona {
        "customer_experience" => (
            "A return request is accepted based on the customer's own account order history, with no additional proof required from the customer",
            "Minimizing friction for a returning customer is more valuable than strict verification for a typical, low-value return.",
        ),
        "compliance" => (
            "Per consumer protection requirements, a return request must be honored within a legally mandated minimum window regardless of the item's condition, using the original payment method for any refund",
            "Verification exists only to confirm the purchase occurred, not to create a barrier to a legally guaranteed right — driven by external obligation, not internal preference.",
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

fn main() {
    let mut args = std::env::args().skip(1);
    let story_id = args.next().unwrap_or_else(|| "order-001".to_string());
    let persona = args
        .next()
        .unwrap_or_else(|| panic!("usage: return_request_materialize_persona <story-id> <persona>"));

    let stories = canopy_storage::load_user_stories().expect("failed to load stories.yaml");
    let story = stories
        .stories
        .iter()
        .find(|s| s.id == story_id)
        .unwrap_or_else(|| panic!("story '{story_id}' not found in stories.yaml"))
        .clone();
    let domain =
        canopy_storage::load_domain_registry().expect("failed to load domain_registry.yaml");
    // Real, already-decided architecture from the shared spec run — read, not modified.
    let existing_adrs = canopy_storage::load_all_adrs().expect("failed to load ADRs");
    let services =
        canopy_storage::load_services_registry().expect("failed to load services.yaml");

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

    // The persona's fact is the ONLY additional input beyond the real, shared architecture —
    // appended, not replacing, the already-decided ADRs.
    let mut adrs_with_fact = existing_adrs.clone();
    adrs_with_fact.push(persona_fact(&persona));

    println!("=== Materializing persona '{persona}' for '{story_id}' ===\n");

    let spec = generate_story_spec(&client, &story, &adrs_with_fact, &services, &domain)
        .unwrap_or_else(|e| panic!("generate_story_spec failed: {e}"));

    if let Some(schema) = &spec.entity_schema {
        println!("entity_schema.entity: {}", schema.entity);
        println!("  system_generated: {:#?}", schema.system_generated);
        println!("  mandatory: {:#?}", schema.mandatory);
        println!("  optional:  {:#?}", schema.optional);
    } else {
        println!("entity_schema: <none> — will retry once, since implementation needs a real schema");
    }
    println!("\nresolved_policies ({}):", spec.resolved_policies.len());
    for p in &spec.resolved_policies {
        println!("  [{}] {} (evidence: {})", p.area, p.resolution, p.evidence);
    }
    println!("\nopen_questions ({}):", spec.open_questions.len());
    for q in &spec.open_questions {
        println!("  - {q}");
    }
    println!("\nscenarios: {}", spec.scenarios.len());
    println!("out_of_scope: {:?}", spec.out_of_scope);

    canopy_storage::save_story_spec(&story_id, &spec).expect("failed to save spec.yaml");
    println!("\nSaved .canopy/stories/{story_id}/spec.yaml");

    match generate_story_openapi(&client, &story, &spec, &services, &adrs_with_fact) {
        Ok(openapi_yaml) => {
            canopy_storage::save_story_openapi(&story_id, &openapi_yaml)
                .expect("failed to save openapi.yaml");
            println!("Saved .canopy/stories/{story_id}/openapi.yaml");
        }
        Err(e) => println!("Warning: generate_story_openapi failed: {e}"),
    }
}
