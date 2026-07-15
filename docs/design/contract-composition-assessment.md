# Contract Composition Assessment

Status: Assessment only — no implementation. Written 2026-07-15, after Stage 4 landed in
production. Every claim below is grounded in a specific file/line, a specific test result, or a
specific real artifact (`manufacturer-001`'s actual `contracts.yaml`), checked directly against
the current repository state while writing this, not recalled from memory of earlier stages.

The question this whole investigation opened with — "can contracts drive implementation?" — is
answered: yes, for the case tested. This document's job is to find the next question, with the
same discipline: ground every claim in what's actually there, distinguish "proven" from
"assumed," and recommend the smallest next experiment rather than a redesign.

---

## 1. The next architectural unknowns

Ranked by how directly the current evidence points at them, not by which sounds most interesting.

### 1. Does contract-driven generation of file *content* work, not just file *discovery*?

**This is the single most important open question, and it was not closed by Stage 4.** Verified
directly: `grep -rn "Contract\|Behavior" canopy-llm/src/prompts/step.rs canopy-llm/src/prompts/
fix.rs canopy-cli/src/commands/implement/execute.rs` returns **nothing**. Every real file-content
generation call (`execute_implementation_step`, `generate_unit_test_stub`,
`execute_implementation_stub`, `execute_implementation_with_test`, and their tool-using
variants — `canopy-llm/src/prompts/step.rs`) still takes `story: &UserStory, spec: &IntentSpec,
openapi_yaml: &str, step: &ImplementationStep` and builds its prompt from the **full**
`entity_schema` and **full** `scenarios` list, exactly as before Stage 4. `fix_file`
(`canopy-llm/src/prompts/fix.rs`) takes no story/spec/contract input at all — only file content,
errors, and skill text.

Stage 4 changed *which files exist and in what order* (`ImplementationStep.file`/`operation`/
`depends_on`/`description`). It did not change *what goes inside* any of those files. Stages 1-3's
own finding — that generation from a narrow, contract-scoped prompt produces correct, compiled,
tested code — was tested against a **hand-built experimental prompt**, never against production's
actual, much richer, full-context `step.rs` prompt. This means the honest state of knowledge is:
contract-scoped generation *can* work in isolation; whether it's *better than, equal to, or worse
than* what `canopy implement` already produces today is genuinely untested. This is the natural
Stage 5 (§4).

### 2. Composition across multiple entities and real, non-empty dependency edges

Confirmed directly: `grep -c "dependencies: \[\]" .canopy/stories/manufacturer-001/contracts.yaml`
→ **6 of 6** real contracts have an empty `dependencies` list. Every dependency-composition
mechanism this investigation built — `dependency_targets` in `contract_plan.rs`, the mechanical
"non-construction depends on construction for the same subject" rule in `contracts.rs` — has only
ever been exercised against **synthetic, hand-written test fixtures** (a fabricated `Widget`
entity in `contract_plan.rs`'s own unit tests). `manufacturer-001` has exactly one entity, and its
6 contracts all merge into one file, so there has never been a real case with two files that
genuinely depend on each other. Multi-entity, multi-file, real-dependency composition is the
single largest gap between "tested" and "assumed" in this whole investigation.

**Correction, 2026-07-15 (§8):** the specific "regenerate the ADR" path proposed below (§7 #1) to
close this gap for free was checked against the actual mechanical rules and found blocked by two
real defects, not merely untested — see §8.

### 3. Multi-service and route-layer (frontend/backend) composition

`generate_story_plan_from_contracts` explicitly refuses two cases by design, not by oversight
(`canopy-llm/src/prompts/contract_plan.rs`): more than one non-frontend service (no mechanical
entity-to-service ownership mapping exists on `Contract`), and any `HttpRequest`/`HttpResponse`
contract (ambiguous between a backend controller and a frontend api-client). `manufacturer-001`
has exactly one backend service and would hit both refusals if either condition arose — neither
has, because Stage 1's own extraction run produced **zero** scenario-derived behaviors at all
(`audit_behavior_coverage` flagged all 12 scenarios; the ADR-event-coverage audit flagged the
domain-event ADR too — both already documented in the Stage 1 write-up). This means the *entire*
category of scenario-derived contracts (persistence, orchestration, http-request, http-response,
error-translation) — everything except mechanical validation/construction/event-shape/
publication — has never actually appeared in a real contracts.yaml this investigation has touched.

### 4. Step ordering when `depends_on` actually matters

A subtler finding, worth naming precisely: `execute.rs` uses `step.depends_on` **only** to build
sibling context for the model (`build_sibling_section`, called from 3 sites in
`canopy-cli/src/commands/implement/execute.rs`) — never to reorder execution. Execution proceeds
in `plan.steps`' own array order. `contract_plan.rs` sorts steps by `(service_tier, layer_weight)`
only — it does not topologically sort by the `depends_on` edges it computes. For
`manufacturer-001` (one step, no dependencies) this is invisible. For a real multi-step,
multi-dependency contract-driven plan, correctness currently rests on `layer_weight`'s fixed
ordering (model → event → repository → infrastructure → service → route → middleware) happening
to agree with the actual dependency direction — true by construction for the common layered case,
but never asserted or tested as an invariant.

### 5. When does the single-backend-service / no-integration-contracts fallback actually get exercised?

Every fallback path `generate_story_plan_from_contracts` can take (`Err` on >1 backend service,
on an http-layer contract, on integration-scope contracts, on a missing tech-stack convention) is
unit-tested with synthetic data but has **never fired against a real story** — `manufacturer-001`
has always taken the success path. Whether the fallback-to-legacy-planner UX is actually smooth
in practice (does the human notice the "falling back" message, does the legacy planner then work
correctly for that same story) is unverified.

---

## 2. What exactly remains of the old planner — a concrete inventory

Grounded directly in `canopy-cli/src/commands/implement/{mod,plan,execute}.rs` and
`canopy-llm/src/prompts/{plan,step,fix,dependencies}.rs` as they exist right now.

| Responsibility | Owner today | Notes |
|---|---|---|
| Which files exist, `operation` (create/modify) | **Contracts**, when `contracts.yaml` exists | `generate_story_plan_from_contracts` — mechanical, zero LLM calls. Legacy `generate_story_plan` (LLM-driven) is the fallback. |
| Step `description` | **Contracts** | Mechanical kind→verb lookup (`describe_group`). |
| Step `depends_on` (declared) | **Contracts** | Mapped from `Contract.dependencies`. Not used for ordering (see §1.4) — only for context assembly. |
| Step *order* in the plan | **Neither, fully** | `layer_weight`/`frontend_tier` sort, shared by both planners (made `pub(crate)` in Stage 4 specifically so the contract-driven path could reuse it, not duplicate it). |
| Dependency proposal (which npm/Maven packages) | **Legacy planner, unconditionally** | `propose_dependencies` (`canopy-llm/src/prompts/dependencies.rs`) runs in `load_or_generate_plan` regardless of which path produced the file list — takes the resulting `plan.steps`, not contracts. |
| Dependency gate (human accept/reject, npm install) | **Legacy planner, unconditionally** | Same function, same code path either way. |
| Test-stub (Red phase) content | **Legacy — full story/spec/scenarios** | `generate_unit_test_stub`/`_with_tools` — no `Contract` input at all (confirmed by grep, §1.1). |
| Implementation (Green phase) content | **Legacy — full story/spec/scenarios** | `execute_implementation_with_test`/`_stub`/`_step` — same. |
| Fix loop (compile/test error repair) | **Legacy — skill + error text only** | `fix_file`/`fix_file_with_tools` — no story/spec, but no `Contract` either; already the least story-coupled of the three (noted in Stage 1's own consumption inventory). |
| Roots sibling-context assembly | **Neither — path-based, shared** | `build_sibling_section` reads `step.depends_on` (file paths) regardless of their origin. |
| Scaffolding check, OpenAPI generation, service-package detection | **Legacy, unconditionally** | All in `cmd_implement`, upstream of the plan-generation branch entirely — contracts don't touch any of this. |

**The boundary, stated plainly:** contracts currently own *planning* (what files, in what
declared relationship) for a story that has them. Everything about *what's actually written into
each file* — and everything upstream of planning (scaffolding, dependencies, OpenAPI) — still
belongs entirely to the pre-existing, story/spec/ADR-driven machinery, unconditionally, for every
story regardless of whether it has contracts.

---

## 3. Contract composition

**Where composition is proven, not assumed:** contracts sharing one resolved file target merging
into one step. This is real, tested against production data three separate ways — Stage 2's
hand-traced six-contract group, Stage 3's real compile of that same group, and Stage 4's
mechanical `generate_story_plan_from_contracts` producing the identical one-step result from the
real `contracts.yaml`. This specific composition shape (many contracts, one file, one entity) is
the best-supported claim in this entire document.

**Where composition has never been tested, named explicitly:**
- **Cross-file dependencies with real (non-empty) `Contract.dependencies`.** Every example above
  is synthetic. A real story where, say, a `ProductService` orchestration contract genuinely
  depends on a `ProductRepository` persistence contract in a *different* file has never been
  generated, planned, or executed.
- **Multiple entities in one story.** `manufacturer-001` has exactly one. Whether
  `generate_story_plan_from_contracts`'s single-backend-service assumption, its per-entity file
  grouping, and its dependency-target mapping all continue to behave correctly with two or more
  entities sharing a story is unverified in either direction — no evidence for or against.
  Nothing tested has been *shown* to fail with multiple entities; nothing has been *shown* to
  succeed either.
  - **Multi-file step ordering under real dependency edges** (§1.4) — the layer-weight sort has
  never been checked against an actual dependency graph that could disagree with it.
- **Integration-scope contracts, entirely.** Explicitly refused by `generate_story_plan_from_
  contracts` today (§1.3, §1.5) — no composition story exists for them at all, mechanical or
  otherwise.
- **A story spanning a frontend and backend service together.** `manufacturer-001` has both
  service types in `services.yaml`, but no `HttpRequest`/`HttpResponse` contract has ever been
  generated for it (§1.3), so the frontend side of this story has never gone through
  contract-driven discovery even once — it would always have hit the legacy planner regardless
  of Stage 4, because the required contracts don't exist for it.

---

## 4. The natural Stage 5

Per the explicit ask — not "replace everything," the smallest experiment that teaches the most,
same methodology as Stages 1-4 (design → implement small → verify against real data → decide).

**Proposal: Contract-Scoped Step Generation — A/B Against Production's Own Prompt**

**Scope.** Reuse the existing Stage 3 Maven harness and the existing real target
(`manufacturer-001`'s `Manufacturer.java`, the same six-contract group Stages 2-4 already used —
no new data needed). Generate the test and implementation for this file **two ways**: (a) by
calling the actual, real, unmodified `canopy-llm::step::step_prompt`/`unit_test_stub_prompt`
prompt-building functions as `canopy implement` calls them today (full `entity_schema`, full
`scenarios`, ADRs, tech skill) — not a hand-rolled substitute; (b) by the contract-scoped prompt
already validated in Stages 2-3 (the six contracts' own `required_tests`, nothing else). Compile
and run both through the same real `mvn clean test`, 3 runs each, same reproducibility standard
as every prior stage.

**Hypothesis.** Contract-scoped generation (b) matches or exceeds today's real production prompt
(a) on real compile-and-test pass rate, while using meaningfully less prompt content — and does
not regress the ownership-correctness property Stage 2 specifically fixed (worth checking whether
today's real, full-context prompt (a) already avoids over-invention because it has *more* context
than Stage 1's narrower probe ever did, not less — an open question this design doc has not
previously asked).

**Success criteria.**
1. (b)'s real pass rate is ≥ (a)'s, across 3 runs each, on the same target.
2. (b)'s prompt is measurably smaller (token/character count) than (a)'s for the same file.
3. Neither approach exhibits ownership violations (fields/methods with no corresponding contract
   or scenario) — checked for both, not assumed clean for either.

**Stop condition.** If (b) underperforms (a) — lower real pass rate, or new defects (a) doesn't
have — that is a direct, falsifying result: it would mean today's fuller context is pulling its
weight, and wiring contract-scoped generation into `execute.rs` for real would be premature. The
right response in that case is not to redesign contracts, but to treat "does narrower, contract-
scoped context generation actually help" as an open question requiring more evidence, and to
leave `step.rs`/`execute.rs` exactly as they are until it's answered.

This deliberately does **not** propose touching `execute.rs`/`step.rs` yet — it answers the
prerequisite question (is contract-scoped generation actually competitive with what's shipping
today) with the same standalone-experiment discipline Stages 1-3 used, before Stage 4's own
precedent (assessment → design → implement small, with fallback → verify) would apply to wiring
generation itself.

---

## 5. What would justify retiring the legacy planner

Not now — explicit, falsifiable criteria for "eventually":

1. **Contract-scoped generation must be shown to be at least as good as today's production
   prompt** (Stage 5 above), not merely "capable of working" in isolation — the distinction this
   document draws in §1.1 between "can work" and "is better than what already ships."
2. **Multi-entity, multi-file, real-dependency composition must be exercised at least once**
   (§1.2, §3) — today's zero-dependency, one-entity evidence base cannot support retiring
   anything that depends on multi-file correctness.
3. **The single-backend-service and no-integration-contract restrictions must be lifted or
   proven unnecessary** — either a mechanical entity→service ownership signal gets added (a
   `Contract` field, though per §6 this is exactly the kind of change that should wait for
   concrete failing evidence, not be pre-built speculatively) or every real story this project
   generates turns out to only ever have one backend service in practice (unverified either way).
4. **The fallback path must be exercised for real, not just unit-tested** — a real story that
   genuinely triggers each `Err` branch, confirming the human-facing fallback message and
   subsequent legacy-planner run both behave correctly end to end.
5. **Evidence must span more than one story/entity.** Every claim in this document, and every
   claim in Stages 1-4, rests on `manufacturer-001` alone. "Retiring" implies generalization; one
   data point does not support it regardless of how clean that one data point looks.

Until these hold, the legacy planner is not legacy in the retirement sense — it is the only
proven-general path, and the fallback contract-driven discovery depends on.

---

## 6. Has the contract schema reached stability?

**Approaching stability — not yet validated for broad use, and the evidence supports both halves
of that claim precisely.**

**Evidence for "approaching stability":** `Contract` changed exactly once with justification
(Option 2: `kind`/`entity`/`member`, driven by Q3's hand-traced litmus test finding real
ambiguity) and once more narrowly (`mandatory`, driven by a live, reproducibility-tested probe
that found a genuine, load-bearing gap — 3 runs, 3 different wrong outcomes). Since `mandatory`
landed, **four full stages of adversarial, real-data testing** (Stage 1's single-contract trial,
Stage 2's full-file-visibility trial, Stage 3's real compile/test, Stage 4's production wiring)
produced defects every time — but not one of them traced back to the schema. Stage 1's failures
were model variance and a skill gap. Stage 2's were the same skill gap plus a genuine
process/visibility finding (not a schema gap). Stage 3's was a harness scope limitation. Stage
4's were two Rust bugs in the new mechanical function itself, caught by review. Zero schema churn
across four stages that each actively tried to break something is a real, positive signal — this
project's own established pattern (`compute-facts-mechanically`, `deterministic-audits-vs-
compensation`) treats reproduced-across-multiple-independent-problems as the bar for confidence,
and the schema has now cleared an analogous bar for *stability specifically*.

**Evidence against declaring it "stable enough for wider use" yet:** every one of those four
stages tested the *same* story, the *same* entity, and (per §1.2/§3) the *same* zero-dependency,
single-file composition shape. The [[implementation-ownership-requires-full-file-scope-
visibility]] principle already rates its own evidence `medium`, explicitly for this reason. A
schema can be stable *for the one shape of problem it's been tested against* without being stable
for shapes it hasn't — multi-entity composition, integration contracts, route-layer contracts,
and per-file generation (this entire document's §1) are all real, named gaps where the schema
has not been exercised at all, not gaps where it's been exercised and passed.

**Conclusion:** stable enough to build Stage 5 on without expecting another schema change — the
evidence base for *that* claim is solid. Not yet stable enough to call "validated for general
use" — that requires the composition and multi-entity evidence §1-3 name as still missing.

---

## 7. Recommendation — ranked by learning value for the next month

1. **Composition experiment (§1.2/§3)** — highest learning value. This is the single largest gap
   between "tested" and "assumed" in the whole investigation. **Correction, §8: the "cheap to
   close" claim below was checked and found false as currently coded** — regenerating the ADR
   does not, by itself, produce the dependency edge this recommendation assumed. See §8 for what
   actually blocks it and what closing it now requires.
2. **Stage 5: contract-scoped generation A/B (§4)** — the second-highest value, because it
   resolves the central open question (§1.1) this document surfaced: does contract-driven
   discovery's success actually extend to content generation, or does that remain an open
   hypothesis? Both this and #1 are standalone experiments, not production changes — low risk,
   high information.
3. **TDD-loop integration** — not ranked separately; it's the natural continuation of #2, not a
   distinct workstream. Folding it in avoids treating "wire generation" and "wire the loop that
   calls generation" as two different pieces of work when they're the same question asked twice.
4. **Skill improvements** — deprioritized for now, deliberately. Both concrete gaps this
   investigation found (Bean Validation triggering, eager id assignment) are already fixed;
   there's no currently-pending, evidence-backed skill gap to work from. Skill work here has
   always been *reactive* to a specific experiment's finding, not a standing priority — the right
   trigger is #1 or #2 surfacing a new one, not scheduling skill work speculatively.
5. **Planner retirement work** — premature by the criteria in §5; none of the five conditions
   hold yet. Not a good use of the next month.
6. **Contract schema work** — lowest priority, and deliberately so per §6's own conclusion: zero
   evidence across four adversarial stages points at a missing fact. Speculative schema work now
   would repeat exactly the mistake this investigation's own house rule warns against — "only
   revisit the contract schema if a future experiment reveals a genuinely missing fact." None has.

---

## 8. Correction (2026-07-15): the ADR-fix composition path is blocked, not free

**Original assumption (§7, recommendation #1).** This document proposed regenerating
`manufacturer-001`'s domain-event ADR (`adr-006-domain-event-for-manufacturer-registration.yaml`)
with a topic clause as the cheapest way to close the composition gap — "cheap to close... from
data already in hand," expected to produce a real, non-empty dependency edge (Publication/
EventShape depending on Construction) with no new story or entity needed.

**The investigation that checked it.** Before running anything, per this project's own "verify the
mechanics before trusting a prediction" discipline (the same discipline Stage 5 applied to its
harness before trusting a pass/fail result), a research pass read the actual code paths the
prediction depends on — `mechanical_event_behaviors`/`parse_event_adr`
(`canopy-llm/src/prompts/behaviors.rs`), `resolve_implementation_target`/
`abstract_layer_for_kind` (`canopy-llm/src/skills/file_targets.rs`), and the mechanical
Construction-dependency rule (`canopy-llm/src/prompts/contracts.rs`) — rather than assuming the
prediction would hold because it sounded mechanically plausible.

**What was confirmed to work.** `parse_event_adr` (`behaviors.rs:415-421`) is a plain string split
on the literal substring `" on topic "`, requiring both halves non-empty and
`event_name.starts_with(entity)` (`behaviors.rs:433`). Changing ADR-006's `decision:` field to
`"ManufacturerRegistered on topic manufacturer.registered"` would parse cleanly and produce 3
`EventShape` behaviors (`eventId`, `occurredAt`, `{entity}Id`) plus 1 `Publication` behavior
(`behaviors.rs:435-460`). This half of the prediction holds.

**Blocker 1: no JVM file-target convention for the layers these behaviors resolve to.**
`abstract_layer_for_kind` (`file_targets.rs:28-37`) maps `EventShape → "event"` and
`Publication → "infrastructure"`. But `resolve_implementation_target`'s `TechFamily::Jvm` match
arm (`file_targets.rs:83-94`) only handles `"model"`, `"repository"`, `"service"`, `"route"` — no
arm exists for `"event"` or `"infrastructure"`, so resolution falls through to `_ => None`
(a pre-existing, already-disclosed gap: the comment there already notes `spring_boot_skill`
doesn't define event/infrastructure/middleware layout yet). Consequence, confirmed by reading
`generate_story_plan_from_contracts` (`contract_plan.rs:156-162`): a `None` file target doesn't
skip just the new contract — it makes the function return `Err` for the *entire* plan, falling
back to the legacy LLM planner for the whole story. Regenerating the ADR would not add one new
step to a contract-driven plan; it would silently disable contract-driven planning for
`manufacturer-001` altogether.

**Blocker 2: the mechanical dependency rule wouldn't fire even if blocker 1 were fixed.** The
Construction-dependency rule (`contracts.rs`, `mechanical_unit_contracts`) gives every
non-Construction contract a dependency edge to whichever contract shares its `subject` and has
`kind == Construction`. But `mechanical_event_behaviors` sets the Publication behavior's `subject`
to the literal string `"EventPublisher"` (`behaviors.rs:456`), and the EventShape behaviors'
`subject` to the event name (`"ManufacturerRegistered"`) — neither equals `"Manufacturer"`, the
subject `ManufacturerConstruction` (contract-006) actually has. The rule matches on exact subject
equality; nothing today maps an event/publication behavior's subject back to the entity it
concerns. Even with blocker 1 fixed, this ADR change would still produce contracts with
`dependencies: []`, the exact non-finding this whole experiment was meant to fix.

**Why the original path doesn't currently work, stated plainly.** The prediction conflated "the
ADR-parsing mechanism exists and would fire" (true) with "a resulting dependency edge to
Construction would form" (false) — two independent mechanical rules, only the first of which was
checked before this document made the "cheap to close" claim. Both blockers are pre-existing gaps
in shipped code (not introduced by this correction), simply never exercised because no story has
ever had an ADR in the required format until this document proposed creating one.

**Status of the recommendation now.** These two blockers are the concrete, evidence-backed
prerequisites this project's escalation order requires before a code fix (CLAUDE.md: "propose
gated code... only once tiers 1 and 2 are genuinely exhausted... a real compliance limitation, not
a missing-lookup or missing-instruction problem") — found by a real planned experiment hitting
them, not proposed speculatively. Next: add a JVM event/infrastructure file-target convention to
`resolve_implementation_target`, and align the Construction-dependency rule's subject-matching so
an event/publication behavior for entity `E` is recognized as depending on `E`'s own construction
contract — then re-run the ADR-fix experiment as originally designed.

**Addendum (2026-07-15, during the fix itself): two more real bugs surfaced, neither previously
named above.** Writing the fix and a regression test for it (rather than hand-verifying the
change) surfaced two further pre-existing defects, both silent until now for the same reason as
blockers 1–2: nothing had ever exercised a non-empty unit-contract dependency before.
1. `generate_story_plan_from_contracts` (`contract_plan.rs`) always passed `event_name: None` to
   `resolve_implementation_target`, unconditionally, regardless of contract kind — meaning the
   "event" layer could never resolve for *any* tech family, not just JVM, even before this
   document's two named blockers. Fixed by recovering the event's own name from `Contract.name`
   (stripping its fixed `"EventShape"` suffix), since `Contract` itself carries no separate
   `subject` field to read it from directly — this module's own doc comment had referenced a
   `Contract.subject` that no longer exists, a stale reference from an earlier schema shape.
2. The Construction-dependency rule's fix above still stored the wrong id: `other.id.clone()`
   copied the *cluster's* id (`UnitCluster.id`, its own separate id namespace) into
   `Contract.dependencies`, which documents itself as holding *contract* ids. A cluster id would
   never match anything `dependency_targets` (`contract_plan.rs`) looks up, silently. Fixed by
   assigning every cluster's contract id up front (before computing any dependency), keyed by
   cluster id, so a dependency can reference the real contract id a sibling cluster receives.

Both are now covered by regression tests (`canopy-llm/src/skills/file_targets.rs`'s
`jvm_events_directory_is_recognized_by_detect_layer`; `canopy-llm/src/prompts/contracts.rs`'s
`event_shape_and_publication_contracts_depend_on_construction_for_the_same_entity`, which is what
caught bug 2 immediately on the first run). Neither changes the status of `manufacturer-001`'s
existing 6 published contracts: both bugs only ever mattered for a non-empty dependency case,
which didn't exist until this fix introduced the first real one.

**Resolved, 2026-07-15: the ADR-fix experiment re-run, with all four fixes in place, produced
the predicted real dependency edge — see `docs/design/contract-driven-implementation-experiment.md`'s
"Stage 6 Results" section and `docs/reports/manufacturer-001.md`'s matching entry for the full
before/after diff and the resulting cross-file, dependency-aware plan.** The composition question
this document opened with has moved from "can composition work at all?" (unanswered until now) to
"what happens as dependency complexity increases?" (multi-entity, multiple dependency edges,
deeper dependency chains — all still untested against real, non-synthetic data).
