use crate::adr_wizard::maybe_prompt_testing_strategy;
use crate::ui::{input_text_required, input_text_with_initial, select_required};
use crate::util::build_client;
use anyhow::{Context, Result};
use canopy_core::{Adr, DomainRegistry, IntentSpec, StoryStatus};
use canopy_llm::{generate_story_openapi, generate_story_spec, identify_architectural_questions, parse_event_adr};
use canopy_storage::{
    load_all_adrs, load_domain_registry, load_services_registry, load_user_stories,
    save_adr, save_services_registry, save_story_openapi, save_story_spec,
};
use dialoguer::theme::ColorfulTheme;

/// Mechanical guard, not an LLM judgment: does spec generation's own `entity_schema` name an
/// entity already established upstream (domain vocabulary, extracted from the story/intent)?
/// Live-verified need: a reproducibility sweep found a spec-generation call that produced
/// `entity_schema.entity: Account` for a story whose `as_a` and domain vocabulary both already
/// said "Manufacturer" — nothing caught this, and it silently saved as if accepted, corrupting
/// every stage built on top of it. Fails loudly instead of letting a fully different domain
/// persist unnoticed. Only checks against already-known vocabulary — a story introducing its
/// first-ever entity (domain.entities empty) has nothing to diverge from yet.
fn check_entity_continuity(spec: &IntentSpec, domain: &DomainRegistry) -> Result<()> {
    let Some(schema) = &spec.entity_schema else { return Ok(()) };
    if domain.entities.is_empty() {
        return Ok(());
    }
    let matches = domain.entities.iter().any(|e| e.name().eq_ignore_ascii_case(&schema.entity));
    if !matches {
        let known: Vec<&str> = domain.entities.iter().map(|e| e.name()).collect();
        anyhow::bail!(
            "entity continuity violation: spec generation produced entity_schema.entity = '{}', \
             which doesn't match any entity already established in domain vocabulary ({}). This \
             usually means spec generation drifted onto an unrelated domain — check the story's \
             `want` field and .canopy/domain_registry.yaml, then re-run `canopy spec {}`. Nothing \
             was saved.",
            schema.entity, known.join(", "), spec.intent_ref,
        );
    }
    Ok(())
}

/// Mechanical guard, not an LLM judgment: does a newly-accepted domain-event ADR's event name
/// share the same entity prefix as an entity already established in domain vocabulary? Same
/// placement and severity as `check_entity_continuity`, one stage earlier — live-verified need:
/// a reproducibility run's domain-event ADR named "ManufacturerRegistered" while that same run's
/// domain vocabulary had already extracted the event as "ManufacturerCreated" — same entity,
/// different verb, which is accepted, ambiguous wording per the domain-extraction rules
/// themselves (never gated on here). What this catches is the ENTITY an event's name starts with
/// silently diverging from established vocabulary — e.g. an ADR naming "AccountRegistered" when
/// domain vocabulary only knows "Manufacturer" — the same class of derailment Entity Continuity
/// guards against for `entity_schema`. Only checks ADRs accepted during THIS call, not the whole
/// project's accumulated history, and only once domain vocabulary is established.
fn check_event_continuity(story_id: &str, newly_accepted_adrs: &[Adr], domain: &DomainRegistry) -> Result<()> {
    if domain.entities.is_empty() {
        return Ok(());
    }
    for adr in newly_accepted_adrs {
        let Some((event_name, _topic)) = parse_event_adr(&adr.decision) else { continue };
        let event_lower = event_name.to_lowercase();
        let matches_known_entity = domain.entities.iter()
            .any(|e| event_lower.starts_with(&e.name().to_lowercase()));
        if !matches_known_entity {
            let known: Vec<&str> = domain.entities.iter().map(|e| e.name()).collect();
            anyhow::bail!(
                "event continuity violation: domain-event ADR '{}' names event '{}', which \
                 doesn't start with any entity already established in domain vocabulary ({}). \
                 This usually means the ADR drifted onto an unrelated entity — review the ADR, \
                 then re-run `canopy spec {}`. Nothing further was saved.",
                adr.title, event_name, known.join(", "), story_id,
            );
        }
    }
    Ok(())
}

pub(crate) fn cmd_spec(story_id: &str, debug: bool) -> Result<()> {
    let theme = ColorfulTheme::default();

    let stories = load_user_stories().context("failed to load stories.yaml")?;
    let story = stories
        .stories
        .iter()
        .find(|s| s.id == story_id)
        .ok_or_else(|| anyhow::anyhow!("story '{}' not found", story_id))?;

    if story.status != StoryStatus::Accepted {
        anyhow::bail!(
            "story '{}' has status '{:?}' — only accepted stories can be specified",
            story_id,
            story.status
        );
    }

    println!("\nStory: As a {}, I want {}, so that {}", story.as_a, story.want, story.so_that);

    let mut existing_adrs = load_all_adrs().context("failed to load ADRs")?;
    let mut services = load_services_registry().context("failed to load services registry")?;
    let domain = load_domain_registry().context("failed to load domain registry")?;

    let client = build_client("architect", debug)?;

    println!("\nIdentifying architectural questions...");
    let mut proposed = identify_architectural_questions(&client, story, &existing_adrs, &services)
        .context("failed to identify architectural questions")?;

    // Marks where THIS call's own newly-accepted ADRs start — `existing_adrs` began as the
    // whole project's accumulated history, so checking event continuity against all of it would
    // false-positive on unrelated ADRs from other stories/entities. Only this run's own ADRs are
    // relevant to check against this story's own domain vocabulary.
    let adrs_before_this_run = existing_adrs.len();

    if proposed.proposals.is_empty() {
        println!("No architectural questions identified — proceeding to spec generation.");
    } else {
        println!("\n{} architectural question(s) to address:\n", proposed.proposals.len());

        for i in 0..proposed.proposals.len() {
            let proposal = proposed.proposals[i].clone();
            println!("--- Question {} of {} ---", i + 1, proposed.proposals.len());
            println!("Question : {}", proposal.question);
            println!("Proposed : {}", proposal.title);
            println!("Decision : {}", proposal.decision);
            println!("Reason   : {}", proposal.reason);
            if !proposal.alternatives.is_empty() {
                println!("Alternatives: {}", proposal.alternatives.join(", "));
            }
            if let Some(ref svc) = proposal.service {
                if !svc.is_empty() {
                    println!("Service  : {}", svc);
                    if let Some(ref tech) = proposal.technology {
                        if !tech.is_empty() {
                            let ct = proposal.component_type.as_deref().unwrap_or("service");
                            println!("  Technology: {} ({})", tech, ct);
                        }
                    }
                    if !proposal.service_responsibilities.is_empty() {
                        println!("  Responsibilities: {}", proposal.service_responsibilities.join(", "));
                    }
                }
            }

            let choice = select_required(&theme, "Accept this ADR?", &["Accept", "Modify decision text", "Reject"], 0, "failed to read ADR choice")?;

            match choice {
                0 => {
                    // Accept
                    let adr = Adr {
                        title: proposal.title.clone(),
                        decision: proposal.decision.clone(),
                        reason: proposal.reason.clone(),
                        alternatives: proposal.alternatives.clone(),
                    };
                    let index = existing_adrs.len() + 1;
                    let slug = canopy_storage::intent_slug(&proposal.title);
                    save_adr(index, &slug, &adr).context("failed to save ADR")?;
                    println!("  Saved: adr-{:03}-{}.yaml", index, slug);
                    existing_adrs.push(adr);
                    services.apply_adr_proposal(&proposal);
                    maybe_prompt_testing_strategy(
                        &theme, &mut existing_adrs,
                        proposal.technology.as_deref().unwrap_or(""),
                        proposal.service.as_deref().unwrap_or("service"),
                    )?;
                }
                1 => {
                    // Modify
                    let modified_decision = input_text_with_initial(&theme, "Enter revised decision text", &proposal.decision, "failed to read modified decision")?;

                    let mut modified_proposal = proposal.clone();
                    modified_proposal.decision = modified_decision;

                    // If this proposal names a service, let the user rename it so subsequent
                    // proposals (e.g. the database ADR) reference the correct name.
                    if let Some(ref old_name) = proposal.service {
                        if !old_name.is_empty() {
                            let new_name = input_text_with_initial(&theme, "Service name (leave unchanged to keep current)", old_name, "failed to read modified service name")?;
                            let new_name = new_name.trim().to_string();
                            if !new_name.is_empty() && &new_name != old_name {
                                // Propagate the rename to all remaining proposals in this batch.
                                for later in proposed.proposals[i + 1..].iter_mut() {
                                    if later.service.as_deref() == Some(old_name) {
                                        later.service = Some(new_name.clone());
                                    }
                                }
                                modified_proposal.service = Some(new_name);
                            }
                        }
                    }

                    let adr = Adr {
                        title: modified_proposal.title.clone(),
                        decision: modified_proposal.decision.clone(),
                        reason: modified_proposal.reason.clone(),
                        alternatives: modified_proposal.alternatives.clone(),
                    };
                    let index = existing_adrs.len() + 1;
                    let slug = canopy_storage::intent_slug(&modified_proposal.title);
                    save_adr(index, &slug, &adr).context("failed to save ADR")?;
                    println!("  Saved: adr-{:03}-{}.yaml", index, slug);
                    existing_adrs.push(adr);
                    services.apply_adr_proposal(&modified_proposal);
                    maybe_prompt_testing_strategy(
                        &theme, &mut existing_adrs,
                        modified_proposal.technology.as_deref().unwrap_or(""),
                        modified_proposal.service.as_deref().unwrap_or("service"),
                    )?;
                }
                _ => {
                    println!("  Rejected — skipping.");
                }
            }
        }

        // Catch any service or frontend that ended up without a decided technology —
        // can happen when the LLM omits a tech stack proposal or the user renames a component.
        let missing_tech: Vec<String> = services.services.iter()
            .filter(|s| {
                let ct = s.component_type.as_deref().unwrap_or("service");
                ct != "infrastructure" && s.technology.is_none()
            })
            .map(|s| s.name.clone())
            .collect();

        for name in missing_tech {
            println!("\n  '{}' has no decided technology.", name);
            let tech = input_text_required(&theme, &format!("Technology for '{}'", name), "failed to read technology")?;
            let tech = tech.trim().to_string();
            if !tech.is_empty() {
                let ct = services.services.iter()
                    .find(|s| s.name == name)
                    .and_then(|s| s.component_type.clone())
                    .unwrap_or_else(|| "service".to_string());
                if let Some(entry) = services.services.iter_mut().find(|s| s.name == name) {
                    entry.technology = Some(tech.clone());
                }
                let adr = Adr {
                    title: format!("Tech stack for {}", name),
                    decision: tech.clone(),
                    reason: format!("Technology for {} decided during spec — no proposal was generated.", name),
                    alternatives: vec![],
                };
                let index = existing_adrs.len() + 1;
                let slug = canopy_storage::intent_slug(&adr.title);
                save_adr(index, &slug, &adr).context("failed to save tech stack ADR")?;
                println!("  Saved: adr-{:03}-{}.yaml", index, slug);
                existing_adrs.push(adr);
                // Ensure component_type is set correctly for scaffold
                if let Some(entry) = services.services.iter_mut().find(|s| s.name == name) {
                    if entry.component_type.is_none() {
                        entry.component_type = Some(ct);
                    }
                }
            }
        }

        save_services_registry(&services).context("failed to save services registry")?;
    }

    check_event_continuity(story_id, &existing_adrs[adrs_before_this_run..], &domain)?;

    println!("\nGenerating BDD spec for story '{}'...", story_id);
    let spec =
        generate_story_spec(&client, story, &existing_adrs, &services, &domain)
            .context("failed to generate story spec")?;

    check_entity_continuity(&spec, &domain)?;

    save_story_spec(story_id, &spec).context("failed to save story spec")?;
    println!("\nSpec saved to .canopy/stories/{}/spec.yaml", story_id);

    println!("\nGenerating OAS 3.1.0 spec...");
    match generate_story_openapi(&client, story, &spec, &services, &existing_adrs) {
        Ok(openapi_yaml) => {
            save_story_openapi(story_id, &openapi_yaml).context("failed to save OpenAPI spec")?;
            println!("OpenAPI spec saved to .canopy/stories/{}/openapi.yaml", story_id);
        }
        Err(e) => {
            eprintln!("Warning: OpenAPI spec generation failed: {e}");
        }
    }


    if let Some(ref schema) = spec.entity_schema {
        println!("\nEntity Schema: {}", schema.entity);
        if !schema.system_generated.is_empty() {
            println!("  System-generated:");
            for f in &schema.system_generated {
                println!("    {} ({})  {}", f.name, f.field_type, f.description);
            }
        }
        if !schema.mandatory.is_empty() {
            println!("  Mandatory:");
            for f in &schema.mandatory {
                println!("    {} ({})  {}", f.name, f.field_type, f.description);
            }
        }
        if !schema.optional.is_empty() {
            println!("  Optional:");
            for f in &schema.optional {
                println!("    {} ({})  {}", f.name, f.field_type, f.description);
            }
        }
    }

    println!("\nScenarios:");
    for s in &spec.scenarios {
        println!("  [{}] {}", s.id, s.name);
        for g in &s.given {
            println!("    Given {}", g);
        }
        println!("    When  {}", s.when);
        for t in &s.then {
            println!("    Then  {}", t);
        }
        if !s.constraints.is_empty() {
            println!("    Constraints: {}", s.constraints.join("; "));
        }
    }
    if !spec.out_of_scope.is_empty() {
        println!("\nOut of scope: {}", spec.out_of_scope.join(", "));
    }
    if !spec.open_questions.is_empty() {
        println!("Open questions: {}", spec.open_questions.join("; "));
    }

    Ok(())
}

#[cfg(test)]
mod entity_continuity_tests {
    use super::check_entity_continuity;
    use canopy_core::{DomainEntity, DomainRegistry, EntitySchema, IntentSpec};

    fn spec_with_entity(entity: &str) -> IntentSpec {
        IntentSpec {
            intent_ref: "manufacturer-001".to_string(),
            entity_schema: Some(EntitySchema {
                entity: entity.to_string(),
                system_generated: vec![],
                mandatory: vec![],
                optional: vec![],
            }),
            scenarios: vec![],
            resolved_policies: vec![],
            out_of_scope: vec![],
            open_questions: vec![],
        }
    }

    fn domain_with(entities: &[&str]) -> DomainRegistry {
        DomainRegistry {
            entities: entities.iter().map(|e| DomainEntity::Simple(e.to_string())).collect(),
            events: vec![],
        }
    }

    #[test]
    fn passes_when_entity_matches_known_vocabulary() {
        let spec = spec_with_entity("Manufacturer");
        let domain = domain_with(&["Manufacturer"]);
        assert!(check_entity_continuity(&spec, &domain).is_ok());
    }

    #[test]
    fn passes_case_insensitively() {
        let spec = spec_with_entity("manufacturer");
        let domain = domain_with(&["Manufacturer"]);
        assert!(check_entity_continuity(&spec, &domain).is_ok());
    }

    #[test]
    fn fails_on_live_verified_account_divergence() {
        // Live-verified regression: a reproducibility sweep produced entity_schema.entity =
        // "Account" for a story whose domain vocabulary already established "Manufacturer".
        let spec = spec_with_entity("Account");
        let domain = domain_with(&["Manufacturer"]);
        let err = check_entity_continuity(&spec, &domain).unwrap_err();
        assert!(err.to_string().contains("entity continuity violation"));
        assert!(err.to_string().contains("Account"));
        assert!(err.to_string().contains("Manufacturer"));
    }

    #[test]
    fn passes_when_no_entity_schema_present() {
        let spec = IntentSpec {
            intent_ref: "manufacturer-001".to_string(),
            entity_schema: None,
            scenarios: vec![],
            resolved_policies: vec![],
            out_of_scope: vec![],
            open_questions: vec![],
        };
        let domain = domain_with(&["Manufacturer"]);
        assert!(check_entity_continuity(&spec, &domain).is_ok());
    }

    #[test]
    fn passes_when_domain_vocabulary_not_yet_established() {
        // A story introducing its first-ever entity has nothing to diverge from yet.
        let spec = spec_with_entity("Manufacturer");
        let domain = domain_with(&[]);
        assert!(check_entity_continuity(&spec, &domain).is_ok());
    }
}

#[cfg(test)]
mod event_continuity_tests {
    use super::check_event_continuity;
    use canopy_core::{Adr, DomainEntity, DomainRegistry};

    fn event_adr(title: &str, event_and_topic: &str) -> Adr {
        Adr {
            title: title.to_string(),
            decision: event_and_topic.to_string(),
            reason: "test fixture".to_string(),
            alternatives: vec![],
        }
    }

    fn domain_with(entities: &[&str]) -> DomainRegistry {
        DomainRegistry {
            entities: entities.iter().map(|e| DomainEntity::Simple(e.to_string())).collect(),
            events: vec![],
        }
    }

    #[test]
    fn passes_when_event_entity_prefix_matches_known_vocabulary() {
        let adrs = vec![event_adr("Domain Event ADR", "ManufacturerRegistered on topic manufacturer-events")];
        let domain = domain_with(&["Manufacturer"]);
        assert!(check_event_continuity("manufacturer-001", &adrs, &domain).is_ok());
    }

    #[test]
    fn passes_on_created_vs_registered_wording_variance() {
        // Same entity, different verb — accepted, ambiguous wording per the domain-extraction
        // rules themselves; this check gates on entity prefix only, not the exact verb.
        let adrs = vec![event_adr("Domain Event ADR", "ManufacturerCreated on topic manufacturer-events")];
        let domain = domain_with(&["Manufacturer"]);
        assert!(check_event_continuity("manufacturer-001", &adrs, &domain).is_ok());
    }

    #[test]
    fn fails_on_live_verified_entity_prefix_divergence() {
        // Live-verified: an ADR named an event whose entity prefix didn't match any entity
        // established in domain vocabulary for that same story.
        let adrs = vec![event_adr("Account Registration Domain Event", "AccountRegistered on topic account-events")];
        let domain = domain_with(&["Manufacturer"]);
        let err = check_event_continuity("manufacturer-001", &adrs, &domain).unwrap_err();
        assert!(err.to_string().contains("event continuity violation"));
        assert!(err.to_string().contains("AccountRegistered"));
        assert!(err.to_string().contains("Manufacturer"));
    }

    #[test]
    fn ignores_non_event_adrs() {
        let adrs = vec![event_adr("Tech Stack for Manufacturer Service", "Spring Boot")];
        let domain = domain_with(&["Manufacturer"]);
        assert!(check_event_continuity("manufacturer-001", &adrs, &domain).is_ok());
    }

    #[test]
    fn passes_when_domain_vocabulary_not_yet_established() {
        let adrs = vec![event_adr("Domain Event ADR", "AccountRegistered on topic account-events")];
        let domain = domain_with(&[]);
        assert!(check_event_continuity("manufacturer-001", &adrs, &domain).is_ok());
    }
}
