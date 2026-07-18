# Correcting the First Learning Iteration: Testing Discovery, Not Detection

Status: correction to `docs/design/backlog-discovery-first-learning-iteration.md`. The prior
recommendation tested whether a passively-printed note happens to be followed by unrelated later
activity — a weak proxy for the actual hypothesis, and, on reflection, the exact same mistake this
investigation chain already flagged and corrected once before for a different mechanism (passive,
skippable elicitation reliably underperforming an explicit ask — the same lesson the Role Meaning
thread's Link 2 discussion already established). This document corrects it directly rather than
defending the original design.

Date: 2026-07-17

---

# What Was Wrong With the Passive Note

A printed console line, with no required response, tests "did something happen later, maybe" —
not "does surfacing lead to backlog discovery." It has no moment where the human's decision is
actually observed, and no negative control: if a story about the flagged concept eventually gets
written anyway, there's no way to tell whether the note caused it or whether it would have happened
regardless. This is the identical shape of weakness already identified for passive, skippable
elicitation elsewhere in this chain — I should have applied that lesson here without needing it
pointed out a second time.

---

# The Strongest Opportunity Category, Unchanged

Domain-Boundary / Vocabulary Discrepancy remains the right category to test this against — nothing
about the correction below changes that choice. It is still the best-evidenced, most replicated,
most mechanically-detectable concern category in the entire chain. What changes is only how the
detected discrepancy is surfaced, not which discrepancy is chosen.

---

# The Minimum Intervention That Actually Tests Discovery

Not a passive note — a single, explicit, bounded decision point at the moment the discrepancy is
detected, reusing an interaction pattern already present elsewhere in Canopy (the same
Accept/Reject-shaped confirm already used at other review points), asking one narrow question:
*does the human want this flagged term acknowledged as a candidate for a future story, or not.*

This is deliberately still small — it reuses an existing interaction shape rather than inventing a
new one, and it does **not** ask the human to fully articulate the new story on the spot (that
would conflate this test with a separate, heavier question about whether humans can author a story
in the moment). It only asks for a lightweight, explicit acknowledgment: yes, note this as worth
addressing later, or not now. The acknowledgment itself can be recorded minimally — a single
timestamped entry is enough; nothing about this requires a new stage or a blocking gate.

**Why this is the right size, not merely a smaller one**: it converts an inferred, unobservable
signal (something happened later, maybe because of the note) into a directly observed one (the
human's explicit choice, right now) — and it creates a genuine negative control for free.
Acknowledged and dismissed flags can be compared against each other for what happens next, which
the passive version could never do, since it had no equivalent decision moment to split on.

---

# What User Behavior Would Constitute Evidence of Discovery

Two distinct signals, and both matter — the first alone is not sufficient:

1. **The explicit acknowledgment itself** ("yes, note this") is direct evidence a human, when
   actually asked, recognizes the flagged term as worth pursuing — a real, in-the-moment data
   point, not an inference.
2. **Follow-through**: whether a *subsequent*, independently-initiated `canopy intent` call later
   produces a story whose own domain content addresses the flagged term — checkable directly
   against `stories.yaml`'s accumulated history, the same way `stories.yaml` has already been used
   as ground truth elsewhere in this chain.

**The comparison that actually tests the hypothesis, not just its precondition**: whether
follow-through occurs *more often* after an explicit acknowledgment than after a dismissal. If
acknowledged flags are followed by a real story materially more often than dismissed ones, that is
direct, causal-shaped evidence surfacing leads to backlog discovery. If follow-through happens at
roughly the same rate regardless of what the human chose, that would mean the flag isn't actually
driving anything — stories get written on their own timeline either way, and the mechanism's
value would need to be reassessed, not merely tuned.
