use crate::ui::{input_text_required, input_text_with_initial, select_required};
use crate::util::build_client;
use anyhow::{Context, Result};
use canopy_core::{Role, StoryStatus, UserStory};
use canopy_llm::{extract_domain_from_stories, generate_stories_from_intent};
use canopy_storage::{
    load_domain_registry, load_idea, load_roles_registry, load_user_stories, save_domain_registry,
    save_roles_registry, save_user_stories,
};
use dialoguer::theme::ColorfulTheme;

pub(crate) fn cmd_intent(statement: Option<String>, debug: bool) -> Result<()> {
    let theme = ColorfulTheme::default();

    let statement = match statement {
        Some(s) => s,
        None => input_text_required(&theme, "Behavioral intent", "failed to read intent statement")?,
    };

    let context = load_idea()
        .map(|i| i.description)
        .unwrap_or_else(|_| String::from("No context available."));

    let mut existing = load_user_stories().context("failed to load stories")?;
    let roles = load_roles_registry().context("failed to load roles")?;

    let client = build_client("intent", debug)?;
    println!("\nDeriving stories from intent...");
    let new_stories = generate_stories_from_intent(
        &client, &statement, &context, &existing, &roles,
    ).context("failed to generate stories from intent")?;

    // Gate each new story interactively before saving.
    let existing_ids: std::collections::HashSet<String> =
        existing.stories.iter().map(|s| s.id.clone()).collect();

    let fresh: Vec<_> = new_stories.stories.into_iter()
        .filter(|s| !existing_ids.contains(&s.id))
        .collect();

    if fresh.is_empty() {
        println!("No new stories generated.");
        return Ok(());
    }

    println!("\n{} new story/stories to review:\n", fresh.len());

    let mut accepted_count = 0;
    let mut rejected_count = 0;
    let mut curated: Vec<UserStory> = Vec::new();

    for (i, mut story) in fresh.into_iter().enumerate() {
        println!("--- Story {} ---", i + 1);
        println!("As a   : {}", story.as_a);
        println!("I want : {}", story.want);
        println!("So that: {}", story.so_that);

        let choice = select_required(&theme, "Accept this story?", &["Accept", "Accept with edit", "Reject"], 0, "failed to read story choice")?;

        match choice {
            0 => {
                story.status = StoryStatus::Accepted;
                accepted_count += 1;
                println!("  Accepted.");
            }
            1 => {
                let want = input_text_with_initial(&theme, "I want", &story.want, "failed to read edited want")?;
                let so_that = input_text_with_initial(&theme, "So that", &story.so_that, "failed to read edited so_that")?;
                story.want = want;
                story.so_that = so_that;
                story.status = StoryStatus::Accepted;
                accepted_count += 1;
                println!("  Accepted with edits.");
            }
            _ => {
                story.status = StoryStatus::Rejected;
                rejected_count += 1;
                println!("  Rejected.");
            }
        }

        curated.push(story);
    }

    for story in &curated {
        existing.stories.push(story.clone());
    }
    save_user_stories(&existing).context("failed to save stories.yaml")?;

    // Update roles registry from accepted stories only.
    let mut roles = load_roles_registry().context("failed to load roles")?;
    for story in curated.iter().filter(|s| s.status == StoryStatus::Accepted) {
        let role = story.as_a.trim().to_string();
        if !roles.roles.iter().any(|r| r.name().eq_ignore_ascii_case(&role)) {
            roles.roles.push(Role::Simple(role));
        }
    }
    save_roles_registry(&roles).context("failed to save roles.yaml")?;

    // Extract domain vocabulary from accepted stories only.
    let accepted_stories: Vec<_> = curated.iter()
        .filter(|s| s.status == StoryStatus::Accepted)
        .cloned()
        .collect();
    if !accepted_stories.is_empty() {
        print!("Extracting domain vocabulary...");
        match extract_domain_from_stories(&client, &accepted_stories) {
            Ok(extracted) => {
                let mut domain = load_domain_registry().context("failed to load domain registry")?;
                let mut added_entities = 0usize;
                let mut added_events = 0usize;
                for e in &extracted.entities {
                    if !domain.entities.iter().any(|x| x.name().eq_ignore_ascii_case(e.name())) {
                        domain.entities.push(e.clone());
                        added_entities += 1;
                    }
                }
                for e in &extracted.events {
                    if !domain.events.iter().any(|x| x.name().eq_ignore_ascii_case(e.name())) {
                        domain.events.push(e.clone());
                        added_events += 1;
                    }
                }
                save_domain_registry(&domain).context("failed to save domain_registry.yaml")?;
                println!(" +{added_entities} entities, +{added_events} events → .canopy/domain_registry.yaml");
            }
            Err(e) => println!(" (skipped: {e})"),
        }
    }

    println!("\n{accepted_count} accepted, {rejected_count} rejected. Run `canopy stories` to view backlog.");
    Ok(())
}
