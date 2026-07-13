# Behavior-First Planning Pipeline

Status: Proposed — not yet implemented. The existing `spec` → `scaffold` → `implement`
pipeline remains in place until this is built.

Date: 2026-07-13

## Context

Three live `canopy implement product-001` runs (against a dogfooding project, 2026-07-12/13)
all stopped at the same point: the Red-phase test for the repository layer re-tested the
model factory's own "missing name" validation — `createProduct(undefined as any, ...)` as test
setup, expecting `subject.saveProduct(invalid)` to reject. The factory throws before
`saveProduct` is ever reached, so the test is structurally unfixable; the fix loop correctly
gave up each time with "No fixable errors found."

Each run received a strictly stronger prompt fix than the last:

1. An abstract "don't borrow the other constraint's number or message" warning.
2. A role-framed instruction, a concrete WRONG/CORRECT example naming this exact scenario, and
   a `find_symbol`/`read_file` tool-lookup hint.
3. The general DDD architecture principle ("business invariants live in the aggregate, not the
   application service or route handler"), threaded into the test-generation prompt for the
   first time, correctly positioned immediately after the scenario list, with duplicate
   tech-stack phrasing removed so nothing competed with it.

All three produced the identical violation, with the same false `##CANOPY_DEVIATIONS##: None`
self-report. `find_symbol`/`read_file` were available at every call across all three runs
(including Green-phase implementation, which gained tool access during this same investigation)
and were never invoked once.

Three independently-confirmed failures of the same rule under increasing prompt strength is a
different kind of signal than "the wording needs one more iteration." The failure is structural,
not lexical.

## Root cause: the current pipeline is architecture-first

```
Story → ADRs → Files/components → Tests → Code
```

Given ADRs (Node/Express, DDD, event-driven), Canopy's tech-stack and architecture skills
already prescribe a fixed file breakdown (model → event → repository → infrastructure →
service → route → middleware → app → index) *before* anything is known about which specific
behaviors the story requires. The story's BDD scenarios — the actual specification of what must
happen — are then shown in full, undifferentiated, to every one of those files' test-generation
calls, with a bolted-on instruction telling the model which scenarios don't apply to it. The
model is asked, once per file, to correctly filter a shared scenario list using judgment. It got
this judgment wrong on the same file, three times, regardless of how the judgment aid was worded.

A structural fix removes the judgment call rather than improving the aid. If a file's test-gen
prompt is never given a scenario that isn't its job, there's no filtering decision left for the
model to get wrong.

## Decision: Specification → Behaviors → Files

```
Story → Behaviors → Decisions → Clusters → Contracts/Files → Tests → Code
```

Files are the *last* thing decided, derived from grouping atomic behaviors — not the first
thing decided, with behaviors retrofitted onto a pre-existing architectural template.

### Stage 0 — Specification Completeness

**Input:** entity schema, BDD scenarios, OpenAPI contract, ADR-derived requirements.
**Output:** completeness findings, unresolved gaps, blocking questions. **Human gate.**

Questions answered before anything else proceeds:
- Does every entity-schema constraint (`max_length`, `max_items`, ...) have at least one
  corresponding scenario?
- Does every scenario resolve to a clear, observable outcome?
- Are open questions resolved, or explicitly accepted as blocking?

No architecture vocabulary appears at this stage. This is validating the specification is
complete on its own terms, independent of how it will later be implemented.

### Stage 1 — Behavior Extraction

Decompose the (now-complete) specification into a flat list of atomic, independently-testable
behaviors. Each one is traceable to the scenario, constraint, or ADR requirement that produced
it. No file, layer, or component name appears anywhere in this list — only what must be true,
observably.

Every behavior additionally receives three tags, assigned while its provenance is still known
— this is the single mechanism the rest of the pipeline depends on:

| Tag | Meaning |
|---|---|
| `scope` | `unit` (verifiable in isolation) or `integration` (a property of the assembled system) |
| `subject` | the concept this behavior concerns (`ProductName`, `Product`, `ProductCreated`, `ProductRegistration`, `ProductEndpoint`) |
| `kind` | closed taxonomy: `validation \| construction \| persistence \| event-shape \| publication \| orchestration \| http-request \| http-response \| error-translation` |

Examples:

```
Behavior: Name longer than 200 characters is rejected.
Tags: scope=unit, subject=ProductName, kind=validation

Behavior: ProductCreated contains eventId.
Tags: scope=unit, subject=ProductCreated, kind=event-shape

Behavior: ProductCreated is published on product-events.
Tags: scope=unit, subject=EventPublisher, kind=publication

Behavior: Registering a product publishes ProductCreated.
Tags: scope=integration, subject=ProductRegistration, kind=orchestration

Behavior: Invalid registration persists nothing.
Tags: scope=integration, subject=ProductRegistration, kind=orchestration
```

`ProductCreated contains eventId` and `ProductCreated is published on product-events` both
mention the same event but land in different `(subject, kind)` pairs — one concerns the event's
own data shape, the other concerns infrastructure topic-routing. Tagging by concern rather than
by surface keyword keeps that distinction intact into Stage 3.

`scope=integration` exists because some behaviors are properties of a workflow spanning
multiple responsibilities, not of any single unit — "invalid registration persists nothing"
can't be observed from inside a repository's own unit test, since the repository is never
called. Forcing every behavior into exactly one unit cluster would recreate the same category
of mistake this pipeline exists to eliminate, just relocated. Integration-scoped behaviors are
set aside for Stage 4's integration-test contracts instead of being clustered as units.

**Human gate.**

**Hardened 2026-07-13, during Stage 3's live verification.** A live run surfaced a bug more
fundamental than a coverage gap: the model correctly generated http-response/error-translation
behaviors and correctly reasoned about a FAILURE scenario's implied persistence/publication
prevention, but wrote the `kind` value into the `scope` field for those entries (e.g.
`scope: http-response, kind: http-response` instead of `scope: integration, kind: http-response`).
Since `scope` only accepts `unit`/`integration`, every one of those malformed entries failed
per-item YAML validation and was silently dropped — not a reasoning failure, a serialization
failure: `Generated Behavior → Serialization Error → Parser Rejection → Coverage Loss`, rather
than `Generated Behavior → Missing Reasoning`. One scenario (`product-001-02`) lost all three of
its behaviors this way, with nothing recording that it had produced anything at all. Fixed three
ways:
1. **Prompt fix at the root cause** — an explicit WRONG/CORRECT rule added to
   `scenario_behavior_prompt`'s Rules section: "`scope` is ALWAYS exactly `unit` or
   `integration` — never the same value as `kind`."
2. **A new mechanical audit** (`audit_behavior_coverage`, mirroring Stage 0/2's own audits): does
   every scenario in the spec have at least one surviving behavior, or an entry in
   `gaps.blocked`? Anything with neither is flagged as a real coverage loss, not a legitimate
   empty outcome — this is the exact check that would have caught the bug above mechanically
   instead of requiring manual YAML inspection to notice. Saved to `behavior-audit.yaml`.
3. **A `derivation` tag** (`mechanical` | `inferred`) added to every `Behavior`, defaulting to
   `inferred` for schema compatibility with files saved before this field existed. Motivated
   directly by this investigation: the disappearance was isolated entirely to inferred
   behaviors, and having the tag up front would have narrowed the search space immediately
   instead of requiring source-by-source elimination.

Live-verified after the fix: all three of `product-001-02`'s behaviors now survive with correct
`scope=integration`, http-response/error-translation behaviors from all four scenarios now
appear, and `audit_behavior_coverage` reports zero findings. Also fixed in the same pass: a
schema-ambiguous mechanical statement — `categories` (`type: [string]`) with `max_length: 100`
was worded "Categories longer than 100 characters is rejected," ambiguous between "the collection"
and "each element." `is_collection_field` now detects array-typed fields from the schema's own
`type` string and rewords to "Each item in categories longer than 100 characters is rejected."

### Stage 2 — Decision Extraction and Gating

Added 2026-07-13, alongside Stage 0's checklist fix — the same session that showed Stage 0
needed forcing into smaller mechanical steps surfaced a related gap one level up: an unresolved
business question (`open_questions`, or something Stage 0 flagged as `unresolved_question`)
gets *recorded* today, but nothing stops behavior extraction or clustering from proceeding
around it. A small model asked to extract behaviors will not stop and ask what "duplicate
product names" should do — it will silently pick an interpretation, and that interpretation
becomes a hidden business decision baked into a behavior, then a cluster, then a contract, with
no record that a choice was ever made.

**New artifact: Decision Point.**

```
Decision ID: product-001-dec-001

Question: How should duplicate product names be handled?

Options:
  - Allow duplicates
  - Reject duplicates globally
  - Reject duplicates per manufacturer

Impacted Behaviors:
  - Product registration succeeds
  - Product registration fails
  - Product persistence rules

Impacted Contracts:
  - ProductRepository
  - RegisterProduct
  - POST /products

Status: Pending
Gate: Human Decision Required
```

**Rules:**
1. Every unresolved business question becomes a Decision Point — not a note left in
   `open_questions` that planning quietly works around.
2. Every Decision Point records the behaviors it affects.
3. Every affected behavior records the Decision Point it depends on.
4. Contract generation (Stage 4) may proceed only for fully resolved behaviors.
5. Implementation may not begin for any contract that depends on an unresolved Decision Point.

This creates a three-way distinction Stage 1 alone doesn't have: **known requirements**
(ordinary behaviors), **behavior candidates** (behaviors whose exact shape depends on an
unresolved question), and **pending decisions** (the questions themselves, tracked as
first-class, gated artifacts) — rather than letting a model treat all three as equally settled
facts.

**Heuristic for what becomes a Decision Point**, not just a note: if answering the question
would change a validation rule, a persistence rule, an API contract, an event contract, or a
test expectation, it's a Decision Point. Recurring categories worth watching for: duplicate/
uniqueness handling, default values, retention policies, event payload contents, ordering
guarantees, authorization rules, error message semantics, idempotency, consistency expectations.

**Resolution isn't limited to "answer it."** A Decision Point's gate can close by: resolving it
(a human picks an option), explicitly accepting a stated option as a temporary assumption
(tracked, not silently assumed), or leaving it open and blocking every behavior/contract that
depends on it. What's not acceptable is the fourth, implicit path — proceeding as if the
question never existed.

**Human gate** — this is the "Human Gating" step in the pipeline diagram above; it is Stage 2's
own gate, not a restatement of Stage 1's.

**Implemented 2026-07-13.** One `DecisionPoint` is created mechanically per
`spec.open_questions` entry (Rule 1). A bounded LLM call links every behavior Stage 1 marked
`blocked` to an existing Decision Point or a newly-surfaced one — restructured into two phases
within a single call after prompt review found the original one-item-at-a-time framing let the
model rephrase the same underlying question differently across items; Step 1 now has the model
enumerate the distinct set of new questions across the whole batch once, Step 2 assigns each
blocked item to a decision id or one of those refs. A second bounded LLM call classifies each
Decision Point (business/technical/behavioral-ambiguity) and proposes resolution options (Rule
2/3). Three mechanical audits catch: a blocked behavior with no linked decision, an open question
with no Decision Point, and a Decision Point with no dependent behaviors (a signal it may be
stale). Resolution is interactive — a human picks an option, marks it a considered decision or a
temporary assumption (`Resolved` vs `AcceptedAssumption`), or defers (stays `Pending`, blocking
Stage 3/4 per Rules 4/5). Live-verified against product-001's real `open_questions` entry:
mechanical creation, linking, and classification all produced correct output; the interactive
resolution step itself hasn't been exercised end-to-end by automated testing since it requires a
real terminal (same `dialoguer` constraint the existing ADR-gating flow already has), only by
manual/live use.

### Stage 3 — Mechanical Clustering

**Input:** behaviors with `scope`/`subject`/`kind` metadata.

1. Separate by `scope`.
2. Group unit behaviors by `(subject, kind)` — mechanically, in code, no LLM involved for this
   step. This is the baseline clustering.
3. LLM review of the mechanical baseline — not generation from scratch. Bounded questions:
   should two clusters merge, is anything mis-tagged, are responsibilities cohesive, does any
   cluster's dependency reach outside its layer.
4. **Human gate** on the reviewed clustering.

This is the stage most likely to fail if left as free-form LLM reasoning over a flat list —
"invent a grouping" is a materially harder and more novel task than "review a plausible
pre-computed grouping and flag what's wrong with it." Pushing the `(subject, kind)` judgment
into Stage 1, where it's a small decision made once per behavior with fresh context, converts
Stage 3 from the pipeline's single largest risk into a mechanical fold plus a bounded review.

**Output:** approved unit clusters and approved integration groupings.

**Implemented 2026-07-13.** `mechanical_cluster` groups unit behaviors by `(subject, kind)` and
integration behaviors by `subject` alone — the latter wasn't fully specified when this doc was
first written; the resolution is that an integration behavior's `kind` names which observable
effect it is (persistence, orchestration, http), not a separate grouping axis, so the workflow
named by `subject` is the natural integration-test boundary, and all of a workflow's integration
behaviors land in one grouping regardless of kind. `audit_clustering` mechanically checks every
behavior lands in exactly one cluster or grouping matching its own scope — same audit-after-
generation shape as Stage 0/1/2. `review_clustering` is the bounded LLM review: given the
mechanical baseline, flag (not fix) cohesion problems, cross-layer dependencies, and merge
candidates; findings are surfaced for a human to act on by editing `clusters.yaml` directly, not
auto-applied.

Live-verified against `product-001` (post Stage 1 hardening, above): mechanical clustering
produced exactly the expected shape — one cluster per validation-bearing field, one for
construction, one for event-shape, one for publication, and a single integration grouping
holding all 12 of `ProductRegistration`'s integration behaviors (persistence, orchestration,
http-response, and error-translation together) — confirming the "group integration by subject
alone" resolution above. The mechanical audit found zero issues. The LLM review ran successfully
end-to-end but its two findings were weak-to-wrong on inspection (flagging a plain "required
field" validation behavior as an unexplained "external dependency," and proposing to merge the
construction cluster with the event-shape cluster — two responsibilities the design explicitly
keeps separate, see Stage 1's `ProductCreated contains eventId` vs. `...is published on
product-events` note above). This is expected, not a defect: the review step is advisory,
gated by a human, exactly like Stage 0/2's findings — its value is surfacing candidates for a
human to judge, not being correct unassisted. One prompt tightening applied after observing
this: an explicit line that an empty `findings` list is a good, expected outcome, so the model
doesn't manufacture a plausible-sounding problem just to have something to report.

### Stage 4 — Contract Generation

**Input:** approved clusters, only for behaviors not blocked by an unresolved Decision Point
(Stage 2).

- Unit clusters → implementation contracts (one file/component each).
- Integration groupings → integration-test contracts.

Each contract carries: owned behaviors, dependencies, forbidden imports, implementation target.

**Structural constraint:** a contract may only contain behaviors from its own approved cluster.
This is what makes the guarantee real — it isn't a rule telling the model "don't test
validation in the repository," there is no path by which a validation behavior could ever reach
the repository contract, because it was clustered elsewhere in Stage 3, before the repository
contract was even generated.

## The recurring principle

The same insight surfaced at two different levels of this investigation:

> Reduce one large decision into many small decisions made when context is freshest.

First, at the level of implementation: today's failing step asked a model to generate a whole
file (`ProductRepository.ts`) and its test in one shot, reasoning freely over a full scenario
list to decide what applied. The fix was behaviors-first, files-derived — many small, explicit
units of work instead of one large one carrying implicit judgment.

Second, at the level of planning itself: the first draft of this pipeline still asked the
clustering stage to discover structure by reading a complete, untagged behavior list — the same
shape of problem, one level up. The fix was identical in kind: attach `scope`/`subject`/`kind`
per behavior in Stage 1, while each behavior's origin is still known, so clustering has almost
nothing left to infer.

Third, at the level of Stage 0 itself: live-verified 2026-07-13. An initial holistic
"review the schema and scenarios together, note what's missing" version of Stage 0 found 4 of 9
real constraint gaps against the dogfooding project's `product-001` schema — correctly catching
`manufacturer`, `model`, and `categories`' constraints, but silently missing the identical
constraint shape (`max_length`) on `name`. Not a conceptual failure — the model plainly
understood what a gap looked like, since it found the same shape elsewhere — a coverage
failure: nothing forced it to visit every field × constraint pair. Restructuring the same
prompt into three explicit, mechanically-enumerated checklists (one line per field-constraint
pair, one per scenario, one per open question), walked item-by-item rather than reasoned about
holistically, found 9 of 9 on the re-run with the same model and the same schema. This is the
same principle again, stated as its own reusable rule:

> Coverage-critical stages should operate through exhaustive enumeration rather than holistic
> review. Whenever a stage can be reformulated as "iterate through every already-identified item
> and verify one property" rather than "review the whole artifact and identify what's missing,"
> prefer the enumeration — omission risk in a holistic pass has no reliable pattern to guard
> against, while a checklist can only fail item-by-item, visibly.

Expect this to reappear at Stage 1 (behavior extraction — has every scenario/constraint/ADR
requirement produced at least one behavior?), Stage 3's review pass (has every behavior been
assigned to some cluster?), and later contract-to-test coverage verification (does every
behavior in a contract have a corresponding test?) — anywhere the question is "did we cover
everything," prefer enumerating the everything mechanically over asking the model to notice
gaps in an undifferentiated whole.

## What this replaces in the current implementation

- `scenario_coverage_note`, `boundary_rule`, `missing_field_exception_rule`,
  `mock_dependencies_rule` (`canopy-llm/src/prompts/step.rs`) — all exist to tell an
  execution-time model which of a shared scenario list applies to it. Under this pipeline, a
  file's contract only ever contains the behaviors already assigned to it; there is nothing left
  to filter.
- The DDD/event-orientation architecture skill (`canopy-llm/src/skills/architecture.rs`), as
  prose reinjected at every execution call — replaced by architecture-derived behaviors flowing
  through the identical Stage 1 → 2 → 3 → 4 pipeline as story-derived ones. "The system is
  event-driven" stops being a sentence the model has to internalize and becomes concrete
  behaviors ("`ProductCreated` contains `eventId`", "...is published on `product-events`")
  tagged and clustered the same way as anything else. There is no longer a separate mechanism
  for "teach the model DDD" versus "teach the model this story."
- Constraint-to-layer assignment stops being an open question — a constraint is a behavior
  (`kind=validation`), and its cluster is computed, not reasoned about.
- Shared/cross-cutting types (`EventId`, `Instant`) are not special-cased. If architecture-derived
  behaviors repeatedly require event identity or timestamps, clustering naturally produces those
  as their own reusable clusters; later stories reference the existing cluster instead of
  re-deriving one.

## Role of tools during implementation

Captured 2026-07-13 as a principle to work through when implementation begins — not yet
integrated into the stage design above.

The pipeline's overall direction is reducing how much architectural and planning reasoning is
delegated to the coding model at execution time — Stages 0–3 front-load that reasoning into
accepted, human-gated artifacts (behaviors, clusters, contracts) instead. If that direction
holds, tools become most valuable as **retrieval and verification mechanisms**, not reasoning
mechanisms:

> Use tools to provide facts. Use contracts and behaviors to provide decisions.

**Avoid relying on tool use for:** architecture design, behavior extraction, clustering,
contract generation, or deciding what to implement next. These stages are increasingly
deterministic transformations of already-accepted artifacts — giving a smaller model freedom to
explore or reinterpret them at that point is more likely to introduce drift than value. This
also applies to *how* execution proceeds: a model should execute a contract, not decide what
work exists next — the next action should come from the accepted contract graph, not from a
"figure out what to do" tool loop. Repeated search → open files → search again cycles tend to
generate noisy context and encourage drift; retrieval should be driven by the current contract,
not by model-directed exploration. Long-running open-ended think/tool/think/tool loops are
similarly at odds with the direction this pipeline is moving.

**Where tools remain genuinely valuable:**
1. **Context retrieval, contract-driven rather than exploration-driven.** Given a contract,
   supply exactly the files it names as dependencies — "here is the contract, here are the 4
   relevant files" rather than "search the repository and figure out what matters." The
   mechanism can still be tool-based; what changes is that the *query* comes from the contract,
   not from the model deciding what to look for.
2. **Test execution.** Implementation → run tests → collect failures → repair. Objective
   feedback from a real test run, not the model reasoning from assumptions about its own code.
3. **Compiler and lint feedback.** Deterministic signal for whether generated code actually
   integrates — the model shouldn't have to guess.
4. **OpenAPI/contract validation.** Where a contract or OpenAPI spec exists, use tools to verify
   compliance mechanically: OpenAPI conformance, contract behavior coverage, missing test
   coverage for a contract's declared behaviors.
5. **Contract-to-test verification (future capability).** Given a contract's behavior list,
   verify a test exists for each one — "is there a test for behavior A/B/C?" is a tractable,
   mechanical verification problem, and a much safer question than asking a model to judge
   whether its own implementation is complete.

**Overall framing to carry into implementation:** treat the coding model primarily as a
contract-to-code translator. Treat tools primarily as retrieval, compilation, testing,
validation, and verification — not as an exploration or reasoning aid. The more planning and
architectural reasoning is front-loaded into accepted behaviors and contracts, the less reason
there should be to depend on tool-assisted exploration during execution — consistent with what
the day's live runs already showed: `find_symbol`/`read_file` were available at every call
across all three failed runs and were never once invoked.

## Open design note: integration contract dependencies

Unit contracts are mechanically derivable: `(subject, kind) → contract`. Integration contracts
are not — `Registering a product publishes ProductCreated` has `subject=ProductRegistration`,
which doesn't literally match any unit contract's subject, yet the behavior depends on `Product`,
`ProductRepository`, `EventPublisher`, and `ProductCreated` all existing. There's no obvious
mechanical mapping from an integration subject to the unit contracts it exercises.

Current thinking: this is acceptable as a small, bounded Stage 4 review question — "given this
integration cluster, which existing unit contracts does it exercise?" — rather than something
Stage 3's mechanical pass needs to resolve. Not yet fully specified.

## Open design note: business policy discovery before scenario generation

Raised 2026-07-13, during Stage 2's live verification. Two different classes of upstream input
feed this pipeline, and today only one of them gets a real gate before scenario generation:

- **Domain facts** — field names, types, structural constraints. These already get first-class
  treatment: `mechanical_validation_behaviors`/`mechanical_construction_behaviors`
  (`canopy-llm/src/prompts/behaviors.rs`) derive behaviors straight from `entity_schema`,
  bypassing scenarios entirely.
- **Business policies** — uniqueness, defaults, retention, ordering, authorization, error
  semantics, idempotency, consistency (Stage 2's own heuristic list for what becomes a Decision
  Point). These get no equivalent gate. `story_spec_prompt` (`canopy-llm/src/prompts/spec.rs`)
  generates `entity_schema` and scenarios in one call, with scenarios explicitly required to be
  grounded in the schema — but nothing asks "what policies does this schema imply" before that
  call runs. A policy question either gets silently guessed, silently omitted, or accidentally
  encoded as a scenario detail, and only surfaces later as a byproduct of Stage 0 noticing a
  completeness gap or Stage 1 noticing a blocked behavior.

**Motivating example:** the system had enough information to know `Product` has `name`,
`manufacturer`, `model` long before Stage 0 or Stage 1 ever ran. At that point a human analyst
would naturally ask "what makes a product unique — are duplicates allowed, what's the
uniqueness scope?" — a policy question, not a behavioral or implementation one. Canopy instead
let scenario generation proceed without an answer, and the omission was only caught downstream,
as a Stage 0 gap, requiring a human to hand-patch `spec.yaml` after the fact.

**Why this isn't "invert Behaviors and Scenarios":** a resolved policy ("name+manufacturer+model
must be unique") doesn't itself define HTTP status, error semantics, or example flows — something
still has to operationalize the policy into an observable, testable consequence, and that's
scenarios' real job, not a redundant one. Scenarios and behaviors serve different purposes
(human-readable example vs. mechanically-processable unit); collapsing one into "derived from the
other" in either direction loses that distinction.

**Proposed shape — reuse the ADR gate's own pattern, not a new artifact hierarchy.** ADRs already
follow Discovery → Questions → Human Gate → Resolved ADRs → Scenario Generation
(`identify_architectural_questions` runs before `generate_story_spec`). Business policy is the
missing parallel track:

```
Business Policy Discovery → Decision Points → Human Gate → Resolved Policies → Scenario Generation
```

concretely: walk the drafted `entity_schema` against Stage 2's heuristic checklist right after
it's known, producing `DecisionPoint`s (the type Stage 2 already has — no new artifact needed)
for anything unresolved, gated before `story_spec_prompt`'s scenario-writing call runs. Scenario
generation would then receive resolved ADRs *and* resolved policies as grounding input, the same
way it already receives resolved ADRs today. Stage 0 and Stage 2 keep running afterward too, as a
safety net for anything that still slips through — consistent with the audit-after-generation
pattern used everywhere else in this pipeline, not a redundancy to design away.

**Status: not yet implemented, deliberately deferred.** Stage 3 and Stage 4 are still unbuilt and
unproven — opening this as a second front before the core behavior-first pipeline has run
end-to-end once would risk destabilizing both at the same time. Revisit once Stage 3/4 land.

## Evidence this is worth the restructuring cost

A direct probe (same model Canopy uses, `qwen2.5-coder:14b`, run outside Canopy against a
from-scratch DDD/hexagonal prompt for this same story) produced materially finer, more
behavior-aligned decomposition unprompted by Canopy's own machinery — separate `ProductId`/
`ProductName` value objects, a repository interface split from its implementation, and an
explicit per-file test task — versus Canopy's actual 9-step backend plan, which bundles the
whole entity + factory into one file and has no interface/implementation split at all. The
repository's lack of an interface boundary is precisely why an invalid `Product` was
constructible and passable to it in the first place. Finer, behavior-derived structure isn't
just a process preference — it independently removes the shape of object that let the recurring
bug exist.

Stage 0 itself, once actually built (see "Status" below), produced its own confirming data
point: the checklist-driven rewrite found 9 of 9 real constraint gaps against `product-001`'s
schema where the initial holistic version found 4 of 9 — see "The recurring principle" above
for the full comparison. The same shape of problem (ask a small model to hold a large context
and notice everything wrong with it, versus ask it to answer one narrow question at a time)
reproduced and was fixed the same way, one level below where the original bug was found.

## Status and next steps

**Stages 0-3 are implemented**, all in `canopy behaviors <story-id>`
(`canopy-cli/src/commands/behaviors.rs`): Specification Completeness
(`SpecificationCompleteness`/`CompletenessGap`/`GapKind`/`GapSeverity`), Behavior Extraction
(`Behavior`/`BehaviorList`/`BehaviorGaps`/`BehaviorAudit`), Decision Extraction and Gating
(`DecisionPoint`/`DecisionLog`/`DecisionAudit`), and Mechanical Clustering
(`ClusteringResult`/`UnitCluster`/`IntegrationGrouping`/`ClusterReview`/`ClusteringAudit`) — types
in `canopy-core`, prompts in `canopy-llm/src/prompts/behaviors.rs`, `decisions.rs`, and
`clustering.rs`. All four have been live-verified against `product-001`, each surfacing and
fixing a real coverage gap along the way (see "The recurring principle", Stage 1's hardening
note, and Stage 2/3's notes above). The command currently stops after Stage 3's gate — Stage 4
(Contracts) is not yet implemented.

Before building Stage 4: decide how a behavior's Decision Point dependency (Stage 2 Rule 3) is
represented in `behaviors.yaml` so Stage 4 can check it mechanically rather than re-deriving it,
wire Stage 4's implementation gate against Stage 2's `DecisionLog` (Rules 4/5), and resolve the
integration-contract-dependency question above (which unit contracts does an integration grouping
actually exercise). Migration path from the current `plan.yaml` shape to this one is also
unresolved — out of scope for this document.
