---
title: "From Stories to Behaviors: The Origin of Behavior-First Planning"
status: draft
narrative_type:
  - planning-evolution
  - architecture-evolution

time_span:
  start_date: 2026-06-23
  end_date: 2026-07-13

related_principles:
  - exhaustive-enumeration-over-holistic-review
  - structure-emerges-from-behavior

related_retrospectives:
  - 2026-06-23-to-06-25-reconstructed
  - 2026-07-12
  - 2026-07-13

related_blog_posts:
  - why-we-replaced-holistic-review-with-enumeration

confidence: high
---

# Summary

Three separate live runs, each with a strictly stronger prompt fix than the last, produced the
identical bug. Not a similar bug — the identical one, on the identical file, with the same false
self-report claiming no deviation. That's the moment the project concluded the problem wasn't
wording; it was that the whole pipeline decided a story's file structure before it knew what
behaviors the story actually required. The fix wasn't a fourth prompt attempt. It was a new pipeline.

# Initial Vision

By late June, Canopy's planning pipeline followed one path for every story: `Story → ADRs →
Files/components → Tests → Code`. Given a story and its accepted architecture decisions, tech-stack
and architecture skills prescribed a fixed file breakdown — model, event, repository,
infrastructure, service, route, middleware, app, index — before anything was known about which
specific behaviors that story actually required. The story's BDD scenarios were then shown, in full
and undifferentiated, to every one of those files' test-generation calls, each with an instruction
telling the model which scenarios didn't apply to it.

# Early Assumptions

The implicit assumption was that architecture and file structure were the right first cut, and that
a well-written per-file instruction — "here's the full scenario list, filter to what's relevant to
you" — was a reasonable way to keep each file focused on its own responsibility. If a file's
generated test came out wrong, the fix was assumed to be a clearer instruction: a sharper rule, a
concrete example, a relevant tool.

# Turning Points

Three live `canopy implement product-001` runs, 2026-07-12 into 07-13, all failed at the identical
point: the repository layer's Red-phase test re-tested the model factory's own "missing name"
validation as if it were the repository's responsibility — `createProduct(undefined as any, ...)`
as test setup, expecting `saveProduct(invalid)` to reject, when the factory itself throws before
`saveProduct` is ever reached. Structurally unfixable as written.

Each run received a strictly stronger fix than the last: (1) an abstract "don't borrow the other
constraint's message" warning; (2) a role-framed instruction with a concrete WRONG/CORRECT example
naming this exact scenario, plus tool-lookup access; (3) the general DDD principle ("business
invariants live in the aggregate, not the service or route") threaded into the prompt for the first
time, positioned immediately after the scenario list with competing text removed. All three
produced the identical violation. Tool access (`find_symbol`/`read_file`) was available at every
call, including Green-phase generation, and was never invoked once.

# Contradictory Evidence

The project's own design note states this plainly: "Three independently-confirmed failures of the
same rule under increasing prompt strength is a different kind of signal than 'the wording needs one
more iteration.' The failure is structural, not lexical." This directly contradicted the working
assumption that a bad generation was always a prompt-quality problem — no version of the instruction,
however strong, changed the outcome, because the instruction wasn't the actual point of failure.

# Evolution of Understanding

The root cause, as stated in `docs/design/behavior-first-planning.md`: "The model is asked, once per
file, to correctly filter a shared scenario list using judgment. It got this judgment wrong on the
same file, three times, regardless of how the judgment aid was worded. A structural fix removes the
judgment call rather than improving the aid. If a file's test-gen prompt is never given a scenario
that isn't its job, there's no filtering decision left for the model to get wrong." This is the same
"enumeration over holistic judgment" shift already independently found in Stage 0's own constraint
checklist (4 of 9 gaps found holistically, 9 of 9 once enumerated) — the design note cites that exact
comparison as precedent for applying the same fix one level higher in the pipeline.

# Architecture Changes

The pipeline was redesigned end to end: `Story → Behaviors → Decisions → Clusters →
Contracts/Files → Tests → Code`, replacing `Story → ADRs → Files/components → Tests → Code`. Five
new gated stages were implemented over the following day (2026-07-13): Specification Completeness
(does the spec have enough information to extract behaviors from), Behavior Extraction (atomic,
taggable behaviors derived mechanically where possible), Decision Extraction and Gating (unresolved
questions become first-class blocking artifacts), Mechanical Clustering (behaviors grouped by
subject/kind before any file is named), and Contract Generation (one contract per cluster, owning an
explicit, non-overlapping behavior list). A file's test-generation prompt now only ever sees the
behaviors its own contract owns — the filtering decision that failed three times no longer exists
for the model to get wrong, because it isn't asked to make it.

# Principles That Emerged

`exhaustive-enumeration-over-holistic-review` is this narrative's direct output — the pattern found
here (holistic scenario-filtering fails, itemized ownership doesn't) is the same shape as Stage 0's
own finding, applied one level higher. `structure-emerges-from-behavior` connects as well: file
structure now emerges from clustered behaviors rather than being decided upfront from architecture
alone.

# Current View

All five stages are implemented and have been live-verified against real dogfooding stories, closing
what the design doc calls "the planning half of the redesign." The pipeline is fully traceable from
behavior to contract without re-reading the original specification artifacts.

# Why This Matters

This is a clean instance of a fix that didn't stop at symptom level even though a working symptom-
level fix was actively being attempted (three times, with escalating effort). What made the
difference wasn't more effort on the existing approach — it was recognizing that three identical
failures under increasing prompt strength is itself evidence about the *shape* of the problem, not
just its stubbornness.

# Open Questions

The design doc itself flags what's left as out of scope for this narrative's period: wiring
`canopy implement` to actually consume the new `contracts.yaml` instead of the old ADR/architecture-
skill-driven planning (see "The Road to Contracts"), and how a behavior's Decision Point dependency
gets re-materialized once its blocking decision is later resolved — noted as an open gap, not yet
addressed as of this narrative's end date.
