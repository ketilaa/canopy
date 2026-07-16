//! Role Meaning Value Validation (`docs/design/role-meaning-value-validation.md`): does explicit
//! role meaning, made available to production's real, unmodified `generate_story_spec` the same
//! way an existing ADR already is, change the entity schema, business-policy checklist, or
//! scenarios it produces?
//!
//! **Standalone experiment, same discipline as Stage 5/6 and the reproducibility sweep.** Calls
//! `generate_story_spec` (`canopy-llm/src/prompts/spec.rs`) directly, unmodified, four times
//! against the same frozen pre-spec input the reproducibility sweep already established as this
//! story's real starting condition — the only difference between the four calls is one additional
//! synthetic ADR representing a role-meaning fact, present in three of the four and absent in the
//! baseline. No new prompt is authored; the role fact is injected through the same
//! `existing_adrs: &[Adr]` channel `entity_schema_prompt` already reads and renders as "Existing
//! Architecture Decisions" today.
//!
//! **Conditions**: `none` (today's real baseline — empty ADRs, matching the reproducibility
//! sweep), `internal`, `external`, `affiliated` — the four-way answer set
//! `docs/design/role-classification-stability-test.md` settled on, superseding the original
//! three-way set.
//!
//! **Strictly read-only against the dogfooding project.** Same as the reproducibility sweep: only
//! `stories.yaml` and `domain_registry.yaml` are read from disk; every `IntentSpec` result is
//! printed and discarded, never written anywhere, no log path, no save call.
//!
//! ## Run
//!
//! ```sh
//! cargo run -p canopy-llm --example role_meaning_value_experiment -- <project-root> <story-id>
//! ```

use canopy_core::{Adr, ServicesRegistry};
use canopy_llm::{generate_story_spec, LlmClient};

fn role_fact_adr(condition: &str) -> Option<Adr> {
    let (decision, reason, alternatives): (&str, &str, Vec<&str>) = match condition {
        "internal" => (
            "Internal — an employee or operator of our own business, acting on its behalf",
            "The role registers data as part of this business's own operations, not on behalf of an outside party.",
            vec!["External — a representative of an outside party", "Affiliated — a recognized but organizationally separate party"],
        ),
        "external" => (
            "External — a representative of the manufacturer itself, not an employee of our business",
            "The role acts on behalf of the manufacturer being registered, an outside party to our business.",
            vec!["Internal — an employee or operator of our own business", "Affiliated — a recognized but organizationally separate party"],
        ),
        "affiliated" => (
            "Affiliated — a recognized, ongoing but organizationally separate party (e.g. a long-term contracted agent), neither an employee nor an arms-length outsider",
            "The role has an established, privileged relationship with our business without being directly employed by it.",
            vec!["Internal — an employee or operator of our own business", "External — an arms-length outside party"],
        ),
        _ => return None,
    };
    Some(Adr {
        title: "Role Definition: Manufacturer Representative".to_string(),
        decision: decision.to_string(),
        reason: reason.to_string(),
        alternatives: alternatives.into_iter().map(String::from).collect(),
    })
}

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
        "=== Role Meaning Value Experiment: '{story_id}' ===\n\
         Frozen inputs: story='{}' (real), domain_registry (real, {} entities / {} events), \
         services=[] (reconstructed pre-spec state)\n",
        story.id,
        domain.entities.len(),
        domain.events.len(),
    );

    for condition in ["none", "internal", "external", "affiliated"] {
        let existing_adrs: Vec<Adr> = role_fact_adr(condition).into_iter().collect();

        println!("\n=================================================================");
        println!("=== Condition: {condition} (existing_adrs.len() = {}) ===", existing_adrs.len());
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
