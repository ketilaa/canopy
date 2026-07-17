# Final Recommendation — Resolving Link 2

Status: closes the Role Meaning investigation thread. One recommendation, not a survey. Draws
directly on `docs/design/minimal-experiment-audit.md`'s already-simplified design and
`docs/design/role-meaning-value-chain-evidence-map.md`'s conclusion that Link 2 (human provides
fact) is the sole dominant remaining uncertainty. No UI, prompt, or workflow designed.

Date: 2026-07-17

---

# The Recommendation

**The smallest real-world validation is a single-condition field observation: the next time a real
dogfooding session naturally introduces a new role, ask the role-meaning question live, to a real
person, in the forced-but-honestly-deferrable form only — no comparison arm, no second condition.
Record whatever answer results through both real storage channels (an ADR, and a populated
`Role::Described` entry) from that one event.**

This is not a new design — it is the simplification `docs/design/minimal-experiment-audit.md`
already arrived at once the original two-condition comparison was shown to be unresolvable by a
single occurrence. Nothing further needs to be added to it; the recommendation here is to actually
run it, not to refine it again.

Why the forced-but-deferrable condition specifically, restated briefly rather than re-argued: the
skippable condition already has a real, if imperfect, reference point — `init`'s existing optional
prompt, skipped 0 of 1 times. Spending the one available natural occurrence on the untested
condition extracts strictly more new information than re-observing the one already partially known.

---

# What Information It Produces

Exactly one thing, cleanly: **whether Link 2 can happen at all, once, under real conditions.**
Not a rate, not a comparison, not a generalizable answer — a single existence data point in a chain
that currently has zero. As a byproduct, riding on the same event without adding a second
condition, it also produces a first real-world reading on two already-partially-answered questions:
whether the Value Experiment's Link 4 result (ADR-channel consumption) holds up with organically-
elicited content instead of experimenter-authored content, and whether `Role::Described`'s
never-tested wiring into a later story's context actually fires.

---

# Why This Is the Highest-Leverage Next Learning Step

Every other link in the chain (storage, consumption, downstream change) already has at least one
positive result to build confidence on. Link 2 has none. A single real observation — succeed, defer
honestly, or fail — moves this specific link from *zero evidence in either direction* to *one data
point*, which is the largest relative gain in evidence available anywhere in the current chain: a
first observation always carries more information than an nth confirmation of something already
partially known. It also gates everything else: Links 3–5 cannot be exercised with real,
human-sourced content until Link 2 produces one instance to carry forward — every other possible
learning activity in this portfolio would still be conditional on this one resolving.

---

# Outcomes That Would Meaningfully Increase Confidence

- **The user provides a definite classification.** The first-ever positive existence proof that a
  real human, live, supplies this kind of fact — directly closes the specific gap this whole
  synthesis identified.
- **The user explicitly selects "unresolved."** Nearly as valuable as an answer: it shows genuine
  engagement with the mechanism (a deliberate choice, not silence) and validates that the deferral
  option reads as legitimate rather than as an obstacle — a distinct, positive signal in its own
  right.
- **The ADR-recorded copy resolves `authorization` citably, as in the controlled experiment.**
  Would confirm the Value Experiment's result generalizes beyond experimenter-authored phrasing,
  closing a gap this chain has already named honestly rather than assumed away.

# Outcomes That Would Meaningfully Decrease Confidence

- **The user ignores the question despite it being non-skippable.** The single most damaging
  possible result: it would extend the existing 0-of-1 non-engagement finding to a *second*,
  structurally stronger mechanism, suggesting live elicitation may not work in this pipeline
  regardless of framing — a result that would call the entire remaining strategy set into question,
  not just this one instance of it.
- **The user answers with free text instead of the offered classification.** A different, also
  consequential negative signal: it would reveal friction between what the pipeline consumes well
  (closed-set values, per the Value Experiment) and what a real, unconstrained person naturally
  produces — a failure mode this chain has never had direct evidence about, positive or negative,
  until this observation.
- **The fact is answered and stored correctly, but neither channel is consumed downstream.** Would
  not indict Link 2 itself, but would undercut the value of resolving it — confirming a wiring gap
  rather than a fact-quality gap, and meaning a positive Link 2 result alone would not yet justify
  treating Role Meaning as a working capability.

---

# Portfolio Framing

If only one more learning activity can be funded before deciding whether Role Meaning deserves to
become a real capability, it should be this one. It is the cheapest possible experiment remaining
(one real, already-anticipated dogfooding event, no manufactured scenario), it targets the single
link with the least existing evidence in the entire chain, and its result — in any of the outcomes
above — would change what the next decision should be, rather than merely adding another data point
to an already-supported claim.
