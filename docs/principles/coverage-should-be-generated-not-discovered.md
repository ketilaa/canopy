---
title: "Coverage Should Be Generated Deliberately, Not Discovered Accidentally"
status: draft

confidence: high

maturity: validated

themes:
  - specification-completeness
  - prompt-engineering
  - test-generation

evidence_strength: high

source_artifacts:
  - "commit 1b295df — Implement Stage 0 (Specification Completeness)"
  - "commit 7553d07 — Split spec generation into schema/policy and coverage-driven scenario calls"
  - "reproducibility sweep, manufacturer-001, 2026-07-14 (scenario-count-matches-checklist-size result)"

related_principles:
  - exhaustive-enumeration-over-holistic-review
  - compute-facts-mechanically
---

# Principle

When a system needs a generated artifact (test scenarios, validation cases, coverage
documentation) to cover a known set of requirements, compute that set of requirements first and
generate directly against it — rather than generating the artifact holistically and relying on a
separate audit step, after the fact, to discover what was missed.

# Problem That Revealed It

This project's first approach to specification quality was audit-shaped: generate a schema and its
test scenarios in one pass, then run a separate completeness check afterward to catch anything the
generation step missed. That audit step worked, once it was itself rebuilt around exhaustive
enumeration rather than holistic review — but it revealed something about the layer beneath it:
scenario generation itself was still holistic, and its output still varied run to run, in ways the
audit step would sometimes catch and sometimes not, depending on what happened to get audited how
thoroughly. Fixing the audit made gaps visible. It didn't stop gaps from being created in the first
place.

# Evidence

- Commit `1b295df`: the original completeness-audit stage, whose own initial version needed the
  enumeration fix described elsewhere (finding 4 of 9 real gaps holistically, 9 of 9 once rebuilt
  around an explicit checklist) — establishing that even the *audit* layer needed enumeration to be
  reliable, before the generation layer underneath it was addressed at all.
- Commit `7553d07`: the follow-on change that moved coverage decisions upstream of generation
  entirely. Instead of writing scenarios holistically and auditing them afterward, a mechanical,
  code-computed checklist (one item per field constraint, one for missing-mandatory-fields, one per
  resolved business rule) is now produced *before* any scenario is written, and the model's only job
  is to write exactly one scenario per already-enumerated item.
- A controlled reproducibility test (three runs, identical starting inputs, only sampling varying):
  the number of scenarios generated matched the size of the independently-computed coverage
  checklist exactly in every run — coverage that could be predicted and verified before generation
  ran, not just checked afterward.
- The dominant source of scenario-count and scenario-content variance shifted, measurably, from
  "the model's own judgment about what needs testing" (unpredictable, silent, only visible via a
  downstream audit) to "how many items the mechanically-computed checklist contains" (visible,
  computed, and verifiable before generation starts at all).

# Counter-Evidence

This principle depends on the requirement set being mechanically computable in the first place —
it is not evidence against the principle, but it does bound where it applies cleanly. The coverage
checklist in the confirmed case (field constraints, mandatory-field checks, resolved business
rules) was fully derivable from already-structured data (a schema, a list of resolved policies).
Nothing in the project's own work has yet tested this pattern against a requirement set that isn't
already structured enough to enumerate mechanically — for instance, coverage requirements that
themselves depend on a judgment call (which behaviors are "important enough" to test) rather than a
fixed schema. In that harder case, the "compute the requirement set first" step may not be
mechanically available, and the principle's applicability would need to be re-examined rather than
assumed to transfer directly.

# Applicability

- Test-scenario and validation-case generation from a structured schema or contract
- Documentation-coverage generation against a known API or feature surface
- Any generation task where the "correct" coverage is derivable from already-structured data before
  generation begins

# Confidence Assessment

High, within its demonstrated scope. The shift from audit-after-generation to compute-then-generate
produced a directly measurable result (exact numeric correspondence between a computed set and
generated output across repeated runs), and the underlying motivation — an audit step alone left
too much run-to-run variance even after the audit itself was made reliable — was observed directly
rather than assumed. Confidence is appropriately lower for cases where the requirement set isn't
already structured enough to compute mechanically, since that variant hasn't been tested.

# Generalization

Likely useful in any AI-assisted generation pipeline producing an artifact meant to cover a
requirement set that can be derived from existing structured data — API contract test generation,
schema-validation test suites, configuration-completeness checks. The general lesson is a
sequencing one: if you find yourself building an audit to catch a generation step's gaps, ask
whether the audit's own logic for "what should have been covered" could instead run *before*
generation and hand the model an explicit worklist, rather than running *after* generation and
hoping to catch what it missed. Where that's possible, moving the computation upstream removes a
whole class of variance rather than detecting it after the fact.

# Future Validation

The clearest next test is exactly the boundary flagged in the counter-evidence: does this pattern
hold, and how, for a coverage requirement that isn't fully derivable from structured data — where
determining "what needs to be covered" itself requires judgment rather than mechanical
computation. If a requirement set can only be partially computed mechanically, a hybrid version
(mechanically enumerate what can be derived, explicitly flag the remainder as requiring a separate
judgment pass) may be needed, and testing that variant would clarify whether "generate deliberately"
degrades gracefully or needs a genuinely different design once the requirement set stops being
cleanly structured.
