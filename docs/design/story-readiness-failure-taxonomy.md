# A Taxonomy of Story Readiness Failure Classes

Status: taxonomy only. No mechanism, check, or detection framework proposed anywhere below, per
explicit instruction — this maps the failure space `product-010` opened up, using only evidence
already on file. Extends `docs/design/product-010-story-readiness-failure-diagnosis.md` (which
categorized one incident) into a broader survey across every real readiness-relevant finding this
project has recorded, not just that one story.

Date: 2026-07-19

Reviewed: `docs/reports/product-010-customer-vertical-slice.md`, `docs/design/human-insight-
inventory.md`, `docs/design/human-insight-inventory-rerun.md`, `docs/design/product-010-story-
readiness-failure-diagnosis.md`, `docs/open-questions/{story-readiness-vs-backlog-evolution,
domain-boundary-explicitness}.md`, `docs/design/{domain-event-decision-point-criteria-comparison,
unestablished-referent-hypothesis-review, domain-boundary-hypothesis-assessment}.md`, `docs/design/
roadmap-reassessment.md` (2026-07-16 update, Product-Owner Perspective Experiment findings),
`docs/principles/{unresolved-decisions-become-explicit-decision-points, cross-artifact-consistency-
audits-prevent-drift, exhaustive-enumeration-over-holistic-review, compute-facts-mechanically}.md`.

---

# Candidate Readiness Failure Classes

Eight candidates surface from the evidence on file. Not claimed exhaustive — these are the classes
this project's own real findings actually support naming, nothing more.

**A. Cross-artifact, same-story contradiction** — two fields or generated artifacts belonging to
the *same story* assert incompatible things (a scope exclusion and an accepted behavior that
presupposes the excluded thing).

**B. Unresolved decision silently resolved** — a genuine open business question is recorded as
settled (via a vacuous or absent citation, or by never being surfaced as a question at all) rather
than routed to a human.

**C. Capability or entity presupposed but never established** — a story's accepted output depends
on a capability, entity, or relationship that has no representation anywhere else in the project
(no domain-registry entry, no role, no other story, no ADR).

**D. Ambiguous referent / undefined role semantics** — a term or actor label is used fluently as
though its meaning were already settled, without ever being confirmed against the project's own
vocabulary.

**E. Dependency assumed but never modeled** — a story structurally requires another story's or
contract's capability, and no `depends_on` entry or dependency edge names the relationship.

**F. Checklist/enumeration axis missing at the review-mechanism level** — a readiness-checking
stage's own enumerated checklist has no item for a class of thing that needed checking (distinct
from an existing item being under-walked).

**G. Instruction-compliance gap** — a downstream artifact violates an unconditional, already-stated
rule; not a judgment call the model lacked a basis for, a rule it simply didn't reliably follow.

**H. Missing-upstream-fact / sequencing gap** — a downstream artifact's output is the textually
correct fallback given the project's current state, but looks incomplete only because an upstream
decision (an ADR, a convention) hasn't been made yet — flagged separately below because it is the
one candidate whose status as a "failure" at all is itself questionable.

---

# Evidence For Each

### A — Cross-artifact, same-story contradiction

- `product-010`: `out_of_scope` explicitly excludes "Customer authentication and authorization";
  the accepted scenarios and contract include a full `401 Unauthorized` behavior pair. Confirmed
  directly against the artifacts (`product-010-customer-vertical-slice.md`).
- Code-level confirmation that no check exists at this layer: grepping every consumer of the
  `out_of_scope` field across the codebase shows it is set once, echoed to the console at review
  time, and never read again by Stage 1, 2, 3, or 4 (`product-010-story-readiness-failure-
  diagnosis.md`, §2.2).
- **Detectable from one artifact?** No — requires comparing at least two fields/artifacts
  (`out_of_scope` vs. the scenario/contract set) within the same story.
- **Requires project-wide context?** No — fully resolvable using only that one story's own
  directory, independent of anything else in the project (established via the counterfactual in
  the prior diagnosis: the contradiction holds even if the presupposed capability existed
  elsewhere).

### B — Unresolved decision silently resolved

- `product-010`: the `authorization` policy item was filed under `resolved_policies` with the
  citation *"the story does not explicitly mention any authorization requirements"* — an absence of
  evidence accepted as a positive fact. Code-confirmed: `bucket_policy_checklist`
  (`canopy-llm/src/prompts/spec.rs`) checks only that `evidence` is *present*, not that its content
  is on-topic and positive.
- `manufacturer-001`, independently, earlier: the pre-fix reproducibility evidence behind
  `unresolved-decisions-become-explicit-decision-points` itself — 5 of 6 named policy questions
  "resolved" with specific, invented answers with **zero** citation at all (a more severe variant —
  no citation, rather than a vacuous one).
- `manufacturer-001`'s duplicate-name handling (`unestablished-referent-hypothesis-review.md`, §4):
  the model picked name-string equality as the uniqueness criterion and never flagged the
  alternative — read by that document's own final analysis as "an instance of the existing,
  validated principle that this one story's Stage 2 run happened to miss," not a new class.
- **Detectable from one artifact?** Yes, for the citation-loophole variant — reading a
  `resolved_policies` entry's own `evidence` field against its `classification` is enough, no
  comparison to anything else needed. The "never surfaced as a question at all" variant
  (duplicate-name) is closer to a single-stage generation question — still resolvable by reading
  the one artifact where the policy was decided, not by comparing across stories.
- **Requires project-wide context?** No.

### C — Capability or entity presupposed but never established

- `product-010`: no authentication/session/identity capability exists anywhere in this project;
  the accepted 401 behavior presupposes one anyway.
- `manufacturer-001`, independently, earlier: the Product-Owner Perspective Experiment
  (`roadmap-reassessment.md`, 2026-07-16 update) found "authenticated" in every one of 12
  scenarios' `given` clauses, with no `security` scheme anywhere in the generated OpenAPI spec and
  no authentication capability anywhere in the project — noticed by exactly one of five simulated
  reviewers.
- `manufacturer-001`, a second, structurally distinct instance in the same experiment: the story's
  own `so_that` field ("so that products can reference them") names a `Product` relationship that
  `domain_registry.yaml` has never captured, in this project's entire history.
- **Detectable from one artifact?** No, by definition — requires checking `domain_registry.yaml`,
  `roles.yaml`, and every other story to establish absence.
- **Requires project-wide context?** Yes, definitionally — this is the one class built entirely
  around project-wide grounding.

### D — Ambiguous referent / undefined role semantics

- `manufacturer-001`: `roles.yaml` contains exactly one undefined line, "manufacturer
  representative" — never elsewhere defined as internal staff or an external actor. Noticed by
  exactly one of five simulated Product-Owner personas.
- This was investigated directly as a candidate for its own class ("unestablished referent",
  `unestablished-referent-hypothesis-review.md`) and **explicitly downgraded**: the reviewing
  document's own conclusion reads role-semantics and authorization as "one gap, not two," best
  explained by class F below (Stage 0/2's checklists don't currently enumerate "is every named
  actor/role defined" as an item), not evidence of an independently new failure shape.
- **Detectable from one artifact?** In principle yes (comparing `roles.yaml`'s bare definition
  against how the term is used in scenarios), but no dedicated check for this has ever been
  proposed or tested — it remains a within-story property, not one needing project-wide state.
- **Requires project-wide context?** No.

### E — Dependency assumed but never modeled

- No confirmed real instance found anywhere in the reviewed material. The one case that looked like
  a candidate — `product-010`'s authorization gap — was directly argued, in the prior diagnosis
  document, to fit class A/C better: the story's own `out_of_scope` field *declines* the
  dependency rather than omitting a link to it, which is a contradiction, not an omission.
- The Contract Composition investigation's real dependency edges (Stage 6, `EventShape`/
  `Publication`) are examples of a dependency being *correctly* modeled, not a counterexample of one
  being missed — no failure instance exists in that material either.
- **Detectable from one artifact?** No.
- **Requires project-wide context?** Yes, would be, if a real instance existed.

### F — Checklist/enumeration axis missing at the review-mechanism level

- The general shape is independently, directly confirmed elsewhere: Stage 0's original
  constraint-completeness check found 4 of 9 real gaps holistically, and 9 of 9 once rewritten as
  an explicit field × constraint traversal (`exhaustive-enumeration-over-holistic-review`,
  high confidence, validated).
- Its *application* to the role-semantics/authorization gap specifically is the best current
  explanation offered (`unestablished-referent-hypothesis-review.md`, §5): "Stage 0/2's own
  checklists don't currently enumerate role-definition or authorization-implication as items to
  check" — but that document is explicit that this is "a testable, narrow... question," not
  something verified by an actual fix-and-remeasure the way the original 4/9 → 9/9 result was.
- `product-010`'s own diagnosis (§2.5) considered this framing directly and rated it low-medium
  confidence as a fit for *that* incident specifically, since there was never an enumerated
  checklist axis for "compare `out_of_scope` against scenario content" to under-walk — the axis
  itself didn't exist, which is a related but distinct gap from an existing axis being walked
  incompletely.
- **Detectable from one artifact?** Yes, typically — checkable by comparing a stage's own stated
  checklist scope against what a human reviewer would think to check, without needing state beyond
  what that stage is already given.
- **Requires project-wide context?** No.

### G — Instruction-compliance gap

- `domain-event-decision-point-criteria-comparison.md`, §2: whether a domain event should exist at
  all is stated as an unconditional rule ("MANDATORY whenever... event-driven... creates/updates/
  deletes an aggregate") — not framed as a judgment call at all — yet the reproducibility sweep
  measured it firing in only 3 of 5 runs. A stated rule not reliably followed, not an unresolved
  question the model lacked grounding for.
- **Detectable from one artifact?** Yes — checkable directly against the stated rule and the
  produced output.
- **Requires project-wide context?** No.
- Not observed in `product-010` itself — this class's only confirmed instance is domain-event
  existence in the reproducibility sweep, a different mechanism (spec-generation reliability)
  than the Stage 0–4 readiness gates `product-010` exercised.

### H — Missing-upstream-fact / sequencing gap

- Same source, sub-decision (c): the "`<EventName>` on topic `<topic>`" clause is only produced
  when a Topic Naming Convention ADR already exists; when it doesn't, "name the event only" is the
  textually correct fallback, not an invented answer. The real project genuinely had no such ADR at
  `spec` time — the sweep's 1-of-5 convention-compliance rate reflects that true absence, not
  fabrication.
- **Detectable from one artifact?** Only by also knowing whether the upstream ADR exists — which
  makes this closer to class C's project-wide shape than to a single-artifact property.
- **Requires project-wide context?** Yes.
- Flagged distinctly because the criteria-comparison document's own analysis treats this as *not*
  a defect in the artifact produced — the fallback is correct given the state that actually
  exists — which raises a genuine question of whether this belongs in a *failure* taxonomy at all,
  addressed in Open Questions below.

---

# Which Classes Are Already Confirmed

Confirmed means: at least one real instance exists in this project's actual dogfooding history,
not a hypothetical constructed for this document.

| Class | Confirmed instances | Stories involved |
|---|---|---|
| A — cross-artifact same-story contradiction | 1 | `product-010` |
| B — unresolved decision silently resolved | 3 (citation-loophole, zero-citation fabrication, unflagged duplicate-name) | `product-010`, `manufacturer-001` (×2) |
| C — capability/entity presupposed, never established | 3 (authorization ×2, `Product` relationship) | `product-010`, `manufacturer-001` (×2) |
| F — checklist/enumeration axis missing | 1 directly proven (Stage 0 constraint audit), 1 plausibly explained but unproven (role-semantics) | project-wide mechanism; `manufacturer-001` for the applied case |
| G — instruction-compliance gap | 1 (domain-event existence, measured 3/5) | `manufacturer-001` (reproducibility sweep) |
| H — missing-upstream-fact / sequencing gap | 1 (domain-event topic clause, measured 1/5) | `manufacturer-001` (reproducibility sweep) |

**B and C are the best-evidenced classes in this taxonomy** — each with independent instances
across two different stories, not single anecdotes. That's a materially stronger evidence base
than the original `product-010` diagnosis alone suggested, since that document only had
`product-010` itself to reason from; folding in the Human-Insight Inventory and Product-Owner
Perspective Experiment material shows both classes recurring.

# Which Classes Are Still Hypothetical

| Class | Status |
|---|---|
| D — ambiguous referent / undefined role semantics | Investigated directly and **downgraded**, not merely unconfirmed — `unestablished-referent-hypothesis-review.md` concluded this reduces to class F (a checklist gap), not a standalone class. Recorded here because the investigation itself is real evidence about the shape of the space, even though its conclusion was negative. |
| E — dependency assumed but never modeled | No real instance found anywhere in the reviewed material. Structurally plausible (the story-level analogue of `entity-with-no-story`), but the one candidate case (`product-010`) was argued to fit A/C instead. Remains a real gap in the *evidence*, not a claim that the class can't occur. |

---

# Where `product-010` Fits

`product-010` is a confirmed instance of **A** (the load-bearing, detectable-in-principle property:
its own `out_of_scope` and its own accepted output contradict each other) with **B** as the
upstream mechanical cause (the citation-loophole that let the authorization question skip Stage 2
entirely) and **C** as real, confirmed context that sharpens the incident's severity without being
required to detect it (per the counterfactual in the prior diagnosis: the contradiction exists
independent of whether authentication happens to be built elsewhere).

It is **not** an instance of D (its role, `customer`, was never flagged as ambiguous), E (the
dependency is disclaimed, not omitted), G, or H (neither concerns Stage 0–4's readiness gates —
both are spec-generation reliability findings from a different investigation). It touches F only
weakly: Stage 0 never had a checklist axis for `out_of_scope`-vs-scenario comparison, but that's
better described under A (no check exists at that layer at all) than under F's classic shape (an
existing item under-walked).

---

# Open Questions That Remain

- **Does class A's gap generalize, or is it specific to `out_of_scope`?** Only one field
  (`out_of_scope`) has been checked against downstream content and found unconsulted anywhere.
  Whether other same-story fields (e.g., a policy's `detail` text vs. the actual generated
  validation rule) have the same blind spot is unexamined.
- **Does class C generalize beyond authentication, or is authentication the one capability every
  story so far happens to silently assume?** All three confirmed C instances involve
  authentication/authorization or one specific missing entity (`Product`). No evidence yet
  distinguishes "this project's stories keep colliding with the same two gaps" from "any
  presupposed-but-absent capability produces this failure shape."
- **Is F's explanation of the role-semantics/authorization gap actually correct, or just
  plausible?** `unestablished-referent-hypothesis-review.md` itself frames this as untested — the
  original Stage 0 constraint-audit case had a direct before/after fix-and-remeasure; the
  role-semantics application of the same principle does not yet have one.
- **Do D and H actually deserve separate slots, or do they collapse further?** D was already folded
  into F by direct investigation. H's status as a "failure" at all is unsettled — the criteria-
  comparison document treats the topic-clause omission as a *correct* fallback given true upstream
  state, not a defect, which raises whether it belongs in a failure taxonomy or in a separate
  "expected variability, not a defect" category not attempted here.
- **Is E genuinely rare, or just unobserved because no real project has produced the multi-story,
  multi-entity dependency structure that would surface it?** `domain-boundary-explicitness.md`
  already names the same precondition gap for a related question (a second real entity coexisting
  with a first) — both open questions may resolve together once a project accumulates that shape
  of real data, rather than needing two separate future experiments.
- **Do A, B, and C compose in a fixed pattern, or did `product-010` just happen to exercise all
  three at once?** This taxonomy currently has one story where all three co-occur and no story
  where they occur separately in isolation — insufficient to say whether they're independent axes
  or a bundle that tends to arrive together.
