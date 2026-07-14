---
title: "Implementation Ownership Requires Full File-Scope Visibility"
status: draft

confidence: medium

maturity: emerging

themes:
  - ai-assisted-code-generation
  - system-design
  - contract-driven-implementation

evidence_strength: medium

source_artifacts:
  - "docs/design/contract-driven-implementation-experiment.md — Stage 1 Results (2026-07-14)"
  - "docs/design/contract-driven-implementation-experiment.md — Stage 2 Implementation and Results (2026-07-14)"
  - "docs/reports/manufacturer-001.md — Contract-Driven Implementation, Stage 1 and Stage 2 sessions"
  - "canopy-llm/examples/contract_driven_stage1_experiment.rs"
  - "canopy-llm/examples/contract_driven_stage2_experiment.rs"

related_principles:
  - compute-facts-mechanically
  - deterministic-audits-vs-compensation
  - structure-emerges-from-behavior

cluster: "Full-Scope Visibility"
---

# Principle

When a model is asked to generate or modify a file whose full, correct shape is really the union
of several independent authorization units (behaviors, contracts, requirements) — not just the
one unit currently being worked on — show it every unit that shares that file, not one at a time.
A model given only a partial view of a file's authorized scope reliably tries to "complete" what
it recognizes as a familiar, whole artifact shape (an entity class, a config file, an API
resource), inventing structure that belongs to units it was never shown — even when explicitly
told not to. Full visibility of the *combined* authorized scope, not a stronger instruction on
the same partial view, is what stops the invention.

# Problem That Revealed It

Testing whether Canopy's `Contract` type (id/kind/entity/member/mandatory/required_tests/
dependencies — see `docs/contract-readiness-assessment.md`) carries enough to drive
implementation, a single-contract trial gave a model exactly one validation contract
(`ManufacturerNameValidation`: one field, two behaviors, zero dependencies) plus the resolved
target file and the relevant tech-stack skill, and asked it to write that file. The prompt
explicitly instructed: "This file may eventually need to satisfy OTHER fields/contracts not shown
to you here — implement ONLY what THIS contract requires... do NOT invent unrelated fields,
methods, or class structure beyond the minimum." In 2 of 3 runs, the model added `@Entity`, `@Id`,
and `@GeneratedValue` anyway — JPA persistence-identity annotations belonging to a *different*
contract (`ManufacturerConstruction`) that was never shown to it. The instruction was present,
correctly worded, and correctly positioned; it was still overridden by what looked like a strong
training-driven default ("a JPA entity has an `@Entity`/`@Id`").

# Evidence

- **Stage 1** (`docs/design/contract-driven-implementation-experiment.md`, "Stage 1 Results"):
  single contract shown, 2 of 3 runs invented `@Entity`/`@Id`/`@GeneratedValue` with zero
  supporting contract data, despite an explicit scope-limiting instruction in the same prompt.
- **Stage 2** (same document, "Stage 2 Implementation and Results"): the *only* variable changed
  was showing all six contracts that share the same resolved file target
  (`Manufacturer.java` — five validation contracts plus the one construction contract that
  actually authorizes `id`/`createdAt`/`modifiedAt`), keeping every other input identical (still
  no story, no scenarios, no `entity_schema`, no ADRs, no OpenAPI, no exploratory tool access).
  Result: 3 of 3 runs produced exactly the eight authorized fields and nothing beyond them —
  `@Entity`/`@Id`/`@GeneratedValue` appeared in every run and were now *correct*, since a contract
  explicitly licensed them.
- The isolation is clean specifically because nothing else changed between the two stages —
  the same model, the same tech-stack skill, the same target file, the same withheld inputs. The
  before/after shift (2/3 failure → 3/3 clean) attaches to the one thing that did change.
- Corroborating negative control, in the same Stage 2 run: two *other* defects (a Bean-Validation
  triggering-mechanism gap, and `@GeneratedValue`-only id assignment never firing on plain
  construction) persisted at the same or worse rate with full visibility — exactly as predicted
  before running the experiment. This is useful confirmation that full-scope visibility fixes
  *ownership* specifically, not generation quality broadly; the other defects trace to a
  documented tech-stack-skill gap, unaffected by how many contracts the model can see.

# Counter-Evidence

This is a single entity, a single shared file, and a single story — real, clean, causally
isolated evidence, but from one case, not the "reproduced across several distinct problems"
standard this project's higher-confidence principles rest on (contrast
`compute-facts-mechanically`, rated `high` after four independent instances). No evidence yet on
whether the effect holds as the number of contracts sharing one file grows large — six here; a
file legitimately owned by twenty contracts might reintroduce a different failure mode (context
dilution, or the combined contract list itself becoming too large to reliably hold in view) that
this experiment's scale can't rule out. Rated `medium` accordingly, not `high`, until a second,
independent file/entity/story confirms the same shape.

# Applicability

- Contract-driven code generation, specifically: before generating any file, assemble every
  contract whose `resolve_implementation_target` output matches that file — never generate from
  one contract in isolation once more than one targets the same place.
- More generally, any generation task where an artifact's "correct" scope is the union of several
  independently-authored units, and giving the model only one risks it over-completing the rest
  from a strong training prior rather than leaving it genuinely incomplete.

# Confidence Assessment

Medium. The result itself is clean — a single-variable change producing a stark before/after
shift (2/3 → 3/3) — but it rests on one entity, one file, one story. This is exactly the
"evidence exists, generalization boundary untested" case this project's own evidence-grading
discipline treats as `medium`, not `high` — the next contract-driven trial against a different
entity or a file shared by a different number of contracts is what would move this to `high`, or
surface the boundary condition where it stops holding.

# Generalization

The same shape — "showing a partial slice of an artifact's true scope invites the model to
invent the rest" — is worth watching for wherever Canopy composes several independently-generated
units into one file or one document: OpenAPI-spec generation composing multiple endpoint
contracts, scaffold generation composing multiple services' responsibilities into one
`docker-compose.yml`, or any future generation step assembling from contract-like units this
project hasn't built yet. The general form of the fix is the same: assemble the full authorized
set *before* the generation call, rather than trying to constrain a partial view more tightly.
