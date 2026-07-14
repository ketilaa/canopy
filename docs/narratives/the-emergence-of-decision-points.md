---
title: "The Emergence of Decision Points"
status: draft
narrative_type:
  - planning-evolution
  - methodology-evolution

time_span:
  start_date: 2026-07-13
  end_date: 2026-07-14

related_principles:
  - unresolved-decisions-become-explicit-decision-points
  - deterministic-audits-vs-compensation

related_retrospectives:
  - 2026-07-13
  - 2026-07-14

related_blog_posts:
  - policy-discovery-vs-policy-invention

confidence: high
---

# Summary

Two separate mechanisms, built a day apart in two different parts of the pipeline, turn out to solve
the same underlying problem: a model asked to fill a gap in an incomplete specification will
silently pick an interpretation rather than stop and ask, unless the system makes "ask" a real,
structurally cheaper option than "guess." Decision Points names this concern as design doctrine
before any bug proved it necessary. Policy Discovery hits the exact predicted failure a day later,
in an unrelated part of the pipeline, and needs a second, independent fix to actually close it.

# Initial Vision

Before this thread, an unresolved business question in a specification — should this field be
unique, does this action need authorization — had no dedicated place to live. It might surface as an
`open_questions` list entry, informally, with no guarantee anything downstream would treat it as
still-unresolved rather than implicitly settled by whatever the model happened to generate next.

# Early Assumptions

The working design note (`e61043f`, 2026-07-13) states the risk directly, as a prediction rather
than an observed bug: "a small model asked to extract [requirements] will not stop and ask what an
unresolved question should mean, it will pick an interpretation, and that becomes a hidden business
decision with no record it was ever made." The response was architectural: a new Stage 2 (Decision
Extraction and Gating), inserted between Behavior Extraction and Clustering, elevating unresolved
questions to a first-class, gated `DecisionPoint` artifact. Contract generation (Stage 4) may not
proceed for behaviors blocked by an unresolved decision.

# Turning Points

The prediction was confirmed the next day, 2026-07-14, in a different mechanism entirely. Business
Policy Discovery — a business-rule checklist inside spec generation, upstream of the Stage 2
Decision Points machinery, asking six fixed questions (uniqueness, defaults, retention,
authorization, idempotency, consistency) — was inspected directly in a live-generated result. Five
of six questions had been marked "resolved," each with a specific, invented answer: a named
authorization role, a stated retention policy, a stated default value, none of them present anywhere
in the model's actual inputs. This is the Stage 2 design note's predicted failure, playing out
exactly as anticipated, just in a sibling mechanism rather than the one the prediction was written
for.

# Contradictory Evidence

The first attempted fix (`0fd89d7`) forced the policy checklist into a strict, per-item
classification — resolved, not applicable, or unresolved, six named entries, no freeform bucket.
This closed a related bug (the model had been dumping answers into an unlisted fourth bucket) but,
on its own, did not stop the fabrication. Having an explicit "unresolved" option available did not
change the model's behavior — evidence directly against the assumption that offering the option was
sufficient.

# Evolution of Understanding

The fix that actually worked (`c77f322`) added a requirement neither the original Stage 2 design nor
the first policy-checklist fix had included: every "resolved" (and, after a further review pass
found the same gap from a different angle, every "not applicable") classification must name its
exact source — the specific sentence in the story, ADR, or domain vocabulary that states the rule —
checked by code that fails the whole operation if the citation is missing. A controlled before/after
reproducibility comparison confirmed this changed actual behavior, not just the failure mode: before,
most runs resolved 5 of 6 questions with fabricated specifics; after, 1–2 of 6, with the remainder
correctly routed to an open question.

# Architecture Changes

- Stage 2 (Decision Extraction and Gating) added to the behavior-first pipeline, `2026-07-13`
  (`d9e8451`), gating Contract Generation on unresolved decisions.
- Business Policy Discovery's classification forced into a fixed six-item, three-bucket shape,
  `2026-07-14` (`0fd89d7`).
- Evidence-grounding requirement added to both "resolved" and "not applicable" classifications, with
  code-level citation enforcement, `2026-07-14` (`c77f322`).

# Principles That Emerged

`unresolved-decisions-become-explicit-decision-points` is the direct product of this thread. The
sharper, evidence-grounding half of the fix also feeds `deterministic-audits-vs-compensation` —
review caught an early version of the evidence-grounding code fabricating placeholder text instead
of failing loudly when a citation was missing, which had to be corrected to match the audit (not
compensation) shape this project treats as the safe pattern.

# Current View

Decision Points and Policy Discovery are two separate, sibling mechanisms — one gating Stage 4 on
unresolved behavior-level questions, one gating spec-level business rules earlier in the pipeline —
that turned out to guard against the same underlying failure mode from two different entry points.
Neither is fully sufficient alone: Decision Points names the concern architecturally; Policy
Discovery shows that naming the concern isn't enough without also making the "ask" path cost less
than the "guess" path.

# Why This Matters

This is a rare case where a design note's own predicted failure mode showed up, concretely, in an
unrelated part of the same system the very next day — and the first fix for it (an explicit
"unresolved" option) matched the letter of the original prediction's concern without actually
resolving it. The gap between "we said this could happen" and "we built something that actually
stops it" turned out to be a whole extra fix, not a formality.

# Open Questions

Whether Decision Points (Stage 2) itself, as originally designed, has the same evidence-grounding
gap Policy Discovery had — i.e., whether a resolved Decision Point can be marked resolved without a
citable source the way Policy Discovery's early version could — is not established by the evidence
reviewed for this narrative. This is a natural next check: the same fix that closed the gap in
Policy Discovery may or may not already be present in Decision Points' own resolution path.
