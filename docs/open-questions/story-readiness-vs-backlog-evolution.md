---
title: What Would a Story Readiness Signal Have Needed to Detect the product-010 Authorization Gap?
status: active
origin: Customer vertical slice (Iteration 3, backlog execution plan), evaluated against Iteration 1 (entity-with-no-story)
date_discovered: 2026-07-19
related_principles: [unresolved-decisions-become-explicit-decision-points, exhaustive-enumeration-over-holistic-review, structure-emerges-from-behavior]
related_narratives: []
related_reports: [product-010-customer-vertical-slice]
related_design_docs: [roadmap-reassessment.md]
---

# Question

`product-010` (Customer browses the catalog) passed every existing Story Readiness check —
Stage 0 completeness (`gaps: []`), Stage 2 decisions (`decisions: []`), Stage 3 clustering audit
(`findings: []`), Stage 4 contract audit and dependency review (both empty) — while its own
artifacts directly contradict each other: `out_of_scope` explicitly excludes "Customer
authentication and authorization," yet the accepted scenarios and the accepted contract include a
full `401 Unauthorized` behavior, and no authentication capability exists anywhere in the project.
Is this concrete evidence that Backlog Evolution (does a story exist for concept X) and Story
Readiness (is this already-existing story internally sound) are genuinely separate questions, not
two views of the same one? And precisely: what would a Story Readiness signal have needed to
detect, at each stage, to catch this specific case?

# Why It Matters

The roadmap's own stated premise — "a story can be internally complete while the surrounding
capability area remains incomplete" — was asserted from analysis, then confirmed live in the first
real vertical slice run under it. If the two questions really are separate, and if what's missing
from Story Readiness also can't be filled by extending Backlog Evolution (see below), that names a
real, currently-unaddressed gap class rather than an argument for building one bigger check.

# Evidence So Far

Full session and mechanism-by-mechanism trace in `docs/reports/product-010-customer-vertical-
slice.md`. Summary of what was actually established, not inferred:

- **Backlog Evolution (entity-with-no-story) could not have caught this, even in principle** —
  not merely didn't. Authorization/authentication is not an entity; it never appears in
  `domain_registry.yaml`, so there is nothing for that check to diff against. It operates one level
  of abstraction away from where the contradiction actually lives.
- **Each existing Story Readiness mechanism ran, on this exact story, and reported clean, for a
  specific, traceable reason each time**:
  - Stage 0 checks scenario coverage and outcome observability — it never cross-reads
    `out_of_scope`'s content against the scenario set's content.
  - Stage 2 only processes questions already flagged `unresolved`. The authorization item was
    filed under `resolved_policies`, with the resolution text *"the story does not explicitly
    mention any authorization requirements"* — an absence of evidence recorded as if it were a
    positive finding. Because it was never flagged open, the decision-gating mechanism never saw
    it — Stage 2 is entirely downstream of a spec-stage classification that was itself wrong.
  - Stage 3 checks whether the mechanical clustering is structurally sound — it is (all 10
    behaviors genuinely belong to one cluster). Structural soundness and semantic soundness are
    different properties; this stage only ever checks the former.
  - Stage 4 checks contract-to-behavior mapping and cross-contract dependencies — the 401 behavior
    is correctly owned by the one contract that exists. Nothing here is scoped to ask whether an
    owned behavior presupposes a capability absent elsewhere in the project.
- **The miss doesn't cleanly sort into either bucket.** It isn't "a story is missing" (Backlog
  Evolution's territory) and it isn't purely "this story's own fields disagree with each other"
  either — full resolution requires knowing whether an authentication capability exists *anywhere
  else* in the project, a project-wide check, but along a capability axis rather than an entity
  axis. This is the same *kind* of question entity-with-no-story asks, applied to something that
  isn't an entity and that check cannot reach.

# What We Know

Three distinct things a signal would have needed to detect, established directly from this one
session, not proposed as a design:

1. A misclassification at the point of origin: an `unresolved_policies`-shaped citation (an
   absence, not a positive fact) was filed as `resolved_policies` instead.
2. A same-story, cross-field semantic contradiction: `out_of_scope`'s content and the accepted
   scenario/behavior content materially overlap, and nothing today compares the two.
3. A project-wide grounding check for a *capability*, not an entity — structurally outside what
   entity-with-no-story, as currently scoped, can reach.

# What We Don't Know

- Whether this is a one-off artifact of this specific story's phrasing (a policy resolution
  written carelessly) or a reproducible pattern — only one story has been observed under this
  premise so far.
- Whether item 1 alone (fixing the resolved/unresolved classification) would have been sufficient
  to catch this via the *existing* Stage 2 Decision Point mechanism, or whether items 2 and 3 are
  independently necessary even if item 1 is fixed.
- Whether other completed stories in this project (`manufacturer-001`, `product-001`...`008`) carry
  the same class of contradiction undetected, or whether this is specific to a story that
  presupposes a capability (authentication) with genuinely zero representation anywhere in the
  project, a condition that may not recur often.
- Whether "capability-level grounding," if it generalizes, would need its own registry-like
  artifact (the way entities have `domain_registry.yaml`) or can be answered from what already
  exists — not evaluated here.

# Why Deferred

Explicitly not resolved here on the user's instruction: record the observation, do not propose a
mechanism yet. Recording it now rather than losing it, per this project's own open-questions
convention, while more evidence accumulates from the remaining decided roadmap work (Iteration 2,
future vertical slices) before any design is attempted.

# Possible Experiments

None proposed as a next action, matching the instruction this question was recorded under. If a
future vertical slice reaches contract generation, checking whether its own `out_of_scope` and
accepted scenario/behavior content agree — by hand, not via a new mechanism — would be the cheapest
way to learn whether this was a one-off or a pattern, before designing anything.

# Exit Criteria

Resolve, or reclassify as a principle, once either: (a) a second real story surfaces the same class
of contradiction, giving two independent data points instead of one; or (b) a deliberate design
pass is authorized to address it directly. Until then this stays `active`.
