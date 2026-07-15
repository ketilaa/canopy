---
title: "The Road to Contracts"
status: draft
narrative_type:
  - architecture-evolution
  - planning-evolution

time_span:
  start_date: 2026-06-19
  end_date: 2026-07-15

related_principles:
  - coverage-should-be-generated-not-discovered
  - exhaustive-enumeration-over-holistic-review
  - compute-facts-mechanically
  - implementation-ownership-requires-full-file-scope-visibility

related_retrospectives:
  - 2026-07-13
  - 2026-07-15

related_blog_posts:
  - two-of-three-runs-invented-a-field-we-never-asked-for

confidence: high
---

# Summary

The word "contract" meant something narrow and technical for most of this project's history — an
OpenAPI/HTTP snapshot, generated late, mostly for documentation. It's now the central artifact of
the entire planning pipeline: the thing every earlier stage exists to produce, and the boundary
past which implementation is meant to begin. That shift happened in one day, and the harder half of
the road — actually wiring implementation to depend on it — hasn't happened yet.

# Initial Vision

Early in the project's history, "contract" referred to `contract.yaml` — an OpenAPI/HTTP artifact
generated per story, capturing the API surface a service exposed. It was a downstream, largely
documentary output: useful for describing an interface, not a planning input anything else depended
on.

# Early Assumptions

The implicit assumption through most of the project's first three weeks was that architecture
decisions (ADRs) and file structure were the real planning boundary — what got built, and how it was
organized, was decided from architecture and tech-stack skills, not from a dedicated cross-cutting
"contract" concept. The OpenAPI contract was a byproduct of that structure, not a driver of it.

# Turning Points

The behavior-first pipeline redesign (2026-07-13, see "From Stories to Behaviors") needed a name for
its own final stage: a per-cluster artifact declaring exactly which behaviors a piece of code owns,
what it depends on, and what tests it requires. "Contract" was the natural word for this — but it
collided directly with the existing, narrower `contract.yaml` naming, "entrenched across ~85 call
sites" per the rename commit's own accounting.

The project chose to rename the old concept out of the way rather than rewrite the new design around
a different word: `a1121e4` renamed `contract.yaml`/`generate_story_contract` to
`openapi.yaml`/`generate_story_openapi` throughout canopy-storage, canopy-llm, and canopy-cli — "the
OAS artifact is the older, narrower concept and the new pipeline's contracts are its central
vocabulary." A pure rename, no behavior change, made specifically to free the word for what it was
about to mean instead.

# Contradictory Evidence

Stage 4's own implementation (`40bde93`, same day) shows the new meaning of "contract" is
considerably more structural than the old OpenAPI snapshot ever was: one contract per cluster,
mechanically derived — name, owned behaviors, and required tests (the owned behaviors' own
statements, verbatim) are pure derivations of the cluster, not a separate authoring step. Unit
contracts are fully mechanical. Integration contracts get a mechanical dependency baseline plus one
bounded review pass that can only add or remove dependencies, never touch owned behaviors or invent
a contract — live-verified to correctly catch a dependency (an `EventPublisher` link) the mechanical
baseline missed because the word "EventPublisher" never appears in a behavior statement like "a
ProductCreated event is published." This is a fundamentally different kind of artifact than an
OpenAPI snapshot: not documentation of an interface, but the actual unit of ownership and dependency
the rest of the pipeline is organized around.

# Evolution of Understanding

By the end of 2026-07-13, "Contract" had gone from a peripheral, documentary output to the pipeline's
central noun — the thing Stage 0 through Stage 3 exist to produce inputs for. The design doc states
this explicitly: "Story → Behaviors → Decisions → Clusters → Contracts... now runs end-to-end, gated
at every stage, fully traceable from behavior to contract without re-reading the original
specification artifacts." Traceability, not documentation, is now the artifact's job.

# Architecture Changes

- `a1121e4`: OpenAPI/HTTP contract renamed out of the way, freeing "Contract" as a term.
- `40bde93`: Contract Generation (the behavior-first pipeline's own Stage 4) implemented —
  `Contract`/`ContractSet`/`ContractCoverage`/`DependencyReview`/`ContractAudit` types, one
  contract per cluster/grouping, mechanical unit-contract derivation, reviewed integration-contract
  dependencies.
- The design doc's own "Role of tools during implementation" section reframes the coding model's
  future role around this artifact: "treat the coding model primarily as a contract-to-code
  translator" — retrieval, testing, and validation tools driven by what a contract names, not by
  open-ended model-directed exploration.
- **2026-07-15 — a separate, numbered investigation (its own Stages 1-4, not to be confused with
  the planning pipeline's Stage 4 above) answered whether this artifact actually holds up as an
  implementation boundary.** `dcf0326`/`ccce904`/`02a35ff`: `Contract`/`Behavior` gained
  `kind`/`entity`/`member`/`mandatory` — three small, mechanically-derived fields, each added only
  after concrete evidence named a real gap (`docs/contract-readiness-assessment.md`), not
  speculatively. `5a8a4b4`/`e368cb1`: a single-contract trial found the boundary alone wasn't
  enough (unauthorized field invention, 2 of 3 runs); showing every contract sharing a file fixed
  it (3 of 3 clean) — see [[implementation-ownership-requires-full-file-scope-visibility]] and the
  blog draft `two-of-three-runs-invented-a-field-we-never-asked-for`. `74d4aa7`: the same fix
  confirmed by real compilation and test execution, not just read. `1d0e3a4`: contract-driven file
  discovery landed in `canopy implement` itself — the first production code this whole
  investigation touched, gated behind `contracts.yaml`'s mechanical presence, with a provably
  unchanged fallback for every story that doesn't have one.

# Principles That Emerged

`coverage-should-be-generated-not-discovered` connects directly: `ContractCoverage` is a derived
view (behavior id → contract), not a post-hoc audit trying to discover whether coverage exists.
`exhaustive-enumeration-over-holistic-review` also applies to the dependency-review step, which
reviews a mechanically-generated baseline item by item rather than holistically re-deriving
dependencies from scratch.

# Current View

The planning half of this road is complete: contracts are generated, gated, audited, and fully
traceable back to the behaviors that produced them. The design doc is explicit that this "closes the
planning half of the redesign" — a deliberate scope boundary, not an oversight.

**Updated 2026-07-15.** The implementation half now has real, not just planned, evidence behind
it — and a more precise shape than the design doc originally sketched. Contracts can drive
implementation, but only once a model sees every contract that shares a file, not one at a time;
that finding is itself now a graded principle, not just an experimental result. Contract-driven
file *discovery* is live in production. Contract-driven file *content generation* is not — every
real test/implementation-writing call in `canopy implement` still uses the full story/spec/
scenario/ADR prompts regardless of whether contracts exist for a story, confirmed by grep against
the current codebase, not assumed. The Contract Composition Assessment
(`docs/design/contract-composition-assessment.md`) names this as the single most important
remaining unknown, ahead of composition or schema questions.

**Further updated 2026-07-15.** That single most important unknown is now answered: Stage 5's A/B
experiment ran contract-scoped generation directly against production's own real, unmodified
prompt on the same file — contract-scoped won decisively (0/3 vs. 3/3, real compile and test),
not just "capable of working in isolation" as Stages 1–3 had shown. Composition moved to the top
of the priority list as a direct result, and Stage 6 answered *its* most basic question too: the
mechanical dependency rule and multi-file plan generation work end to end against a real
(non-synthetic) cross-contract dependency, not just the hand-written `Widget` fixture every prior
test of it used. Content generation for *composed* (multi-file, dependency-linked) contracts, and
composition beyond one dependency edge, remain untested — see the updated Open Questions below.

# Why This Matters

This narrative is unusual among the others reviewed because its ending is genuinely still open, not
resolved-and-stable. The word "contract" had to be fought for (an 85-call-site rename) before the new
pipeline could even be built around it, which is a concrete, measurable cost of not choosing
vocabulary carefully the first time — "contract" meant something else for three weeks before it
could mean this.

# Open Questions

The design doc names its own unfinished business precisely: wiring `canopy implement` to actually
consume `contracts.yaml` instead of the current ADR/architecture-skill-driven planning; deciding how
a behavior's Decision Point dependency gets re-materialized once its blocking decision resolves
(nothing currently re-derives a blocked behavior automatically); and the migration path from today's
`plan.yaml` shape to this one. The project's own stated litmus test for whether this succeeds,
discussed separately from the commit record: if switching the target language or framework changes
the contract itself, planning concerns have leaked back into implementation.

**Updated 2026-07-15.** The first of those three — wiring `canopy implement` to consume
`contracts.yaml` — is resolved for file discovery: `generate_story_plan_from_contracts` does this
mechanically, live in production, verified against real data. The litmus test above has not yet
been run in the form originally proposed (no second language/framework target has been tried),
but a related, sharper version of it was: the contract schema itself was checked for
language-independence early in this investigation and found clean by construction (no
Spring-Boot-specific or React-specific vocabulary anywhere on `Contract`) — the tech-specific
translation lives entirely in skills, as designed. The Decision Point re-materialization question
and the `plan.yaml` migration path both remain exactly as open as before — neither was touched.

**New open questions, surfaced by the same investigation that closed the first:** whether
contract-driven *content* generation (not just discovery) actually improves on what
`canopy implement` already produces today, untested in either direction; composition across
multiple entities and real (non-empty) cross-contract dependencies, exercised so far only against
synthetic test fixtures, never a real story; and multi-service/route-layer composition
(frontend + backend together), which the current mechanical enumerator explicitly refuses rather
than guesses at. `docs/design/contract-composition-assessment.md` (2026-07-15) is the fuller
account of all three, with a proposed next experiment for the first.

**Updated 2026-07-15 (same day, later): the first two are now partially resolved.** Content
generation: Stage 5 answered this directly — contract-scoped generation beats production's real
prompt on a single-entity, no-dependency file (0/3 vs. 3/3). Composition: Stage 6 produced the
first real, non-synthetic cross-contract dependency edge (regenerating this story's domain-event
ADR surfaced two blockers plus two more bugs found while fixing them — see the Composition
Assessment §8) and confirmed `generate_story_plan_from_contracts` turns it into a correct 3-step,
dependency-aware plan. Both remain open in their *harder* forms: content generation for a
composed, multi-file, dependency-linked contract group is untested (Stage 5 only tested a single
isolated file); composition beyond one dependency edge — multiple entities in one story, deeper
dependency chains, multi-service/route-layer composition — is still exactly as untested as before.
The question moved from "can this work at all?" to "what happens as complexity increases?" for
both threads, not from open to closed.
