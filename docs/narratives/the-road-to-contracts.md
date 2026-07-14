---
title: "The Road to Contracts"
status: draft
narrative_type:
  - architecture-evolution
  - planning-evolution

time_span:
  start_date: 2026-06-19
  end_date: 2026-07-14

related_principles:
  - coverage-should-be-generated-not-discovered
  - exhaustive-enumeration-over-holistic-review

related_retrospectives:
  - 2026-07-13

related_blog_posts: []

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
a different word: `e12bef6` renamed `contract.yaml`/`generate_story_contract` to
`openapi.yaml`/`generate_story_openapi` throughout canopy-storage, canopy-llm, and canopy-cli — "the
OAS artifact is the older, narrower concept and the new pipeline's contracts are its central
vocabulary." A pure rename, no behavior change, made specifically to free the word for what it was
about to mean instead.

# Contradictory Evidence

Stage 4's own implementation (`a9ac692`, same day) shows the new meaning of "contract" is
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

- `e12bef6`: OpenAPI/HTTP contract renamed out of the way, freeing "Contract" as a term.
- `a9ac692`: Contract Generation (Stage 4) implemented — `Contract`/`ContractSet`/
  `ContractCoverage`/`DependencyReview`/`ContractAudit` types, one contract per cluster/grouping,
  mechanical unit-contract derivation, reviewed integration-contract dependencies.
- The design doc's own "Role of tools during implementation" section reframes the coding model's
  future role around this artifact: "treat the coding model primarily as a contract-to-code
  translator" — retrieval, testing, and validation tools driven by what a contract names, not by
  open-ended model-directed exploration.

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
`plan.yaml` shape to this one. None of these were in scope for the planning-side work completed so
far, and none have started as of this narrative's end date. The project's own stated litmus test for
whether this succeeds, discussed separately from the commit record: if switching the target language
or framework changes the contract itself, planning concerns have leaked back into implementation —
a test that can only be run once this remaining wiring exists.
