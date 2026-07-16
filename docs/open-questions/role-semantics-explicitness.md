---
title: Does Canopy Ever Ask What an Actor Label Actually Means?
status: active
origin: surviving concern from the Exploration Enumeration Gap Investigation
date_discovered: 2026-07-16
related_principles: [unresolved-decisions-become-explicit-decision-points, structure-emerges-from-behavior]
related_narratives: []
related_reports: [manufacturer-001]
related_design_docs: [product-owner-perspective-experiment.md, exploration-enumeration-gap-investigation.md, human-insight-inventory.md, role-semantics-investigation.md]
---

# Question

Does any part of the pre-behavior pipeline ever ask what an actor/role label (e.g. `manufacturer
representative`) actually denotes in this business's context — internal operator vs. external
party, employee vs. customer, trusted vs. unverified — or is a role's real-world meaning always
treated as an already-settled fact once its label has been typed?

# Why It Matters

The Product-Owner Perspective Experiment found that `manufacturer representative`'s meaning is
genuinely ambiguous (self-registering external party vs. internal catalog operator), and that this
ambiguity sits *logically prior to* the authorization question — you cannot decide what an actor is
permitted to do until you know what kind of actor it is. The subsequent Exploration Enumeration Gap
Investigation checked this against the actual current prompt code and found the pipeline's one
adjacent check (the business-policy checklist's `authorization` area) explicitly presupposes the
actor's identity is already resolved — it only asks about permission *beyond* an already-settled
"authenticated" actor, never who that actor is. Unlike two other candidate gaps from the same
investigation (uniqueness, authorization), which turned out to already be enumerated in current
code and whose appearance in `manufacturer-001` was better explained by that artifact predating the
mechanism, role semantics has no such alternative explanation — nothing anywhere in the pipeline
asks this question, at any point in this project's history.

# Evidence So Far

See `docs/design/exploration-enumeration-gap-investigation.md` for this question's origin, and
`docs/design/role-semantics-investigation.md` for a full dedicated pass (role inventory across
every stage, a stage-by-stage trace of `manufacturer representative`, a counter-evidence search,
and relationship analysis against both cited principles). Summary:

- **A real counter-mechanism exists, but doesn't reach the path that matters**: `Role::Described`
  (`canopy-core/src/named_described.rs`) is a genuine, tested, human-facing description-capture
  channel — but it's only reachable through `init`'s bootstrap flow, which has never fired for this
  project's one real role. `intent.rs`'s automatic per-story role registration (the path that
  actually produced `manufacturer representative`) hardcodes `Role::Simple` with no human gate and
  no model-supplied description at all — confirmed directly in code, not inferred.
- **Authorization does not implicitly cover this**: the business-policy checklist's own
  `authorization` wording ("beyond the actor already being authenticated") presupposes the actor's
  identity is already settled; it structurally cannot ask who the actor is, even when working
  correctly.
- **Relationship to principles, more precisely stated**: fits `unresolved-decisions-become-
  explicit-decision-points`'s *shape* closely (a judgment call resolved silently, no supporting
  basis) but was never inside that mechanism's actual enumerated scope. Compatible with, not
  contradicted by, `structure-emerges-from-behavior` — that principle argues roles should emerge
  from behavior rather than upfront elicitation, which is exactly what happened here; the gap is
  narrower and later-stage: whether an already-emerged role's meaning is ever clarified afterward,
  which that principle's own logic neither requires nor forbids.
- **Mechanism vs. instance, precisely separated**: the automatic registration path that produced
  this gap is universal — it fires identically for every accepted story's role, regardless of name
  — so the *mechanism* is not isolated to this one story. Whether the *semantic ambiguity itself*
  recurs across differently-worded roles remains genuinely unverified, with only one real role to
  check against.

- **Supports it**: a role inventory across `intent.rs` (story generation), `roles.yaml` (the
  registry), the business-policy checklist (`authorization` area), and scenario generation found no
  point anywhere that asks what a role label denotes — every stage either stores the bare string
  verbatim or reuses it as an unexamined given.
- **Distinguishes it from the two eliminated candidates**: uniqueness and authorization are both
  already explicit checklist items in current code; `manufacturer-001`'s artifact simply predates
  the mechanism by about four hours (confirmed via `llm-debug.log` timestamps against the
  introducing commit). No equivalent mechanism exists for role semantics to predate — there has
  never been one.
- **Distinguishes it from the `Product`-relationship concern**: that concern is entangled with
  `structure-emerges-from-behavior`'s deliberate exclusion of purpose-clause-only entities — a real
  design tension, not a clean gap. Role semantics has no equivalent competing principle pulling the
  other way (see the design doc's §5 relationship analysis for the full reasoning).

# What We Know

- `intent`'s story-generation prompt instructs: "Reuse a known role if it fits; introduce a new role
  only when genuinely needed" — this governs role *reuse*, not role *definition*.
- `roles.yaml` stores `Role::Simple`, a bare string, with no schema field for definition, scope, or
  internal/external classification.
- The registry update for a newly-accepted story's role is fully automatic — no human gate exists at
  this specific step (per `pre-behavior-planning-review.md`'s Review And Approval Flow table, row 5).
- `manufacturer-001` has exactly one role, `manufacturer representative`, and its meaning has never
  been resolved or even flagged as ambiguous anywhere in this project's real history.

# What We Don't Know

- Whether this is a real, recurring phenomenon across other actors this project's own history has
  produced (or would produce), or specific to how this one story's `as_a` field happened to be
  worded — `manufacturer-001` is currently the only real, fully-specced story in the dogfooding
  project, so there is exactly one data point.
- Whether role ambiguity of this shape is common in realistic domains generally (customer vs.
  administrator vs. product manager vs. supplier vs. reviewer), or whether `manufacturer
  representative` was an unusually ambiguous choice of wording that a differently-phrased `as_a`
  would not have produced.
- Whether resolving this would naturally fit the existing Decision Point mechanism's own shape
  (a recognized, unsupported judgment call) or would require the question to be recognized as
  existing in the first place — which is itself the open part of this question.

# Why Deferred

Not the current priority, and — per the investigation that surfaced it — not yet ready for a
proposed mechanism regardless of priority: only one real story's data exists to check this against,
and the investigation's own charter was evaluation, not design. Filing this now is preservation, the
same rationale `domain-boundary-explicitness.md` used for its own adjacent, similarly single-story-
evidenced concern.

# Possible Experiments

- None proposed as a next action, consistent with the investigation's own scope. If a second real
  story with a differently-shaped `as_a` value is ever produced through normal dogfooding work, that
  is the first real opportunity to check whether this recurs, rather than manufacturing a second
  story purely to test it.
- If it's ever checked: repeat the same role-inventory trace this question's origin investigation
  used, against the new story's own `as_a` value, and note whether the ambiguity shape (external vs.
  internal actor identity) recurs, or whether `manufacturer-001`'s wording was the unusual case.

# Exit Criteria

Resolve (or reclassify) once a second real story's role data exists to check recurrence against, or
once a future investigation determines — from evidence, not assumption — that this is either a
one-off wording artifact or a general property of how `as_a` fields get generated. Until then stays
`active`, not `resolved`.
