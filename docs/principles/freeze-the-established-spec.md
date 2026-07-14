---
title: "Freeze an Established Specification So Generation Cannot Silently Redefine Correctness"
status: draft

confidence: medium

maturity: emerging

themes:
  - system-design
  - test-driven-development
  - ai-assisted-code-generation

evidence_strength: medium

source_artifacts:
  - "commit 8cfabf7 — Add TDD loop, testing skills, validation constraints, and test strategy selection (2026-06-29 to 07-01)"
  - "CLAUDE.md's TDD Red-phase sanity-check section (later period, canopy-cli/src/fix_loop.rs)"
  - "docs/retrospectives/2026-06-29-to-07-01-reconstructed.md"

related_principles:
  - unresolved-decisions-become-explicit-decision-points
  - cross-artifact-consistency-audits-prevent-drift
cluster: "Protecting What's Already Decided"
---

# Principle

Once an artifact is established as the specification for a piece of behavior — a test file, an
accepted architectural decision, a resolved business rule — protect it structurally from being
edited or reinterpreted by a later automated step, rather than relying on an instruction telling the
model not to touch it.

# Problem That Revealed It

Introducing a Red/Green TDD loop (write a failing test, then make it pass) created a new risk that a
single-shot code generator never had: once a fix loop exists to make a failing test pass, that loop
could take the easier path of editing the test to match a broken implementation, instead of fixing
the implementation to match the test. Nothing about a TDD loop's basic mechanics prevents this by
default — "make the test pass" is satisfied either way.

# Evidence

- `8cfabf7`: the test file is explicitly protected from edits during the Green phase (a `skip_files`
  mechanism) — the fix loop can look at the test but cannot modify it while attempting to make the
  implementation satisfy it. This is a structural guarantee, not an instruction asking the model to
  leave the test alone.
- A later, independently-evolved instance of the same underlying concern (documented separately, in
  the project's own house style around TDD): a Red-phase sanity check that verifies a generated
  test actually fails for the *expected* reason (an intentional "not implemented" stub) before
  Green phase — and treats a test that passes cleanly on a stub as itself an error condition (the
  model over-delivered instead of producing a stub), routing it to a bounded fix attempt rather than
  proceeding as if nothing were wrong. Both mechanisms share the same shape: an established
  specification (the test, or the stub-only instruction) is checked for integrity before being
  trusted, not merely assumed correct because it exists.

# Counter-Evidence

The evidence base for this principle is thinner than the others in this set — it rests on one clear
originating instance (`8cfabf7`'s test-file freezing) plus one later, structurally similar but not
identical mechanism from a different period, rather than multiple independently-arrived-at instances
across unrelated parts of the system. It's plausible the Red-phase sanity check was a deliberate,
conscious extension of the test-freezing idea rather than an independent rediscovery of it — the
commit record doesn't establish which. This principle is marked "emerging," not "validated," because
of that gap: the pattern is real and evidenced, but the case for it being a broadly recurring,
independently-rediscovered shape (the way enumeration-over-holistic-review or compute-facts-
mechanically are) is not yet as strong.

# Applicability

- TDD or spec-then-implement loops in AI-assisted code generation, where an automated fix step could
  take the shortcut of altering the specification instead of the implementation
- Any pipeline stage where an earlier step's output is treated as ground truth for a later step,
  and the later step has edit access to files that include that ground truth
- Human-in-the-loop systems where a resolved decision or accepted artifact needs to survive
  unchanged through subsequent automated processing

# Confidence Assessment

Medium. The core mechanism (freezing a test file during the fix loop) is clearly evidenced and its
motivating risk is concrete and easy to verify independently (a fix loop with test-edit access will,
absent a guard, sometimes take the shortcut of editing the test). Confidence is capped at medium
rather than high because the evidence base is narrower than this project's other validated
principles — one clear instance plus one structurally-similar later mechanism, not several
independently-motivated recurrences across unrelated subsystems.

# Generalization

Likely applicable beyond this project wherever a "specification, then generated implementation,
then automated repair" loop exists — any CI/CD pipeline with an auto-fix step that has write access
to both tests and implementation code carries the same risk. The general shape (protect an
established source-of-truth artifact from the same automation that's meant to satisfy it) is a
known concern in software engineering generally (e.g., "don't let the test suite modify itself to
pass"), so the principle isn't a novel invention — but this evidence documents a concrete instance
of the risk actually being anticipated and guarded against in an AI-generation context specifically,
where the temptation for a model to take the shortcut may be higher than for a human engineer.

# Future Validation

More evidence is needed before this principle should be treated as validated rather than emerging: a
clearer test would be finding (or deliberately creating) additional, independent instances of "an
established artifact getting structurally protected from a later automated step" elsewhere in this
project's history, to see whether the pattern recurs the way enumeration and mechanical-fact-
computation demonstrably did. It would also help to directly test what happens without the
`skip_files` guard — does the fix loop measurably edit test files to pass, at some quantifiable
rate, or is this a risk that was anticipated and guarded against before ever being observed to
happen in practice? The current evidence doesn't distinguish "we saw this happen and fixed it" from
"we anticipated this could happen and guarded against it preemptively" — those are different
strengths of evidence.
