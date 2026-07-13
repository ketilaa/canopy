//! `canopy behaviors <story-id>` — Stage 0 (Specification Completeness) of the behavior-first
//! planning pipeline (docs/design/behavior-first-planning.md). Stage 1 (behavior extraction) is
//! not yet implemented; this command currently stops after the completeness gate.

use crate::ui::confirm_default;
use crate::util::build_client;
use anyhow::{Context, Result};
use canopy_core::{GapKind, GapSeverity, StoryStatus};
use canopy_llm::identify_specification_gaps;
use canopy_storage::{load_all_adrs, load_story_spec, load_user_stories, save_specification_completeness};
use dialoguer::theme::ColorfulTheme;

pub(crate) fn cmd_behaviors(story_id: &str, debug: bool) -> Result<()> {
    let theme = ColorfulTheme::default();

    let stories = load_user_stories().context("failed to load stories.yaml")?;
    let story = stories
        .stories
        .iter()
        .find(|s| s.id == story_id)
        .ok_or_else(|| anyhow::anyhow!("story '{}' not found", story_id))?;

    if story.status != StoryStatus::Accepted {
        anyhow::bail!(
            "story '{}' has status '{:?}' — only accepted stories can proceed to behavior planning",
            story_id,
            story.status
        );
    }

    let spec = load_story_spec(story_id)
        .with_context(|| format!("no spec for '{}' — run `canopy spec {story_id}` first", story_id))?;
    let adrs = load_all_adrs().unwrap_or_default();

    let client = build_client("architect", debug)?;

    println!("\nStage 0 — Specification Completeness");
    println!("Checking '{}' for gaps...", story_id);
    let completeness = identify_specification_gaps(&client, story, &spec, &adrs)
        .context("failed to check specification completeness")?;

    if completeness.gaps.is_empty() {
        println!("No gaps found — specification is complete.");
    } else {
        println!("\n{} gap(s) found:\n", completeness.gaps.len());
        for (i, gap) in completeness.gaps.iter().enumerate() {
            let kind_label = match gap.kind {
                GapKind::MissingScenario => "missing scenario",
                GapKind::AmbiguousOutcome => "ambiguous outcome",
                GapKind::UnresolvedQuestion => "unresolved question",
            };
            let marker = match gap.severity() {
                GapSeverity::Gap => "GAP",
                GapSeverity::Review => "review",
            };
            println!("{}. [{kind_label}] ({marker}) {}", i + 1, gap.description);
        }
    }

    save_specification_completeness(story_id, &completeness)
        .context("failed to save completeness.yaml")?;
    println!("\nSaved to .canopy/stories/{}/completeness.yaml", story_id);

    if completeness.has_blocking_gaps() {
        println!(
            "\nBlocking gaps must be resolved (update spec.yaml, or re-run `canopy spec {story_id}`) \
             before behavior extraction can proceed."
        );
        return Ok(());
    }

    if !confirm_default(&theme, "Specification is complete enough to proceed — continue to behavior extraction?", true) {
        println!("Stopped — re-run `canopy behaviors {}` when ready.", story_id);
        return Ok(());
    }

    println!("\nStage 1 (behavior extraction) is not yet implemented.");
    Ok(())
}
