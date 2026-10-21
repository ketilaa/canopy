# Introducing the structured plan schema: a phased test plan for canopy-planner and canopy-implementer

Status: proposal, 2026-10-21. Nothing here is built. Companion to `structured-plan-schema.md` (the schema
itself). This document is the plan for testing it without redoing the mistake A1 just caught: comparing
Planner changes with one Implementer run per plan, where a difference of 3 of 10 is noise.

**Approval boundary.** Phases 0–1 are deterministic Rust (new types, pure audits, a renderer) with no
model calls and no change to `canopy-implementer`'s existing contract — they fit the "compute or verify"
tiers in `CLAUDE.md`'s escalation order and don't need a stop-and-ask before starting. Phase 2 is the
first point that spends compute and touches the model; it needs the maintainer's go-ahead to run, not
because the code is risky, but because it's the first real measurement and should be entered with eyes
open on cost (n=3 runs x n=10 plans x 2 arms x ~3 steps, roughly 180 Implementer step-calls). Phase 3 and
4 are each gated on the previous phase's result, not scheduled in advance.

## Why this order

The schema's value has two independent parts: (a) deterministic audits that catch contradictions and
leaks the moment a plan is written, regardless of who writes it, and (b) whether a model can draft good
`examples`/`throws`/`units` content. Part (a) can be fully tested with zero model calls, on fixtures built
from real historical failures — it's the cheapest, most certain part, so it goes first. Part (b) is the
open, expensive question (`docs/open-questions/planner-input-human-facts.md`, E1) and goes last, after (a)
is proven, so that a disappointing Phase 2 result can be read as "the model struggles with this content"
rather than "the audits don't work" — those are different findings and this ordering keeps them separable.

## Phase 0 — Audits as pure functions, tested against real historical failures

**Where:** a new module in `canopy-planner` (e.g. `structured.rs`): the types from the schema doc
(`Decision`, `Fact`, `Unit`, `Example`, `Throw`, `VerifyRule`) plus the six `verify` rules as pure
functions `fn check(&StructuredPlan) -> Vec<Issue>`, following the existing `Issue`/`verify()` shape in
`canopy-planner/src/verify.rs`.

**Tests, one per rule, each built from a real failure already in the record — not invented cases:**
- `no-duplicate-setup-conflicting-expect`: a fixture with S6a run 3's two tickets-repository checklist
  items (`save(T-1)` → `Optional.empty()` and → `Optional.of(...)`), expressed as two `examples` with the
  same `setup`. Must flag.
- `no-wall-clock-calls`: a fixture with S6a run 4's two `now()` calls. Must flag.
- `exceptions-rendered-only-for-owning-unit`: a fixture shaped like S6c's loans plan (a `Loan` unit with
  `owns_rules: []` but a `throws` entry borrowed from the service). Must flag; a clean fixture must not.
- `preserved-fields-not-contradicted-by-example`: a fixture shaped like S6a run 8 (`preserves: [issuedAt]`
  on `validate`, but an example whose `expect.issuedAt` differs from `setup`). Must flag.
- `every-identifier-in-an-example-is-defined-in-its-own-setup`: a fixture with S6a run 5's undefined
  `ticket` identifier. Must flag.
- `one-owner-per-rule-id`: a fixture shaped like the tickets service plan (the same `rule_id` claimed by
  both `ticket` and `ticket-service`). Must flag.

**Each rule also gets one clean fixture** (the corresponding passing plan from the record — e.g. S6a run 1
for the duplicate-setup check) to confirm it does not flag good input. This is the audit-vs-compensation
discipline applied to the audits themselves: prove they reject only what's actually wrong.

**Exit criteria:** `cargo test -p canopy-planner` green, 12 new tests (6 rules x violating + clean), each
test's fixture traceable in a comment to the run that motivated it. No model calls, no Implementer
involvement, no approval needed to start.

## Phase 1 — A renderer from the structured schema down to the existing `plan.yaml`, no Implementer change

**Why render down instead of changing `canopy-implementer`:** `Plan::from_yaml` (`canopy-implementer/src/plan.rs`)
is a stable, tested contract that the orchestrator, gates and hints all depend on. Changing it is a second,
separable risk this plan doesn't need to take yet. Instead, the structured schema is a richer *upstream*
representation that compiles down to today's flat `id / instruction / create / modify / read_only / covers`
shape — the same relationship `layered.rs::render_instruction` already has to the final plan text, just with
richer, auditable inputs.

**Where:** `canopy-planner/src/render.rs` (or a new `render_structured.rs`), a function
`fn to_legacy_plan(sp: &StructuredPlan) -> implementer_plan::Plan`:
- Steps come from `units` + `depends_on`, topologically ordered (deterministic; no model).
- Each step's instruction is rendered from that unit's `api`, its own `throws` only (never another
  unit's — this is where the S6c leak becomes structurally impossible rather than caught after the fact),
  its `examples` rendered as the existing checklist-sentence shape (reusing `render_instruction`'s current
  wording conventions so the Implementer-facing prompt text doesn't change shape, only its sourcing), and
  the relevant `decisions`/`non_goals` folded into the Rules prose.
- `create`/`read_only`/`covers` derived the same way `layered.rs` already does.

**Golden test:** hand-author `tasks/tickets/plan.structured.yaml` exactly as in the schema doc, render it,
and assert: (a) `Plan::from_yaml` accepts the output, (b) the `ticket-repository` step's instruction
contains zero `Throws` lines (regression test for the S6c leak, on a unit that has none), (c) the rendered
text for `ticket`'s step contains the three ordered `Throws` lines with their conditions, in order.

**Exit criteria:** the golden test passes; `cargo test -p canopy-implementer` stays green untouched (proves
no coupling was introduced); the rendered `plan.yaml` for tickets, run once through `run_step`, passes —
this is a sanity check on the renderer, not yet a measurement (n=1, hand-authored content, expected to
pass since it's the same content as the existing 10/10 hand plan, just re-expressed). No approval needed —
new code path, nothing wired into the default `score_planner.py` flow yet.

## Phase 2 — The actual experiment (E1 from `planner-input-human-facts.md`), measured correctly

**Change from the open-question doc's original design:** every arm runs `run_step --runs 3` (not 1) per
plan, following A1's finding that one run per plan cannot separate a difference of 3 of 10. Report the
mean and the per-plan spread, not a single pass count.

**Arms, all on tickets, n=10 plans each:**
- **A2 (no model):** the Phase 1 renderer fed the hand-authored `plan.structured.yaml` from the schema
  doc, run through the Implementer 3 times. This is a sanity ceiling, not really an "arm" — it should be
  very close to the existing hand plan's 10/10, because the content is the same. If it isn't, the
  renderer has a bug, not the schema.
- **A1 (model drafts `examples` and `throws`, human answers `decisions`):** a new example binary
  `run_structured` (mirroring `run_layered`'s shape in `canopy-planner/src/layered.rs`) that takes the
  fixed `units[].api` and the hand-answered `decisions` (already written in
  `planner-input-human-facts.md`'s gap list) and asks the model, per unit, for `examples` and `throws`
  only — schema-constrained, same generator interface `layered.rs` already uses. Rendered through Phase
  1's renderer, verified by Phase 0's audits (reject-and-retry-then-drop on a hit, the same shape the
  existing case audit already uses — an audit, not a rewrite), then run 3 times per plan.
- **A3 (A2 with the `decisions` rows removed):** tests whether the stated gaps matter at all, as a
  falsifiable control.

**Predictions (restated from the open question, with the n=3 correction; write the exact thresholds in
`canopy-implementer/docs/prompt-and-plan-changelog.md` before running, per project habit):**
- A2 mean pass rate at least 0.8 (8 of 10, averaged over 3 runs each). Falsified at 0.6 or below.
- A1 mean pass rate at least 0.7. Falsified at 0.5 or below.
- A3 mean pass rate at least 0.2 below A2.

**What it decides:**
- If A1 clears its falsifier, the human's job is confirmed to be the `decisions` + a review of the model's
  drafted `examples`/`throws` (via Phase 0's audits, which should catch most of what a human would catch
  by reading) — move to Phase 3.
- If A1 does not clear it but A2 does, the schema's audits and rendering are sound and the gap is the
  model's drafting, not the format — the next move is E2 (can the model even list the ambiguities,
  scored against the 7 gaps) rather than more schema work.
- If A2 itself does not clear its ceiling, the renderer or Phase 0 audits have a bug, and Phase 3 is
  blocked until that's found — this is why A2 exists as a sanity check, not a real arm.

**Needs approval:** yes, to spend the compute (roughly 180 Implementer step-calls) and because it's the
first point the model touches this schema.

## Phase 3 — Generality, only if Phase 2 clears A1's falsifier

Repeat Phase 2's A1 and A2 arms on stock and shipping (the other two stories with a clean hand-plan
reference: stock 10/10, shipping up to 10/10 with B5's wording). Same `--runs 3`, same audits unchanged —
per the project's "audit and trim, don't append" habit, no new `verify` rule gets added without a new
measured failure naming it; a second story's different failure shape is a reason to extend the rule set,
a vague feeling that it "might help" is not.

**Exit:** a one-paragraph generality statement (`one-story` / `two-story` / `three-story`) in the
changelog, same as every other S-series entry.

## Phase 4 — Deferred, not scheduled

Whether `canopy-implementer` should consume the structured schema directly (e.g. a gate that checks
`preserves` against the actual diff, rather than only against the Implementer's own tests; `owns_rules`
scoping `read_only` more precisely than today's per-step list) is explicitly out of scope until Phase 2/3
show the schema earns its keep on the Planner side. Building this speculatively would repeat the mistake
the course correction on 2026-10-17 was written to stop (`docs/design/implementer-first-course.md`):
tightening a schema before it's shown to move the thing that matters.

## Rollback

Every phase is additive: new modules, new example binaries, new fixtures. Nothing in Phase 0–2 modifies
`canopy-implementer/src/plan.rs`, the orchestrator, or any existing `layered.rs`/`draft.rs` path used by
the current `score_planner.py --layered` default. If Phase 2 or 3 falsify, the new modules are simply not
wired into the default path — no revert needed beyond not adopting them.

## What "done" means for this document

This plan is done being a plan once Phase 0 and Phase 1 are built and green, and Phase 2's predictions are
written into the changelog *before* the run — at which point it stops being a design doc and becomes an
S-series-style entry like everything else in `canopy-implementer/docs/prompt-and-plan-changelog.md`.
