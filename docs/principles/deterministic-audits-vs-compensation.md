---
title: "Deterministic Audits Are Safe; Silently Rewriting Model Output Is Not"
status: draft

confidence: high

maturity: validated

themes:
  - system-design
  - ai-assisted-verification
  - code-review-process

evidence_strength: high

source_artifacts:
  - "commit f0aaa74 — Add enumeration-over-holistic-review rule; distinguish audits from compensation"
  - "commit c77f322 — Require textual evidence before Policy Discovery may classify a policy resolved (review-caught fabrication fix)"
  - "commit a254b25 — Add Entity Continuity gate"
  - "commit 0dd7073 — Add Event Continuity gate"

related_principles:
  - compute-facts-mechanically
  - cross-artifact-consistency-audits-prevent-drift
cluster: "Compute, Don't Ask"
---

# Principle

Code that reacts to a model's output is safe when it only *compares* that output against an
independently-known fact and rejects the whole result on disagreement, leaving the model's actual
output untouched. The same code becomes unsafe the moment it *rewrites, silently corrects, or
fabricates content to fill a gap* in what the model produced. The distinguishing question for any
piece of code sitting between a model's output and its consumer: does this change what the model
said, or does it only decide whether to accept what the model said as-is?

# Problem That Revealed It

Early in this project's own internal guidance, "never let code compensate for a model's mistakes"
was written as an absolute rule — any code reacting to model output was treated as suspect. That
rule predated a class of checks (comparing a generated result against already-established project
facts, and failing the whole operation if they disagreed) that turned out to be some of the most
effective reliability fixes found. Read literally, the original rule risked flagging those checks
as violations of the very principle meant to protect quality — because it hadn't yet drawn the
line between *rejecting* bad output and *rewriting* it.

That line got tested directly, not just theoretically. A fix intended to stop a model from
inventing ungrounded business rules used code that sorted each of the model's classifications into
one of two output lists. In two specific branches — a classification with a stated conclusion but
no supporting citation — the code silently generated its own placeholder question to fill the gap,
rather than either passing through what the model actually wrote or rejecting the response
outright. An independent review caught this precisely: the code wasn't just checking the model's
output anymore, it was authoring new content and attributing it to the pipeline's understanding of
the model's intent. The fix was to make those two branches fail the entire operation instead of
generating substitute text — bringing the code back in line with the audit definition rather than
crossing into compensation.

# Evidence

- Commit `4fc8d28`: explicitly rewrote the governing rule after noticing the tension — "Rust-side
  compensation is forbidden. Deterministic audits are encouraged," with worked examples on each
  side: replacing an invalid value, rewriting generated content, or silently defaulting a missing
  field are compensation (forbidden); checking consistency between artifacts, verifying coverage,
  validating an already-known fact are audits (encouraged) — because "Entity/Event Continuity are
  exactly the audit shape and the old wording risked the reviewer flagging its own project's best
  pattern as a violation."
- Commit `98c1783` / `ea3e1b9`: two live examples of the audit shape working exactly as intended —
  each check compares a generated value against already-known project vocabulary and fails the
  whole operation on mismatch, never altering what the model produced, and each was verified via
  dedicated regression tests built from the exact failure case that motivated it.
- Commit `3241e8f`: a live example of the *compensation* failure mode being caught and corrected in
  the same codebase. Code meant to sort a model's own classifications into two lists was, in two
  specific branches, silently generating placeholder text the model never wrote when a required
  citation was missing. Independent review flagged this directly as crossing from audit into
  compensation; the fix made those branches fail loudly (matching the Entity/Event Continuity
  shape) instead of fabricating substitute content.

# Counter-Evidence

No evidence has been found contradicting the distinction itself. The clearest tension in the
record is that the distinction is not always obvious to apply correctly on a first attempt — the
same engineer who articulated the rule wrote code that violated it (the fabricated-placeholder
case above) before an independent review caught it. This suggests the principle is sound but not
self-enforcing: it requires either deliberate review specifically checking for this distinction, or
an explicit test (does this code ever produce text/values the model didn't itself produce?) applied
systematically, rather than being something a single author reliably self-audits by intuition
alone.

# Applicability

- Any post-generation validation layer sitting between a model and its output's consumer
- Code review checklists and automated review tooling for AI-generated content pipelines
- Deciding whether a proposed "fix" for a model reliability problem belongs in the prompt or in
  surrounding code

# Confidence Assessment

High. The distinction is grounded in a real, observed failure of its own absence (a fabrication bug
that slipped past the original author and was caught only by independent review) and in multiple
positive examples of the "audit" side working reliably in production use (Entity Continuity, Event
Continuity, each with dedicated regression coverage). The principle's own history — being refined
specifically because its first formulation was too broad — is itself evidence it was tested against
a real edge case rather than adopted as an untested assumption.

# Generalization

This generalizes to essentially any software system that places code between a model's output and
where that output gets used or persisted. The underlying design question — should code correct a
model's mistake, or only detect and reject it — comes up in any AI-assisted pipeline with a
verification layer: content moderation systems, automated code review, data extraction pipelines,
agentic tool-calling loops. The "audit, don't compensate" framing gives that design question a
crisp, checkable test rather than leaving it as a matter of taste: does this code ever produce
content the model didn't produce? If yes, it's compensation, and the fix belongs in the prompt or
in a human-reviewed gate, not in silent code.

# Future Validation

Because the one weakness identified is that the distinction isn't self-enforcing, a natural next
step is testing whether a lightweight, explicit checklist question — "does this function ever
return, insert, or persist a string/value it did not receive directly from the model's own output?"
— reliably catches compensation-shaped code in review, across a larger sample of changes than the
single case observed so far. A second useful test: whether this distinction holds up equally well
when the "audit" is checking something fuzzier than exact-string or vocabulary-membership
comparisons — for instance, semantic consistency checks that themselves require some judgment to
evaluate — since the clean examples so far have all been comparisons against exact, discrete,
already-known values.
