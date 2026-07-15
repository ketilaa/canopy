---
title: Should Domain-Boundary Decisions Be Made Explicit Somewhere in Pre-Behavior Planning?
status: active
origin: hypothesis proposed against Human-Insight Inventory / reproducibility sweep evidence
date_discovered: 2026-07-15
related_principles: [structure-emerges-from-behavior, unresolved-decisions-become-explicit-decision-points]
related_narratives: []
related_reports: [manufacturer-001]
related_design_docs: [domain-boundary-hypothesis-assessment.md, human-insight-inventory.md, pre-behavior-planning-reproducibility-sweep.md, contract-composition-assessment.md]
---

# Question

Are some architectural recommendations currently produced during pre-behavior planning — service
ownership, domain events, integration boundaries, authorization boundaries, data ownership —
actually standing in for a domain-boundary decision (which entities share a bounded context, which
business capability owns which data) that the pipeline never makes explicit anywhere?

# Why It Matters

If true, some of the instability the Pre-Behavior Planning Reproducibility Sweep measured (service/
frontend naming split between entity-scoped and process-scoped conventions; possibly the
domain-event category too) isn't pure model-sampling noise — it's the visible symptom of an
upstream judgment call the pipeline asks the model to make silently, story by story, with no place
for a human to weigh in on the underlying grouping. That would connect this question directly to
`unresolved-decisions-become-explicit-decision-points`, which already describes this exact failure
shape (silent interpretation of a judgment call with no supporting basis) for a different category
of decision.

# Evidence So Far

See `docs/design/domain-boundary-hypothesis-assessment.md` for the full evaluation. Summary:

- **Supports it**: the sweep's own entity-scoped-vs-process-scoped naming split; `spec`'s
  architectural-questions prompt only ever asks a per-story *existence* check ("is there already a
  service for this"), never a *relationship* question ("should this share a boundary with that");
  the already-documented order-dependent-prompt-content variability source; and direct historical
  precedent — "boundaries" were an explicit upfront elicitation category at `explore`/`init`,
  deliberately removed (`f0e8593`) for being asked too abstractly, too early, and never picked back
  up downstream since.
- **Limits it sharply**: the real dogfooding project's `domain_registry.yaml` has ever contained
  exactly one entity (`Manufacturer`). The necessary precondition for this question to even be
  askable — two or more real entities coexisting in one project — has never occurred in real data.
  The hypothesis is currently untestable against anything this project has actually produced.
- **Doesn't clearly extend** to the domain-event variability specifically, which already has a
  better-evidenced, unrelated explanation (a missing upstream Topic Naming Convention ADR, not a
  boundary question).

# What We Know

- Authorization already has a distinct, existing home (Stage 0 Policy Discovery's `"authorization"`
  area, governed by `unresolved-decisions-become-explicit-decision-points`'s citation mechanism).
  Service ownership, domain events, and integration boundaries do not — they share one
  undifferentiated Accept/Modify/Reject ADR gate with no signal distinguishing a naming correction
  from a genuine domain-model correction.
- `manufacturer-001` has never exercised "Modify" or "Reject" on any ADR proposal in its real
  history — there is no real edit data yet to check this against empirically, only structurally.
- Two currently-existing pipeline elements look like candidate homes and are not: Stage 3's
  `integration_groupings` (a within-story, behavior-subject test-grouping axis, not a cross-entity
  domain-grouping mechanism) and Stage 0 Policy Discovery (a business-rule category, not a
  structural one).

# What We Don't Know

- Whether a second real entity, introduced through a second real story in the same dogfooding
  project, ever actually produces a `spec` call where the model should be asked a
  relationship/grouping question and isn't.
- Whether the entity-scoped/process-scoped naming split the sweep measured is itself evidence of an
  unresolved boundary judgment, or just a stylistic default with no deeper consequence — nothing
  has traced whether the two naming conventions ever produce materially different downstream
  service graphs.
- Whether this generalizes across projects/domains or is specific to this one story's shape.

# Why Deferred

Not the current priority. The Roadmap Reassessment (2026-07-15) already promoted the Human-Insight
Inventory and the domain-event/Decision-Point question above composition's harder questions; this
is a further, more speculative branch of that same thread, gated on evidence (a second real entity)
that doesn't exist yet. Manufacturing that evidence for its own sake — e.g. running a second story
through `spec` purely to test this — would be building a multi-entity domain model to answer an
open question, not because a real story called for it, which risks repeating exactly the
"anticipatory," ahead-of-behavior pattern `structure-emerges-from-behavior` already warns against.

# Possible Experiments

- None proposed as a next action. If a real dogfooding project ever naturally accumulates a second
  entity through its own second story, that `spec` run is the first real opportunity to check
  whether a relationship/grouping question ever should have been asked and wasn't — observe it when
  it happens, rather than construct it.
- If it's ever checked: read that run's `identify_architectural_questions` output against the same
  standard `human-insight-inventory.md` used — compare proposed vs. persisted content, and whether
  any human review action shows differentiated scrutiny for a boundary-adjacent proposal.

# Exit Criteria

Resolve (or reclassify as a principle) once a real project produces a second entity and that run's
`spec` behavior can actually be observed against this question — not before. Until then this stays
`active`, not `resolved`, as a placeholder worth not losing.
