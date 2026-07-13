//! `canopy behaviors <story-id>` — Stages 0-2 of the behavior-first planning pipeline
//! (docs/design/behavior-first-planning.md). Stage 3 (Clustering) is not yet implemented; this
//! command currently stops after Stage 2's gate.

use crate::ui::{confirm_default, select_required};
use crate::util::build_client;
use anyhow::{Context, Result};
use canopy_core::{
    BehaviorKind, BehaviorScope, DecisionCategory, DecisionStatus, GapKind, GapSeverity, StoryStatus,
};
use canopy_llm::{extract_behaviors, extract_decisions, identify_specification_gaps};
use canopy_storage::{
    load_all_adrs, load_story_openapi, load_story_spec, load_user_stories, save_behavior_gaps,
    save_behaviors, save_decision_audit, save_decisions, save_specification_completeness,
};
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

    // Optional context — a story can reach behavior extraction before `canopy spec` has
    // generated an OpenAPI spec; http-request/http-response behaviors are just weaker without it.
    let openapi_yaml = load_story_openapi(story_id).unwrap_or(None).unwrap_or_default();

    println!("\nStage 1 — Behavior Extraction");
    println!("Extracting behaviors for '{}'...", story_id);
    let (behaviors, gaps) = extract_behaviors(&client, story, &spec, &adrs, &openapi_yaml)
        .context("failed to extract behaviors")?;

    println!("\n{} behavior(s) extracted:\n", behaviors.behaviors.len());
    for (i, b) in behaviors.behaviors.iter().enumerate() {
        let scope_label = match b.scope {
            BehaviorScope::Unit => "unit",
            BehaviorScope::Integration => "integration",
        };
        let kind_label = match b.kind {
            BehaviorKind::Validation => "validation",
            BehaviorKind::Construction => "construction",
            BehaviorKind::Persistence => "persistence",
            BehaviorKind::EventShape => "event-shape",
            BehaviorKind::Publication => "publication",
            BehaviorKind::Orchestration => "orchestration",
            BehaviorKind::HttpRequest => "http-request",
            BehaviorKind::HttpResponse => "http-response",
            BehaviorKind::ErrorTranslation => "error-translation",
        };
        println!(
            "{}. [{}] {} ({scope_label}/{kind_label}, subject={}) — {}",
            i + 1, b.id, b.source_ref, b.subject, b.statement
        );
    }

    if !gaps.blocked.is_empty() {
        println!("\n{} behavior(s) blocked on an unresolved decision:\n", gaps.blocked.len());
        for (i, blocked) in gaps.blocked.iter().enumerate() {
            println!("{}. {} — {}", i + 1, blocked.source_ref, blocked.reason);
        }
    }

    save_behaviors(story_id, &behaviors).context("failed to save behaviors.yaml")?;
    save_behavior_gaps(story_id, &gaps).context("failed to save behavior-gaps.yaml")?;
    println!(
        "\nSaved to .canopy/stories/{}/behaviors.yaml, behavior-coverage.yaml, and behavior-gaps.yaml",
        story_id
    );

    if !confirm_default(&theme, "Behavior list looks correct — proceed?", true) {
        println!("Stopped — edit behaviors.yaml directly, or re-run `canopy behaviors {}` to regenerate.", story_id);
        return Ok(());
    }

    println!("\nStage 2 — Decision Extraction and Gating");
    let (mut decisions, audit) = extract_decisions(&client, story, &spec, &gaps)
        .context("failed to extract decisions")?;

    if decisions.decisions.is_empty() {
        println!("No decision points — nothing blocked on an unresolved business question.");
    } else {
        println!("\n{} decision point(s):\n", decisions.decisions.len());
        for d in &mut decisions.decisions {
            let category_label = match d.category {
                DecisionCategory::Business => "business",
                DecisionCategory::Technical => "technical",
                DecisionCategory::BehavioralAmbiguity => "behavioral-ambiguity",
            };
            println!("--- {} ({category_label}) ---", d.id);
            println!("Question : {}", d.question);
            if !d.affects_behaviors.is_empty() {
                println!("Affects  : {}", d.affects_behaviors.join(", "));
            }
            if !d.affects_future_contracts.is_empty() {
                println!("Hints    : {} (non-authoritative — Stage 3/4 don't exist yet)", d.affects_future_contracts.join(", "));
            }

            if d.options.is_empty() {
                println!("(no options proposed — leaving pending)");
                continue;
            }

            let mut items: Vec<String> = d.options.clone();
            items.push("Defer — leave pending (blocks affected behaviors)".to_string());
            let item_refs: Vec<&str> = items.iter().map(String::as_str).collect();
            let choice = select_required(&theme, "Resolution", &item_refs, items.len() - 1, "failed to read decision choice")?;

            if choice == items.len() - 1 {
                // Deferred — status stays Pending.
                continue;
            }
            let chosen = d.options[choice].clone();
            let is_assumption = !confirm_default(
                &theme,
                "Is this a considered decision (not just a temporary assumption to unblock work)?",
                true,
            );
            d.status = if is_assumption { DecisionStatus::AcceptedAssumption } else { DecisionStatus::Resolved };
            d.resolution = Some(chosen);
        }
    }

    if !audit.findings.is_empty() {
        println!("\n{} decision-audit finding(s):\n", audit.findings.len());
        for (i, f) in audit.findings.iter().enumerate() {
            println!("{}. {}", i + 1, f.description);
        }
    }

    save_decisions(story_id, &decisions).context("failed to save decisions.yaml")?;
    save_decision_audit(story_id, &audit).context("failed to save decision-audit.yaml")?;
    println!("\nSaved to .canopy/stories/{}/decisions.yaml and decision-audit.yaml", story_id);

    if decisions.has_pending_decisions() {
        println!(
            "\n{} decision(s) still pending — behaviors depending on them stay blocked. \
             Re-run `canopy behaviors {story_id}` after resolving, or edit decisions.yaml directly.",
            decisions.decisions.iter().filter(|d| d.is_blocking()).count()
        );
        return Ok(());
    }

    println!("\nStage 3 (Clustering) is not yet implemented.");
    Ok(())
}
