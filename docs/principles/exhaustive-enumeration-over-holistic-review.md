---
title: "Exhaustive Enumeration Outperforms Holistic Review for Coverage-Critical Tasks"
status: draft

confidence: high

maturity: validated

themes:
  - prompt-engineering
  - specification-completeness
  - ai-assisted-verification

evidence_strength: high

source_artifacts:
  - "commit 1b295df — Implement Stage 0 (Specification Completeness)"
  - "commit e61043f — Record Decision Points and the enumeration-over-holistic-review principle"
  - "commit f0aaa74 — Add enumeration-over-holistic-review rule; distinguish audits from compensation"
  - "commit 7553d07 — Split spec generation into schema/policy and coverage-driven scenario calls"
  - "commit 0fd89d7 — Force per-item policy classification instead of a freeform resolved/open split"
  - "reproducibility sweep, manufacturer-001, 2026-07-14 (scenario-count-matches-checklist-size result)"

related_principles:
  - coverage-should-be-generated-not-discovered
  - compute-facts-mechanically
cluster: "Enumeration Over Holistic Review"
---

# Principle

When a task requires checking or producing coverage over a bounded, enumerable set of items,
requiring the model to walk that set explicitly — one item at a time, with a required answer for
each — produces reliably more complete coverage than asking the model to review or generate
against the set holistically.

# Problem That Revealed It

The first version of a specification-completeness check asked a model to compare a data schema
against its behavioral test scenarios in a single holistic pass: "here is the schema, here are the
scenarios, report any gap." Against a specification with 9 known constraint-coverage requirements,
that pass found 4 — and the 5 it missed weren't a different category of gap it didn't understand.
One of the missed gaps was the exact same constraint type (a field-length limit) the model had
correctly caught on three other fields moments earlier in the same response. The failure wasn't
comprehension. It was that nothing about the holistic framing forced every item to actually be
visited.

# Evidence

- Commit `1b295df`: direct before/after result — "an initial holistic 'compare schema and
  scenarios in one pass' prompt found 4 of 9 real constraint gaps, silently missing the exact same
  constraint shape (max_length) on one field after correctly catching it on three others...
  Restructuring into three explicit, mechanically-enumerated checklists... found all 9 of 9
  constraint gaps on the re-run."
- Commit `e61043f`: the result was generalized into a named principle and used to predict where
  else it should apply — a later behavior-extraction stage, a later cluster-review stage, and
  future contract-to-test coverage verification — before those confirmations happened.
- Commit `7553d07`: independent reapplication to scenario generation itself. A mechanical,
  code-computed checklist (one item per field constraint, one for missing-mandatory-fields, one per
  resolved business rule) now precedes scenario writing; the model writes exactly one scenario per
  listed item rather than inventing coverage holistically.
- A controlled reproducibility test (three runs, identical starting inputs, only sampling varying):
  after the enumeration-based rewrite, the number of scenarios generated matched the size of the
  independently-computed checklist exactly in every run — a level of coverage predictability the
  pre-enumeration holistic version never showed.
- Commit `f0aaa74`: the pattern was formalized as a standing rule only after independently
  reproducing across four separate parts of the same system — "Stage 0 constraint coverage, Stage
  1 behavior extraction, dependency review, clustering review" — which is what distinguishes this
  from a single lucky fix.

# Counter-Evidence

The principle has one documented boundary case, not a counter-example against the principle
itself but a sharpening of its scope. Commit `0fd89d7` describes a policy-classification task that
was *already* fully enumerative — six named business-policy questions, each requiring a
classification into one of three named buckets — and the model still invented an unlisted fourth
output category for half the items in a live run. Enumerating the *input set* did not, by itself,
constrain the model's *output* to the sanctioned categories. The fix required a stricter output
schema, not additional enumeration. This shows enumeration solves "did every item get considered,"
not "did every answer land in a valid category" — a second, independent problem that needs its own
fix even after enumeration is correctly applied.

No evidence has been found of enumeration *failing* to improve a genuine coverage task; the only
counter-evidence found bounds what kind of failure enumeration actually fixes.

# Applicability

- Specification and requirements completeness checking
- Test-scenario and behavior generation from a structured schema
- Dependency and consistency review passes over a known artifact set
- Any AI-assisted audit of a bounded, listable set of items for completeness

# Confidence Assessment

High. The principle is supported by a controlled before/after comparison (same model, same
context, only the framing changed, moving from 4/9 to 9/9), a second independent confirmation via a
reproducibility sweep showing exact numeric correspondence between a computed checklist and
generated output, and reproduction across four structurally different parts of the same system
rather than a single instance. The one documented boundary case sharpens rather than weakens the
principle, since it identifies a distinct failure mode enumeration was never expected to solve.

# Generalization

Likely useful well beyond this project. The failure mode — a model correctly reasoning about the
items it attends to while silently skipping others under open-ended framing — is a property of how
these models process instructions and context, not of this project's specific domain. Any system
using a language model to check or generate coverage over a bounded artifact (API endpoints against
documented contracts, a codebase's tests against its own requirements, a form's fields against
their validation rules) is exposed to the same failure mode and should benefit from the same fix:
compute the bounded set mechanically wherever possible, and require an explicit per-item answer
rather than an open-ended review.

# Future Validation

An ablation varying only model scale (same task, same enumerated checklist, different model sizes
from the same family) would clarify whether this is primarily a smaller-model-specific reliability
fix or holds equally for larger frontier models under long, complex contexts. A second useful test:
how the pattern degrades as the enumerated set grows very large (hundreds of items) — at some point
the per-item checklist itself may become long enough to reintroduce the same "not every item gets
attended to" failure it was meant to fix, and finding that threshold would sharpen the principle's
practical limits.
