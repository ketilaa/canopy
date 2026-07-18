use anyhow::{Context, Result};
use canopy_core::{DomainRegistry, StoryStatus, UserStories};
use canopy_storage::{load_domain_registry, load_user_stories};

/// Splits a PascalCase entity name into lowercase words — "ProductVariant" -> "product variant" —
/// so a compound entity name can be matched as a phrase against free-form story fields. Only
/// splits at a lowercase-to-uppercase boundary, so an all-caps acronym like "SKU" stays "sku"
/// rather than becoming "s k u" (live-verified bug: the naive "space before every uppercase char"
/// version broke exactly this case, silently flagging an already-covered entity as uncaptured).
fn spaced_lowercase(name: &str) -> String {
    let mut out = String::new();
    let mut prev_lower = false;
    for ch in name.chars() {
        if ch.is_uppercase() && prev_lower {
            out.push(' ');
        }
        prev_lower = ch.is_lowercase();
        out.extend(ch.to_lowercase());
    }
    out
}

fn strip_plural(s: &str) -> &str {
    s.strip_suffix('s').unwrap_or(s)
}

/// A story "covers" an entity if the entity's name (as a phrase) appears in that story's `as_a`
/// or `want` field — the two fields Canopy's own story-generation prompt requires to explicitly
/// name the actor or the acted-upon domain object. `so_that` is deliberately excluded: it's free-
/// form purpose text where an entity can be mentioned in passing with no real story behind it yet
/// (live-verified case: `product-008`'s `so_that` names "customers" with no `Customer` story
/// anywhere in the project — counting `so_that` would have hidden exactly the gap this check
/// exists to find).
fn story_covers_entity(entity_phrase: &str, story: &canopy_core::UserStory) -> bool {
    let singular = strip_plural(entity_phrase);
    for field in [&story.as_a, &story.want] {
        let field_lower = field.to_lowercase();
        if field_lower.contains(entity_phrase) || field_lower.contains(singular) {
            return true;
        }
    }
    false
}

/// The primary Backlog Evolution signal: entities Canopy already knows about (from
/// `domain_registry.yaml`) that have never been the subject of any accepted story. Purely
/// structural — no LLM call, no scanning for arbitrary new words, just a comparison between two
/// artifacts Canopy already maintains.
pub(crate) fn entities_without_stories(domain: &DomainRegistry, stories: &UserStories) -> Vec<String> {
    let accepted: Vec<_> = stories.stories.iter()
        .filter(|s| s.status == StoryStatus::Accepted)
        .collect();

    domain.entities.iter()
        .filter(|e| {
            let phrase = spaced_lowercase(e.name());
            !accepted.iter().any(|s| story_covers_entity(&phrase, s))
        })
        .map(|e| e.name().to_string())
        .collect()
}

pub(crate) fn cmd_domain_show() -> Result<()> {
    let domain = load_domain_registry().context("failed to load domain registry")?;

    if domain.entities.is_empty() && domain.events.is_empty() {
        println!("No domain vocabulary yet.");
        println!("Run `canopy intent` to start building stories — entities and events are extracted automatically.");
        return Ok(());
    }

    println!("Entities ({}):", domain.entities.len());
    for e in &domain.entities {
        match e.description() {
            Some(d) => println!("  {} — {}", e.name(), d),
            None    => println!("  {}", e.name()),
        }
    }

    println!("\nEvents ({}):", domain.events.len());
    for e in &domain.events {
        match e.description() {
            Some(d) => println!("  {} — {}", e.name(), d),
            None    => println!("  {}", e.name()),
        }
    }

    let stories = load_user_stories().context("failed to load stories.yaml")?;
    let uncovered = entities_without_stories(&domain, &stories);
    if !uncovered.is_empty() {
        println!("\nEntities with no story yet ({}):", uncovered.len());
        for name in &uncovered {
            println!("  {name}");
        }
        println!("(known to the project, but no accepted story creates, reads, updates, or acts as this entity yet)");
    }

    println!("\nEdit .canopy/domain_registry.yaml to add, rename, or remove entries.");
    Ok(())
}

#[cfg(test)]
mod entities_without_stories_tests {
    use super::entities_without_stories;
    use canopy_core::{DomainEntity, DomainRegistry, StoryStatus, UserStories, UserStory};

    fn domain_with(names: &[&str]) -> DomainRegistry {
        DomainRegistry {
            entities: names.iter().map(|n| DomainEntity::Simple(n.to_string())).collect(),
            events: Vec::new(),
        }
    }

    fn story(id: &str, as_a: &str, want: &str, so_that: &str, status: StoryStatus) -> UserStory {
        UserStory {
            id: id.to_string(),
            as_a: as_a.to_string(),
            want: want.to_string(),
            so_that: so_that.to_string(),
            depends_on: Vec::new(),
            status,
        }
    }

    #[test]
    fn flags_the_real_uncaptured_entities() {
        let domain = domain_with(&["Manufacturer", "Product", "ProductVariant", "Order", "Customer"]);
        let stories = UserStories {
            stories: vec![
                story("s1", "product manager", "register a manufacturer", "so_that unused", StoryStatus::Accepted),
                story("s2", "product manager", "register a product", "so_that unused", StoryStatus::Accepted),
                story("s3", "product manager", "register a product variant for a product", "so_that unused", StoryStatus::Accepted),
            ],
        };
        let mut uncovered = entities_without_stories(&domain, &stories);
        uncovered.sort();
        assert_eq!(uncovered, vec!["Customer".to_string(), "Order".to_string()]);
    }

    #[test]
    fn so_that_mention_alone_does_not_count_as_coverage() {
        // Live-verified trap: product-008's so_that names "customers" with no real Customer story.
        let domain = domain_with(&["Customer"]);
        let stories = UserStories {
            stories: vec![story(
                "product-008",
                "product manager",
                "publish a product variant to the catalog",
                "customers can view and order the unique product version",
                StoryStatus::Accepted,
            )],
        };
        assert_eq!(entities_without_stories(&domain, &stories), vec!["Customer".to_string()]);
    }

    #[test]
    fn matches_compound_entity_name_as_a_phrase_in_want() {
        let domain = domain_with(&["ProductVariant"]);
        let stories = UserStories {
            stories: vec![story(
                "s1", "product manager", "register a product variant for a product", "so_that unused", StoryStatus::Accepted,
            )],
        };
        assert!(entities_without_stories(&domain, &stories).is_empty());
    }

    #[test]
    fn draft_and_rejected_stories_do_not_count_as_coverage() {
        let domain = domain_with(&["Manufacturer"]);
        let stories = UserStories {
            stories: vec![
                story("s1", "product manager", "register a manufacturer", "x", StoryStatus::Draft),
                story("s2", "product manager", "register a manufacturer", "x", StoryStatus::Rejected),
            ],
        };
        assert_eq!(entities_without_stories(&domain, &stories), vec!["Manufacturer".to_string()]);
    }

    #[test]
    fn matches_case_insensitively_and_across_simple_plurals() {
        let domain = domain_with(&["Order"]);
        let stories = UserStories {
            stories: vec![story("s1", "product manager", "cancel Orders", "x", StoryStatus::Accepted)],
        };
        assert!(entities_without_stories(&domain, &stories).is_empty());
    }

    #[test]
    fn matches_all_caps_acronym_entity_names() {
        // Live-verified bug: naive space-before-every-uppercase-char logic turned "SKU" into
        // "s k u", which never matches "sku" in real story text — flagging an already-covered
        // entity as uncaptured.
        let domain = domain_with(&["SKU"]);
        let stories = UserStories {
            stories: vec![story(
                "s1", "product manager", "assign an SKU to a product variant", "x", StoryStatus::Accepted,
            )],
        };
        assert!(entities_without_stories(&domain, &stories).is_empty());
    }
}
