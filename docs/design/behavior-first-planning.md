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
Story → Behaviors → Clusters → Contracts/Files → Tests → Code
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
by surface keyword keeps that distinction intact into Stage 2.

`scope=integration` exists because some behaviors are properties of a workflow spanning
multiple responsibilities, not of any single unit — "invalid registration persists nothing"
can't be observed from inside a repository's own unit test, since the repository is never
called. Forcing every behavior into exactly one unit cluster would recreate the same category
of mistake this pipeline exists to eliminate, just relocated. Integration-scoped behaviors are
set aside for Stage 3's integration-test contracts instead of being clustered as units.

**Human gate.**

### Stage 2 — Mechanical Clustering

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
Stage 2 from the pipeline's single largest risk into a mechanical fold plus a bounded review.

**Output:** approved unit clusters and approved integration groupings.

### Stage 3 — Contract Generation

**Input:** approved clusters.

- Unit clusters → implementation contracts (one file/component each).
- Integration groupings → integration-test contracts.

Each contract carries: owned behaviors, dependencies, forbidden imports, implementation target.

**Structural constraint:** a contract may only contain behaviors from its own approved cluster.
This is what makes the guarantee real — it isn't a rule telling the model "don't test
validation in the repository," there is no path by which a validation behavior could ever reach
the repository contract, because it was clustered elsewhere in Stage 2, before the repository
contract was even generated.

## The recurring principle

The same insight surfaced at two different levels of this investigation:

> Reduce one large decision into many small decisions made when context is freshest.

First, at the level of implementation: today's failing step asked a model to generate a whole
file (`ProductRepository.ts`) and its test in one shot, reasoning freely over a full scenario
list to decide what applied. The fix was behaviors-first, files-derived — many small, explicit
units of work instead of one large one carrying implicit judgment.

Second, at the level of planning itself: the first draft of this pipeline still asked Stage 2 to
discover structure by reading a complete, untagged behavior list — the same shape of problem,
one level up. The fix was identical in kind: attach `scope`/`subject`/`kind` per behavior in
Stage 1, while each behavior's origin is still known, so Stage 2 has almost nothing left to
infer.

## What this replaces in the current implementation

- `scenario_coverage_note`, `boundary_rule`, `missing_field_exception_rule`,
  `mock_dependencies_rule` (`canopy-llm/src/prompts/step.rs`) — all exist to tell an
  execution-time model which of a shared scenario list applies to it. Under this pipeline, a
  file's contract only ever contains the behaviors already assigned to it; there is nothing left
  to filter.
- The DDD/event-orientation architecture skill (`canopy-llm/src/skills/architecture.rs`), as
  prose reinjected at every execution call — replaced by architecture-derived behaviors flowing
  through the identical Stage 1 → 2 → 3 pipeline as story-derived ones. "The system is
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

Current thinking: this is acceptable as a small, bounded Stage 3 review question — "given this
integration cluster, which existing unit contracts does it exercise?" — rather than something
Stage 2's mechanical pass needs to resolve. Not yet fully specified.

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

## Status and next steps

Not yet implemented. Before building this out: work through `behaviors.yaml`'s schema in
detail, specify the mechanical Stage 2 grouping algorithm precisely, and resolve the integration-
contract-dependency question above. Migration path from the current `plan.yaml` shape to this
one is also unresolved — out of scope for this document.
