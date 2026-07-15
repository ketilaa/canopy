# Domain-Boundary Hypothesis Assessment

Status: evaluation only — assesses a proposed explanatory model against existing evidence. No
implementation change, no bounded-context feature, no pipeline redesign is proposed here or
implied by anything below.

Date: 2026-07-15

Reviewed: `docs/design/pre-behavior-planning-review.md`, `docs/design/pre-behavior-planning-
reproducibility-sweep.md`, `docs/design/human-insight-inventory.md`, `docs/design/contract-
composition-assessment.md`, `docs/design/roadmap-reassessment.md`, `docs/principles/structure-
emerges-from-behavior.md`, `docs/principles/unresolved-decisions-become-explicit-decision-
points.md`, the real dogfooding project's `domain_registry.yaml`/`services.yaml`/`stories/
manufacturer-001/clusters.yaml`, `canopy-llm/src/prompts/spec.rs` (`identify_architectural_
questions`), `canopy-core/src/lib.rs` (`ClusteringResult`/`IntegrationGrouping`),
`canopy-llm/src/prompts/clustering.rs`.

## The Hypothesis, Restated

Some architectural recommendations produced during pre-behavior planning — service ownership,
domain events, integration boundaries, authorization boundaries, data ownership — may not be pure
architecture questions. They may be standing in for a domain-boundary decision (which entities
share a bounded context, which business capability owns which data) that the pipeline never makes
explicit anywhere.

## 1. What evidence currently supports or contradicts this?

**Supports it:**

- The reproducibility sweep's own naming finding, already on record before this hypothesis was
  raised, is a concrete instance of the same shape of ambiguity. Runs 3 and 4 named the backend
  and frontend `manufacturer-service`/`manufacturer-portal` (entity-scoped); runs 1, 2, and 5 named
  them `manufacturer-registration-service`/`-portal` (process-scoped) — "a real naming-convention
  split the model applies consistently within a run," which the sweep itself flagged as something
  that "could matter for how a second, related story's service ownership resolves later," without
  naming it a boundary question. Entity-scoped vs. process-scoped naming is, structurally, two
  different implicit answers to "what does this service's boundary actually represent" — the same
  underlying decision the hypothesis says is missing an explicit home.
- `identify_architectural_questions`'s own instruction (`prompts/spec.rs:139`) is a per-story
  *existence* check: "skip if the specific service that should own THIS story's domain is already
  in Known Services." It has no instruction anywhere asking whether a *new* entity should share a
  service with an *existing* one because they belong to the same business capability — only
  whether a service already claims this exact entity. If a domain-boundary judgment were ever
  needed, there is currently no question in this prompt that asks it.
- `pre-behavior-planning-review.md`'s own already-documented "order-dependent prompt content"
  variability source is consistent with this: service ownership is decided story-by-story, against
  whatever partial service list happens to exist at that moment, in whatever order stories were
  processed — not against a stable, independently-derived domain model. If service boundaries were
  a downstream consequence of a real, already-settled domain model, processing order shouldn't
  change the outcome. That it does is at least consistent with there being no such settled model
  behind the recommendation.
- Direct historical precedent, found in `structure-emerges-from-behavior.md`: "boundaries" were a
  named, explicit upfront elicitation category at `explore` (the command that became `init`), and
  were deliberately removed (`f0e8593`, "Drop boundaries from explore questions") because asking
  about them abstractly, before any concrete behavior existed, produced worse results than
  deriving structure later from described behavior. This is the *same concept* the current
  hypothesis is about. Its resolution at the time was to defer the concept, not to declare it
  unnecessary — and nothing in the repo's history shows it was ever picked back up downstream after
  being deferred. It was dropped, not relocated.

**Contradicts, or at least sharply limits, how far this can be pushed today:**

- The single most direct evidence source — the real dogfooding project's own
  `domain_registry.yaml` — contains exactly **one** entity (`Manufacturer`), ever, across the
  project's entire history. There has never been a second entity in this project to compare it
  against. The necessary precondition for a bounded-context question to even be askable — two or
  more real entities coexisting — has not yet occurred in any real data this project has produced.
  The hypothesis is not falsifiable against current evidence in the strict sense: nothing has ever
  tested the case it's about.
- `contract-composition-assessment.md` already names "multiple entities in one story" (and by
  extension, multiple entities across stories) as an untested case, for an adjacent but distinct
  reason (contract dependency-chain depth). That gap and this one are the same missing precondition
  observed from two different investigations independently.

## 2. Does the domain-event variability fit this interpretation?

Less directly than the naming-convention evidence, and it should not be forced to fit.

Domain-event presence and topic-convention compliance was the sweep's single least-stable
category (tier 4). A bounded-context reading of that would be: whether an event should be raised
at all is really a question about whether some *other* context needs to react to this state
change — and if boundaries are unresolved, so is the answer to that question. That's a plausible
extension of the hypothesis, but this project has no other service or context in the real project
to test it against either (the same missing-precondition gap as §1), so it stays speculative.

A better-evidenced, already-cited explanation for this specific variability exists and doesn't
require invoking bounded contexts at all: `human-insight-inventory.md` and the sweep design doc
both trace the missing topic clause to a concrete mechanical dependency — the prompt's own rule
("if no Topic Naming Convention ADR exists, name the event only") — combined with the same
judgment-call-plus-no-training-default shape `unresolved-decisions-become-explicit-decision-
points` already describes at high confidence. That explanation is grounded in an actual prompt
rule and an actual missing upstream ADR; the bounded-context explanation for *this specific*
variability is not yet grounded in anything beyond plausibility. Both can be true at once (an
event's necessity and an event's formatting are different questions), but only one currently has
direct supporting evidence.

## 3. How should future edits to these categories be interpreted?

The honest answer, given what's actually recorded today: **it can't be determined from the
review action alone, and there's no real edit history yet to check against anyway.**
`human-insight-inventory.md` found that `manufacturer-001` never exercised "Modify decision text"
or "Reject" on any proposal, in any category — every one of its 5 real ADR proposals was Accepted
verbatim. So this question is currently structural, not empirical: no modification of a service-
ownership, domain-event, or integration-boundary proposal has ever actually happened in real
dogfooding data to inspect.

Structurally, the four categories named in the question are not equally instrumented:

- **Authorization** already has a distinct, existing home: Stage 0 Policy Discovery's named
  `"authorization"` area (`prompts/spec.rs:554,622`), governed by
  `unresolved-decisions-become-explicit-decision-points`'s citation-required resolved/open
  mechanism. An edit or a "no citation" rejection there is already interpretable through that
  principle's own terms — it is a policy-resolution correction, not an undifferentiated ADR edit.
- **Service ownership, domain events, and integration boundaries** currently share the same
  undifferentiated ADR review mechanism (Accept / Modify decision text / Reject, one flat gate,
  `pre-behavior-planning-review.md` row 7). A modified `decision` field carries no signal
  distinguishing "the human wanted the same boundary with a different name" from "the human drew
  a genuinely different domain grouping" — both look identical in the data: one changed string.
  Answering "architecture correction vs. domain-model correction" for these three categories, if it
  ever needs answering, would require reading the actual before/after `decision` text and reasoning
  about what changed semantically — the same manual reconstruction this investigation and
  `human-insight-inventory.md` already had to do by hand, not something the current review gate's
  own data captures on its own.

So: for authorization, "something else" — a policy-discovery resolution, already named and
governed by an existing principle. For the other three, also "something else" for now, but for a
different reason — the current instrumentation genuinely cannot tell the two apart, not because
the two aren't both plausible.

## 4. Where would bounded context / business boundary / domain ownership naturally belong, if it turned out to matter?

Reasoning from `structure-emerges-from-behavior` (high confidence, already validated against four
independent kinds of information in this project's own history): **not at `init`**, which is
exactly where this concept was tried, under the same name ("boundaries"), and removed. The
principle's own logic — derive structure from concrete described behavior, not from abstract
upfront questions — implies a boundary needs at least two concrete things to draw a boundary
between. A single-entity project (which is all real data this project has ever produced) has
nothing to draw a boundary against.

Given that, the earliest point that could ever be evidence-grounded is **the moment a second real
entity enters the picture** — a second story whose `spec` run sees an already-populated
`services.yaml`/domain registry from a prior story. That is already the one place in the current
pipeline where cross-story context reaches the model (`identify_architectural_questions`'s "Known
Services and Responsibilities" block) — not a new stage, an existing one that currently only asks
an existence question ("is there already a service for this"), not a relationship question ("should
this share a boundary with that").

Two currently-existing pipeline elements look like tempting candidate homes and are not, worth
naming explicitly so the temptation doesn't recur unexamined:

- **`integration_groupings`** (Stage 3 clustering, `canopy-core/src/lib.rs:424-430`) groups
  *behaviors* by `subject` for integration-scope test boundaries within one story — a mechanical,
  per-story grouping axis, unrelated to cross-service or cross-entity domain boundaries. It is
  empty for `manufacturer-001` (one entity, one service) precisely because nothing in that story
  needed it, not because it's an unfilled slot waiting for this concept.
  `mechanical_cluster`/`audit_clustering` (`canopy-llm/src/prompts/clustering.rs:17-70`) group
  behaviors, never entities or services — folding a bounded-context judgment in here would conflate
  two different axes (test grouping vs. domain grouping).
  
- **Stage 0 Policy Discovery** is where authorization already lives, but it classifies
  *business-rule* questions (uniqueness, defaults, retention, authorization, idempotency,
  consistency) — "which entities share a domain boundary" is a *structural* question, the same
  category service ownership and UI questions already belong to, not a policy question. Housing it
  there would mismatch the category it actually belongs to, the same "layer-scoping correctness"
  failure mode CLAUDE.md's Prompt House Style names for tech-stack skills.

None of this can be validated against real data today — it can only be reasoned about
structurally, because the necessary condition (two or more real entities in one project) has never
occurred. That is the cheapest possible next fact to check for before this question needs any
design discussion at all: whether a second entity, introduced through a second real story in the
same dogfooding project, produces a `spec` call where the model is ever actually asked (or should
have been asked, but wasn't) a relationship question about an existing entity's service — not
inferred from a single-entity project's data, because that data structurally cannot contain the
answer.

## Summary

The hypothesis is a useful explanatory model for the naming-convention variability the sweep
already measured, and for the structural gap in `identify_architectural_questions`'s own
existence-only check — both concrete, citable. It is speculative, not yet evidenced, for the
domain-event variability specifically, which already has a better-grounded alternative explanation.
It cannot currently be tested against real edit history at all, because no real edit history for
these categories exists yet. And it cannot be tested against the core multi-entity case at all,
because this project has never had more than one entity. The most honest summary: this is a
plausible, partially-evidenced account of *where* human insight might be missing, but the strongest
possible next evidence for or against it — a second real entity in a real project — doesn't exist
yet, and nothing above should be read as recommending that it be created for this purpose.
