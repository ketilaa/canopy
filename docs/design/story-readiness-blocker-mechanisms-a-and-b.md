# Smallest Implementable Mechanisms for Story Readiness Blockers A and B

Status: mechanism design, scoped narrowly per the user's own constraints — no readiness framework,
no pipeline redesign, no new phase, and no attempt to solve C/D/F/G/H here. Both proposals are
sized to be the next dogfoodable iteration, not a finished architecture.

Date: 2026-07-19

Reviewed: `docs/design/story-readiness-{failure-taxonomy, failure-severity-classification,
reduction-to-fundamental-blockers}.md`, `docs/design/product-010-story-readiness-failure-
diagnosis.md`, `docs/principles/{cross-artifact-consistency-audits-prevent-drift, unresolved-
decisions-become-explicit-decision-points, exhaustive-enumeration-over-holistic-review, compute-
facts-mechanically}.md`; code: `canopy-core/src/lib.rs` (`GapKind`, `GapSeverity`,
`CompletenessGap`, `SpecificationCompleteness`, `IntentSpec`, `ResolvedPolicy`), `canopy-llm/src/
prompts/behaviors.rs` (`specification_completeness_prompt`, `identify_specification_gaps`,
`checklist_section`, `scenario_reference_listing`), `canopy-llm/src/prompts/spec.rs`
(`bucket_policy_checklist`), `canopy-cli/src/commands/behaviors.rs` (Stage 0 call site,
`has_blocking_gaps`).

Both mechanisms below reuse machinery that already exists and already enforces the same severity
tier being proposed — neither introduces a new artifact, a new pipeline stage, or a new blocking
code path. They extend two call sites that already do almost exactly this job today.

---

# Proposed Mechanism For A

**A — Cross-artifact same-story contradiction.** The confirmed instance: `product-010`'s
`out_of_scope` field excludes "Customer authentication and authorization" while its own accepted
scenarios/contract require it.

**1. What exact signal would be checked?** For each item in `out_of_scope`, walked one at a time
against the story's own scenario set (never holistically): does any scenario's `given`/`when`/
`then` content presuppose or require the concern that item excludes? A bounded yes/no per pair,
not an open-ended "look for contradictions" read — the same enumeration discipline
`exhaustive-enumeration-over-holistic-review` already established for this exact mechanism (Stage
0's Checklists 1–3 already work this way, and that rewrite is what took the original constraint
audit from 4/9 to 9/9).

**2. Which existing artifact(s) would be compared?** `IntentSpec.out_of_scope` (`Vec<String>`)
against `IntentSpec.scenarios` (`Vec<Scenario>`) — both fields already live on the same struct,
already loaded together, already passed as one argument (`spec: &IntentSpec`) into
`identify_specification_gaps`. No new artifact needs to be read, loaded, or persisted.

**3. Where in the current pipeline should it run?** Inside Stage 0 (`identify_specification_gaps`,
`canopy-llm/src/prompts/behaviors.rs:233`), as a fourth checklist alongside the three that already
exist there. The prompt already builds a `scenario_reference` block once and reuses it across
multiple checklists (`checklist_section`, line 129) — a "Checklist 4 — Scope contradiction" is one
more call to that same helper, built from `spec.out_of_scope.iter()` directly (no computation
needed; unlike Checklist 1's constraint candidates, `out_of_scope` items are already the exact
granularity needed). This is Stage 0's existing call site, run at its existing point in the
pipeline (after `canopy spec`, before Stage 1 behavior extraction) — no new phase.

**4. Blocker, warning, or explicit decision point?** Blocker. A new `GapKind::ScopeContradiction`
variant, with `severity()` returning `GapSeverity::Gap` (blocking) — a one-line addition to the
`match` in `GapKind::severity()` (`canopy-core/src/lib.rs`), reusing the exact mechanism that
already halts `canopy behaviors` today: `SpecificationCompleteness::has_blocking_gaps()` is already
called at `canopy-cli/src/commands/behaviors.rs:77` and already stops the command before Stage 1
when any blocking gap exists. No new control-flow branch is needed — the existing one already does
what a Blocker-tier response requires.

**5. What evidence would prove the mechanism useful?** Re-running this checklist against
`product-010`'s actual persisted `spec.yaml` fires the new gap for the authorization item,
reliably (checked across a small number of repeat runs, matching this project's own standing
reproducibility bar) — and, just as important, running it against every other already-accepted
story's spec (`manufacturer-001`, `product-001`–`008`) produces zero false positives, since none
of those stories' `out_of_scope` entries are known to contradict their own scenarios.

**6. What evidence would falsify its value?** A high false-positive rate against the existing
corpus (flagging legitimate exclusions with no real contradiction) would falsify it directly —
this is the one genuinely new risk, since this is the first *semantic* (not lexical) audit this
project has tried, and `cross-artifact-consistency-audits-prevent-drift`'s own Future Validation
section explicitly names this as untested. Reproducibility variance (the same story producing a
different yes/no on repeat runs) would also weaken it, given the project's own reproducibility
sweeps have found semantic/judgment-shaped LLM outputs vary more than mechanical ones.

---

# Proposed Mechanism For B

**B — Business-significant decision silently treated as resolved.** The confirmed instance:
`product-010`'s `authorization` policy item was accepted as `resolved` with the citation "the
story does not explicitly mention any authorization requirements" — an absence of evidence, not a
positive fact, passed through the existing evidence-presence check.

**1. What exact signal would be checked?** Does a `resolved`/`not_applicable` item's `evidence`
text read as a report of absence rather than a citation of a positive fact? Checked via a short,
explicit set of phrases that name their own vacuity — "does not mention," "does not explicitly
state," "no mention of," "not specified," "nothing in the story" — matching `product-010`'s actual
observed text almost verbatim ("does not explicitly mention"). This is narrower and more targeted
than a broad grounding-quality check (see Risks below for why that trade-off is deliberate for a
first iteration).

**2. Which existing artifact(s) would be compared?** None beyond the item's own `evidence` string
— this is a single-artifact, single-field check, not a cross-artifact comparison (distinguishing it
cleanly from Mechanism A). It runs on the exact same `PolicyChecklistItem`/`ResolvedPolicy` data
`bucket_policy_checklist` already handles.

**3. Where in the current pipeline should it run?** Inside `bucket_policy_checklist`
(`canopy-llm/src/prompts/spec.rs:694`), the exact function that already enforces "resolved requires
`detail` and `evidence` both present." This is a second condition added to the same match arm that
already exists for `"resolved"` (and, per the function's own doc comment, `"not_applicable"` is
already held to the identical bar) — not a new function, not a new call site, not a new stage.

**4. Blocker, warning, or explicit decision point?** Blocker, matching the existing enforcement's
own severity for the sibling case: `bucket_policy_checklist` already fails loudly (returns
`Err`, forces a re-run of `canopy spec`) when `evidence` is absent entirely. Extending "absent" to
"absent or self-referentially vacuous" keeps the exact same failure mode already in place for the
zero-citation case — no new severity machinery, just a wider match on an existing rejection path.

**5. What evidence would prove the mechanism useful?** The phrase check fires on `product-010`'s
actual persisted `authorization` evidence text, and does not fire on any of `manufacturer-001`'s
five real ADR-adjacent resolutions or `product-010`'s other resolved-policy items (a
false-positive check against the same real corpus used for Mechanism A). Both checks should run
against the same real artifacts before either is considered validated, not in isolation.

**6. What evidence would falsify its value?** A legitimate resolution that genuinely, positively
cites `out_of_scope` (e.g., "not applicable — authorization is explicitly excluded per
`out_of_scope`") getting caught by the phrase list would be a real false positive, since citing an
explicit scope decision is a positive fact, not an absence — this is a distinguishable case from
`product-010`'s actual text ("does not mention," a report of nothing having been said) but the
phrase list would need checking against exactly this edge case before trusting it. A second
falsifying observation: the same vacuous-citation failure recurring in a story where none of the
listed phrases appear (a differently-worded absence-report) would show the phrase list has poor
recall, not just a precision risk — the natural next escalation in that case would be the broader
grounding check named in the Risks section, not proposed here as v1.

---

# Why These Are The Smallest Useful Interventions

Both proposals share the same shape, deliberately: **extend an existing, already-blocking check
with one more condition, using data the check already has in hand.** Neither adds a database, a
registry, a new artifact type, a new command, or a new confirmation gate. Concretely:

- **Zero new artifacts.** Mechanism A compares two fields already on `IntentSpec`. Mechanism B
  inspects a field (`evidence`) `bucket_policy_checklist` already reads.
- **Zero new pipeline stages.** Both live inside functions that already run at the exact point in
  the pipeline where the relevant defect first becomes checkable — Stage 0 for A (the earliest
  point where `out_of_scope` and the full scenario set coexist), `bucket_policy_checklist` for B
  (the exact function this project's own principle doc already names as the enforcement point that
  has the loophole).
- **Zero new severity machinery.** A reuses `GapKind::severity()` and `has_blocking_gaps()`,
  functions that already exist and already halt the pipeline for other gap kinds. B reuses
  `bucket_policy_checklist`'s existing `Err`-and-re-run path. Neither needs a new "is this a
  blocker" decision procedure — both borrow one already validated in production.
- **Mechanical where the evidence says mechanical is enough, one LLM call only where it's
  genuinely required.** B is fully deterministic Rust (a phrase check) — no new LLM call at all.
  A cannot be, since detecting whether a scenario "presupposes" an excluded concern is a semantic
  judgment no string match can make reliably (already established: this is the one class in the
  whole taxonomy that is confirmed to have no code-only precedent, unlike Entity/Event Continuity's
  exact-name matching) — but even there, the LLM's role is narrowed to one bounded per-pair
  question per the enumeration principle, and the actual pass/fail decision remains fully
  mechanical Rust code (any "yes" → blocking gap), matching `compute-facts-mechanically`'s
  boundary: reserve the model for the one part that's genuinely judgment, compute everything else.
- **A and B remain independently necessary, not redundant, even though they co-occurred in
  `product-010`.** Fixing B alone would route the authorization question to `open_questions`
  instead of `resolved_policies` — but Stage 2's Decision Point mechanism, per the earlier
  diagnosis, never reads `out_of_scope` either, so a human resolving that Decision Point still
  isn't shown the contradiction automatically. Mechanism A is a separate backstop for exactly this
  case, not a mechanism made redundant by fixing B.

---

# What We Would Learn From Dogfooding Them

- **Whether semantic (not lexical) cross-artifact auditing is viable at all in this pipeline** —
  the single most valuable thing this iteration could establish, since `cross-artifact-
  consistency-audits-prevent-drift`'s own Future Validation section names this as the project's
  next open test of its own central principle. A clean result (fires correctly on `product-010`,
  stays quiet on the rest of the corpus) would be the first real evidence the principle generalizes
  beyond exact-match checks; a noisy result would be equally valuable evidence of where it stops
  generalizing.
- **The real false-positive rate of a targeted phrase-list check for citation vacuity** — cheap to
  measure (run it against every `resolved_policies`/`not_applicable` entry across both real
  stories) and directly answers whether this narrow, mechanical first cut is sufficient or whether
  a broader grounding-comparison check (verifying a citation's text actually traces to the story,
  an ADR, or domain vocabulary — the same technique Entity/Event Continuity already validated) is
  needed as a second iteration.
- **Whether these two mechanisms, applied going forward, actually change what the Human-Insight
  Inventory would measure next time.** Both Human-Insight Inventory passes found uniform Accept
  across every category with real data — if these mechanisms fire on new stories the way they're
  predicted to, the next natural measurement is whether a human presented with a genuine `GAP`-tier
  finding (as opposed to a silently-clean `gaps: []`) actually engages differently, or still accepts
  by default. That's a direct, cheap follow-on question this iteration sets up but does not answer.
- **Whether Mechanism A's enumeration scales past a short `out_of_scope` list.** Every real
  instance on file has 1–3 `out_of_scope` entries; whether the per-pair enumeration stays reliable
  as that list (or the scenario set) grows is untested and would be a direct, measurable output of
  running this against future, larger stories.

---

# Risks And Failure Modes

- **Mechanism A's core risk is exactly the one its own governing principle already flags as
  unresolved**: semantic judgment is inherently softer than lexical matching, and a model asked
  "does this scenario presuppose that excluded concern" could plausibly answer inconsistently
  across near-identical phrasings, or over-fire on loosely related concerns that aren't genuine
  contradictions (e.g., an `out_of_scope` item naming "bulk import" while a scenario mentions
  "importing a single record" — related vocabulary, not a real contradiction). The enumeration
  discipline bounds *which* pairs get asked about; it does not by itself guarantee each individual
  answer is reliable.
- **Mechanism B's phrase list is deliberately narrow, which is both its safety margin and its
  weakness.** It will not catch a vacuous citation phrased without any of the listed markers (e.g.,
  a model could write "authorization is not a concern here" instead of "not mentioned") — a
  precision-favoring choice for a first iteration, at a known recall cost that dogfooding is
  expected to surface, not eliminate.
- **A genuine edge case exists where Mechanism B could produce a real false positive**: a
  legitimate `not_applicable` resolution that explicitly and correctly cites `out_of_scope` itself
  as its grounding is a positive citation, not a vacuous one, but might share surface language
  ("not... mentioned/covered") with the phrase list if worded carelessly. This needs checking
  against real examples before the check is trusted broadly, not assumed away.
- **Both mechanisms are validated so far against exactly two stories.** Every risk and every piece
  of "evidence that would prove usefulness" above draws from the same small, real corpus this
  entire investigation has used throughout (`product-010`, `manufacturer-001`, plus
  `product-001`–`008` for Mechanism A's false-positive check). Neither mechanism has been run
  against a story this project hasn't already seen — the honest position, consistent with this
  project's own standing discipline (Human-Insight Inventory rerun, the reduction document), is
  that "smallest useful" here means smallest to *try next*, not smallest that's already been shown
  to generalize.
- **Neither mechanism addresses C, D, F, G, or H, by design** — a story could pass both new checks
  cleanly and still exhibit any of the other taxonomy classes. This is the deliberate scope
  boundary the user set, not an oversight, but worth stating plainly so a future reader doesn't
  mistake "A and B are covered" for "Story Readiness is covered."
