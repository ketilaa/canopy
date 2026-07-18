use crate::review_log::record_review;
use crate::ui::{
    input_text_required, input_text_with_initial, select_required, select_review_choice,
    ReviewChoice,
};
use crate::util::build_client;
use anyhow::{Context, Result};
use canopy_core::{DomainRegistry, Role, StoryStatus, UserStory};
use canopy_llm::{extract_domain_from_stories, generate_stories_from_intent};
use canopy_storage::{
    load_domain_registry, load_idea, load_roles_registry, load_user_stories, save_domain_registry,
    save_roles_registry, save_user_stories,
};
use dialoguer::theme::ColorfulTheme;

/// Generic function words only — deliberately not tuned to any specific story's vocabulary,
/// so the false-positive rate this produces is honest evidence, not a cherry-picked result.
const STOPWORDS: &[&str] = &[
    "that", "this", "them", "they", "their", "with", "from", "into", "onto", "will", "would",
    "should", "must", "shall", "want", "wants", "need", "needs", "have", "has", "had", "been",
    "being", "were", "was", "are", "then", "than", "when", "where", "which", "what", "some",
    "such", "only", "also", "more", "most", "each", "every", "both", "either", "neither", "just",
    "very", "much", "many", "other", "another", "same", "able",
];

fn known_in_domain(word: &str, domain: &DomainRegistry) -> bool {
    let singular = word.strip_suffix('s').unwrap_or(word);
    domain
        .entities
        .iter()
        .map(|e| e.name().to_lowercase())
        .chain(domain.events.iter().map(|e| e.name().to_lowercase()))
        .any(|n| {
            let n_singular = n.strip_suffix('s').unwrap_or(&n).to_string();
            n == word || n_singular == singular
        })
}

/// Scans a story's `so_that` clause — where both real vocabulary-discrepancy instances this
/// project has ever found were located, not `want` or `as_a` — for content words absent from the
/// known domain vocabulary. Purely mechanical, no LLM call: a deterministic text/registry
/// comparison, same tier as `classify_proposal_category`.
pub(crate) fn find_vocabulary_discrepancies(so_that: &str, domain: &DomainRegistry) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut candidates = Vec::new();
    for raw in so_that.split(|c: char| !c.is_alphanumeric()) {
        if raw.len() < 4 {
            continue;
        }
        let lower = raw.to_lowercase();
        if STOPWORDS.contains(&lower.as_str()) {
            continue;
        }
        if known_in_domain(&lower, domain) {
            continue;
        }
        let singular = lower.strip_suffix('s').unwrap_or(&lower).to_string();
        if seen.insert(singular) {
            candidates.push(raw.to_string());
        }
    }
    candidates
}

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

        let choice = select_review_choice(&theme, "Accept this story?", "Accept with edit", "failed to read story choice")?;

        let outcome = match choice {
            ReviewChoice::Accept => {
                story.status = StoryStatus::Accepted;
                accepted_count += 1;
                println!("  Accepted.");
                "accept"
            }
            ReviewChoice::Edit => {
                let want = input_text_with_initial(&theme, "I want", &story.want, "failed to read edited want")?;
                let so_that = input_text_with_initial(&theme, "So that", &story.so_that, "failed to read edited so_that")?;
                story.want = want;
                story.so_that = so_that;
                story.status = StoryStatus::Accepted;
                accepted_count += 1;
                println!("  Accepted with edits.");
                "accept-with-edit"
            }
            ReviewChoice::Reject => {
                story.status = StoryStatus::Rejected;
                rejected_count += 1;
                println!("  Rejected.");
                "reject"
            }
        };
        record_review("intent", Some(&story.id), "user-story", &story.want, outcome);

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
        match extract_domain_from_stories(&client, &statement, &accepted_stories) {
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

                for story in &accepted_stories {
                    for term in find_vocabulary_discrepancies(&story.so_that, &domain) {
                        println!(
                            "\nThis story references '{term}', which isn't represented anywhere in the known domain."
                        );
                        let choice = select_required(
                            &theme,
                            "Is that gap meaningful?",
                            &["Yes", "No", "Not sure"],
                            0,
                            "failed to read vocabulary discrepancy response",
                        )?;
                        let outcome = match choice {
                            0 => "meaningful",
                            1 => "not-meaningful",
                            _ => "not-sure",
                        };
                        record_review("intent", Some(&story.id), "vocabulary-discrepancy", &term, outcome);
                    }
                }
            }
            Err(e) => println!(" (skipped: {e})"),
        }
    }

    println!("\n{accepted_count} accepted, {rejected_count} rejected. Run `canopy stories` to view backlog.");
    Ok(())
}

#[cfg(test)]
mod find_vocabulary_discrepancies_tests {
    use super::find_vocabulary_discrepancies;
    use canopy_core::{DomainEntity, DomainRegistry};

    fn domain_with(entities: &[&str]) -> DomainRegistry {
        DomainRegistry {
            entities: entities.iter().map(|n| DomainEntity::Simple(n.to_string())).collect(),
            events: Vec::new(),
        }
    }

    #[test]
    fn flags_the_real_manufacturer_001_case() {
        let domain = domain_with(&["Manufacturer"]);
        let found = find_vocabulary_discrepancies("products can reference them in the system", &domain);
        assert!(found.iter().any(|t| t.eq_ignore_ascii_case("products")), "{found:?}");
    }

    #[test]
    fn does_not_flag_a_term_already_in_the_domain_registry() {
        let domain = domain_with(&["Product"]);
        let found = find_vocabulary_discrepancies("products can reference them", &domain);
        assert!(!found.iter().any(|t| t.eq_ignore_ascii_case("products")), "{found:?}");
    }

    #[test]
    fn does_not_flag_stopwords_or_short_words() {
        let domain = domain_with(&[]);
        let found = find_vocabulary_discrepancies("so that they can be with it", &domain);
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn deduplicates_repeated_terms() {
        let domain = domain_with(&[]);
        let found = find_vocabulary_discrepancies("orders reference other orders", &domain);
        assert_eq!(found.iter().filter(|t| t.eq_ignore_ascii_case("orders")).count(), 1, "{found:?}");
    }

    #[test]
    fn matches_case_insensitively_and_across_simple_plurals() {
        let domain = domain_with(&["order"]);
        let found = find_vocabulary_discrepancies("Orders must be tracked", &domain);
        assert!(!found.iter().any(|t| t.eq_ignore_ascii_case("orders")), "{found:?}");
    }
}
