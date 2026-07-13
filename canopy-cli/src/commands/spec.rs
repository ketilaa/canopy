use crate::adr_wizard::maybe_prompt_testing_strategy;
use crate::ui::{input_text_required, input_text_with_initial, select_required};
use crate::util::build_client;
use anyhow::{Context, Result};
use canopy_core::{Adr, StoryStatus};
use canopy_llm::{generate_story_openapi, generate_story_spec, identify_architectural_questions};
use canopy_storage::{
    load_all_adrs, load_domain_registry, load_services_registry, load_user_stories,
    save_adr, save_services_registry, save_story_openapi, save_story_spec,
};
use dialoguer::theme::ColorfulTheme;

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

    println!("\nGenerating BDD spec for story '{}'...", story_id);
    let spec =
        generate_story_spec(&client, story, &existing_adrs, &services, &domain)
            .context("failed to generate story spec")?;

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
