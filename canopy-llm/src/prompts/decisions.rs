//! Stage 2 of the behavior-first planning pipeline (docs/design/behavior-first-planning.md):
//! Decision Extraction and Gating. Elevates unresolved business questions to a first-class,
//! gated artifact instead of a note that later stages can silently work around.
//!
//! Same lesson as Stages 0 and 1, applied again: decision points are constructed mechanically
//! from already-enumerable sources (one per `open_questions` entry), and the LLM is used only
//! for the two genuinely interpretive sub-tasks — linking a blocked behavior to the decision it
//! depends on (or noticing a question that isn't tracked yet), and proposing resolution options
//! for a decision once it exists. Neither call asks the model to "read everything and figure out
//! what decisions exist" — each is a bounded, per-item classification over an already-enumerated
//! list.

use crate::client::{LlmClient, LlmError};
use crate::prompts::yaml_util::{parse_lenient_sequence, strip_code_fence};
use canopy_core::*;

fn linking_prompt(decisions: &[DecisionPoint], blocked: &[BlockedBehaviorCandidate]) -> String {
    let decision_list = decisions.iter().enumerate()
        .map(|(i, d)| format!("{}. id={}, question=\"{}\"", i + 1, d.id, d.question))
        .collect::<Vec<_>>().join("\n");
    let blocked_list = blocked.iter().enumerate()
        .map(|(i, b)| format!("{}. source_ref={}, reason=\"{}\"", i + 1, b.source_ref, b.reason))
        .collect::<Vec<_>>().join("\n");

    format!(
        r#"You are linking blocked implementation candidates to the business decisions they
depend on. You are NOT designing architecture and NOT resolving any decision — only matching
each blocked item to the question it depends on, or noticing a question that isn't tracked yet.

Existing decision points:
{decisions_or_none}

Blocked behavior candidates:
{blocked_list}

Step 1 — Read every blocked candidate above. List the DISTINCT new business questions they raise
that are NOT already covered by an existing decision point. Two candidates blocked by the same
underlying question produce ONE entry here, not two — reuse identical wording for both rather
than rephrasing per candidate. Assign each entry a short ref: q1, q2, q3, ...

Step 2 — For EACH blocked candidate, ONE AT A TIME: does its reason match an existing decision
point above? Record that decision's exact id as `decision_id`. Otherwise record the matching
`new_question_ref` from Step 1. Set exactly one of the two fields per item — never both, never
neither, and do not skip any item.

Return ONLY valid YAML — no prose, no code fences:

new_questions:
  - ref: "q1"
    question: "<phrased question>"
links:
  - source_ref: "<the blocked candidate's source_ref>"
    decision_id: "<matching existing decision id, if any>"
    new_question_ref: "<ref from new_questions, only if no existing id matches>"
"#,
        decisions_or_none = if decision_list.is_empty() { "None yet.".to_string() } else { decision_list },
        blocked_list = blocked_list,
    )
}

fn classification_prompt(decisions: &[DecisionPoint]) -> String {
    let decision_list = decisions.iter().enumerate()
        .map(|(i, d)| format!("{}. id={}, question=\"{}\"", i + 1, d.id, d.question))
        .collect::<Vec<_>>().join("\n");

    format!(
        r#"You are proposing resolution options for business decisions blocking implementation.
You are NOT deciding which option is correct — only enumerating plausible ones for a human to
choose from, and classifying how urgent each decision is.

For EACH decision point below, ONE AT A TIME:
1. Classify it as exactly one of:
   - business — materially changes a validation rule, persistence rule, API contract, event
     contract, or test expectation. Blocks implementation until resolved.
   - technical — a technology/library/infrastructure choice. Usually already an ADR concern.
   - behavioral_ambiguity — a softer wording/ordering/precision call, not truly blocking.
2. Propose 2-4 concrete, mutually exclusive resolution options.
3. Optionally, name components likely affected once resolved — a rough hint only, not
   authoritative (e.g. "WidgetRepository", "WidgetRegistration").

{decision_list}

Return ONLY valid YAML — no prose, no code fences:

classifications:
  - id: "<decision id>"
    category: business | technical | behavioral_ambiguity
    options:
      - "<option>"
    affected_component_hints:
      - "<hint>"
"#,
        decision_list = decision_list,
    )
}

#[derive(Debug, Clone, serde::Deserialize)]
struct RawNewQuestion {
    #[serde(rename = "ref")]
    question_ref: String,
    question: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct RawLink {
    source_ref: String,
    #[serde(default)]
    decision_id: Option<String>,
    #[serde(default)]
    new_question_ref: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct RawClassification {
    id: String,
    category: DecisionCategory,
    #[serde(default)]
    options: Vec<String>,
    #[serde(default)]
    affected_component_hints: Vec<String>,
}

/// Stage 2 entry point. Builds one decision point per `spec.open_questions` entry mechanically,
/// links every blocked behavior candidate to a decision (existing or newly discovered) via one
/// bounded LLM call, classifies + proposes options for every decision via a second bounded LLM
/// call, then runs the three mechanical audits (A: every blocked behavior linked, B: every open
/// question has a decision, C: every decision has at least one dependent behavior).
pub fn extract_decisions(
    client: &LlmClient,
    story: &UserStory,
    spec: &IntentSpec,
    gaps: &BehaviorGaps,
) -> Result<(DecisionLog, DecisionAudit), LlmError> {
    let story_id = story.id.clone();
    let mut counter = 0usize;
    let mut next_id = move || { counter += 1; format!("{story_id}-dec-{:03}", counter) };

    // Step 1 — mechanical: one decision point per open question. Category/options are
    // placeholders until Step 3's classification pass fills them in.
    let mut decisions: Vec<DecisionPoint> = spec.open_questions.iter().map(|q| DecisionPoint {
        id: next_id(),
        question: q.clone(),
        category: DecisionCategory::Business,
        options: Vec::new(),
        status: DecisionStatus::Pending,
        resolution: None,
        affects_behaviors: Vec::new(),
        affects_future_contracts: Vec::new(),
    }).collect();

    // Step 2 — bounded LLM: link every blocked behavior to a decision, existing or newly found.
    // The model dedups new questions itself in one pass (Step 1 of the prompt) and hands Rust
    // refs to look up — no fuzzy string-matching needed on this side to catch reworded dupes.
    if !gaps.blocked.is_empty() {
        let raw = client.complete_large(&linking_prompt(&decisions, &gaps.blocked))?;
        let stripped = strip_code_fence(&raw);
        let new_questions = parse_lenient_sequence::<RawNewQuestion>(&stripped, "new_questions")?;
        let links = parse_lenient_sequence::<RawLink>(&stripped, "links")?;

        let mut ref_to_id = std::collections::HashMap::new();
        for nq in new_questions {
            let id = next_id();
            ref_to_id.insert(nq.question_ref, id.clone());
            decisions.push(DecisionPoint {
                id,
                question: nq.question,
                category: DecisionCategory::Business,
                options: Vec::new(),
                status: DecisionStatus::Pending,
                resolution: None,
                affects_behaviors: Vec::new(),
                affects_future_contracts: Vec::new(),
            });
        }

        for link in links {
            let target_id = link.decision_id
                .or_else(|| link.new_question_ref.as_ref().and_then(|r| ref_to_id.get(r).cloned()));
            let Some(target_id) = target_id else { continue };
            if let Some(d) = decisions.iter_mut().find(|d| d.id == target_id) {
                d.affects_behaviors.push(link.source_ref);
            }
        }
    }

    // Step 3 — bounded LLM: classify + propose options for every decision now known.
    if !decisions.is_empty() {
        let raw = client.complete_large(&classification_prompt(&decisions))?;
        let stripped = strip_code_fence(&raw);
        let classifications = parse_lenient_sequence::<RawClassification>(&stripped, "classifications")?;
        for c in classifications {
            if let Some(d) = decisions.iter_mut().find(|d| d.id == c.id) {
                d.category = c.category;
                d.options = c.options;
                d.affects_future_contracts = c.affected_component_hints;
            }
        }
    }

    // Mechanical audits — computed, not asked of the model.
    let mut findings = Vec::new();
    for b in &gaps.blocked {
        let linked = decisions.iter().any(|d| d.affects_behaviors.contains(&b.source_ref));
        if !linked {
            findings.push(DecisionAuditFinding {
                description: format!("Blocked behavior '{}' is not linked to any decision point.", b.source_ref),
            });
        }
    }
    for q in &spec.open_questions {
        if !decisions.iter().any(|d| &d.question == q) {
            findings.push(DecisionAuditFinding {
                description: format!("Open question '{q}' has no decision point."),
            });
        }
    }
    for d in &decisions {
        if d.affects_behaviors.is_empty() {
            findings.push(DecisionAuditFinding {
                description: format!("Decision '{}' ({}) has no behaviors depending on it — potentially obsolete.", d.id, d.question),
            });
        }
    }

    Ok((DecisionLog { decisions }, DecisionAudit { findings }))
}
