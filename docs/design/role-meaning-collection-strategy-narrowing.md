# Narrowing Role Meaning's Collection Strategy — Evidence-Based

Status: evidence narrowing only, extending `docs/design/operational-fact-collection-strategies.md`.
No UI, prompt, or workflow designed. Answers what evidence is still missing before deciding how
Role Meaning should be collected — not how to implement it.

Date: 2026-07-17

---

# Strategies Ruled Out

Restricted to strategies actually contradicted, undermined, or disfavored by direct evidence — not
merely weaker in theory:

- **Story-Derived Extraction (fully automatic, no human check).** Ruled out, not merely
  disfavored: this strategy's expected outcome is not uncertain — it reproduces the exact status
  quo gap the Role Semantics Investigation found (silent, unexamined role registration), and is the
  precise failure mode `unresolved-decisions-become-explicit-decision-points` already names
  directly. Choosing this strategy would not be an experiment; it would be a reversion to the
  already-diagnosed problem.
- **Correction of an Inferred Suggestion.** Ruled out by a direct, specific negative result, not an
  analogy: this exact mechanism (`init`'s opt-out bootstrap flow) was offered for the one real role
  this project has ever produced and was skipped 100% of the time. This is the literal strategy
  already tried and already failed once — evidence against it is not inferred from a related case,
  it is the case.
- **Review-Time Confirmation.** Ruled out on the same footing: the Human-Insight Inventory measured
  the exact review gate this strategy would route through and found zero differentiated scrutiny
  across every proposal in the one real session studied. The gate firing was directly observed to
  not produce meaningful review, independent of what it's reviewing.

**Not naturally applicable to Role Meaning at all, distinct from "ruled out"**: Discrepancy
Surfacing. It remains a genuinely viable strategy in general — it is the best-evidenced *approach*
for the vocabulary-mismatch fact type — but nothing in the evidence chain connects it to the Role
Meaning fact type specifically, since a role's meaning is not a discrepancy against an existing
vocabulary entry; there is nothing to compare it to. Excluded from further narrowing here on scope
grounds, not evidentiary grounds.

---

# Strategies Still Viable

## Direct Question

- **Strongest supporting evidence**: matches the exact fact shape (closed-set, fully quotable) the
  Role Meaning Value Experiment proved consumable — the entire injected fact reappeared verbatim in
  the resulting `evidence` field in 2 of 3 conditions.
- **Strongest counter-evidence**: no experiment in this entire chain has ever tested a live human
  answering this question. The one directly comparable real precedent (`init`'s optional prompt)
  is a *skippable* interaction and was skipped every time — suggestive, but not a clean test of
  Direct Question specifically, since it conflates "asked directly" with "answering was optional."
- **Remaining uncertainty**: whether a live human, asked this specific question this specific way,
  answers at a materially different rate than the one skippable precedent — genuinely unknown,
  since the two conditions (asked-and-skippable vs. asked-and-required) have never been isolated
  from each other.

## Forced Classification

- **Strongest supporting evidence**: the closest analogy to an already-validated fix in this exact
  codebase — `unresolved-decisions-become-explicit-decision-points`'s citation-requirement mechanism
  measurably changed a structurally similar non-engagement problem (Policy Discovery's fabrication,
  5/6 → 1–2/6) by pairing an escape hatch with an enforced cost, not by removing the escape hatch.
- **Strongest counter-evidence**: this is an analogy from a *different* fact category
  (business-policy resolution, which has a natural citable source) to Role Meaning (an identity
  classification, which has no external source to "cite" against in the same sense) — the
  mechanism that made the analogy's original case work (a checkable, named source requirement) may
  not transfer cleanly to a question with no equivalent source to check against.
- **Remaining uncertainty**: whether an enforced-cost shape improves engagement for *this* fact
  type specifically, or whether the analogy's mechanism doesn't actually apply here, in which case
  Forced Classification might perform no differently from Direct Question.

## ADR-Driven Declaration

- **Strongest supporting evidence**: the strongest direct evidence of any strategy considered in
  this whole comparison — this is the literal mechanism the Value Experiment used to achieve its one
  clean, measured result. Not an analogy; the same channel, the same consumption path, already
  observed to work.
- **Strongest counter-evidence**: the persona-policy facts in Phases 2–3 used the identical channel
  and mostly failed to leave a trace — direct proof that the channel being proven is not sufficient
  on its own; the fact placed in it still has to independently satisfy the other four
  consumability properties.
- **Remaining uncertainty**: whether `Role::Described` — the storage location actually built for
  this purpose in `canopy-core`, and structurally wired into at least one other real consumer
  (`stories_from_intent_prompt`'s role-reuse context) — would perform comparably if it were ever
  actually populated and exercised. This has never been tested at all; the ADR channel's proven
  status is not evidence against `Role::Described`, only evidence that it has never been needed to
  find out.

---

# Highest-Learning-Value Uncertainty

Not the strategy most likely to succeed — the uncertainty whose resolution would teach the most,
regardless of which way it comes out.

**The single highest-leverage remaining uncertainty is whether a live human, asked in any form,
answers a role-meaning question at a materially different rate than the one existing precedent
(0 of 1).** This is the one link in the entire causal chain that has never been tested at all:
every experiment in this chain — the Role Meaning Value Experiment, both phases of the Human
Insight Process Experiment — used pre-authored, programmatically-injected facts. The
*consumption* half of the chain (does the pipeline do something with a supplied fact) is well
evidenced. The *elicitation* half (does a human supply one in the first place) has exactly one data
point, and it's a zero.

This uncertainty is highest-leverage specifically because it gates every other open question in
this narrowing: if a live human does not answer reliably regardless of how the question is framed,
the entire comparison between Direct Question and Forced Classification collapses to "neither
works," and the storage-channel question (ADR vs. `Role::Described`) becomes moot — there would be
nothing to store either way. Resolving this first is not merely convenient; every downstream
question in this narrowing is conditional on it.

---

# Minimal Discriminating Experiment

**Design, described without any UI/prompt/workflow specification**: the next time a real dogfooding
session genuinely introduces a new role (not manufactured for this purpose, consistent with
`structure-emerges-from-behavior`'s own standing discipline in this chain), present the
role-meaning question live, to a real person, in one of two conditions — **skippable** (Direct
Question) or **non-skippable-but-honestly-deferrable** (Forced Classification, where "unresolved"
remains a genuine, low-cost option, not an eliminated one). Whichever condition is used, record the
resulting answer through **both** existing candidate channels simultaneously: as a real ADR (the
proven channel) and as a populated `Role::Described` entry (the unproven-but-built channel) — not
as competing alternatives to choose between now, but as two measurements riding on the same single
real event.

This single event resolves three distinct questions at once, without requiring three separate
experiments:

1. **Direct Question vs. Forced Classification**: whether the question was answered at all, and
   whether "unresolved" was chosen honestly rather than the question being silently ignored,
   directly comparable against the two conditions and against the existing 0-of-1 baseline.
2. **ADR-Driven Declaration's necessity**: whether `authorization` (or another checklist area) can
   still resolve citably from the ADR-recorded copy, replicating the Value Experiment's own result
   under live, human-sourced conditions rather than pre-authored ones.
3. **`Role::Described`'s viability as an alternative channel**: whether a *second* real story that
   later reuses the same role actually shows the populated description reaching
   `stories_from_intent_prompt`'s own role-reuse context — the one real, already-existing consumer
   for that storage location that has never been exercised end-to-end.

---

# Expected Outcomes And What Each Outcome Would Mean

- **The question goes unanswered (silently skipped, or "unresolved" never chosen) regardless of
  condition.** The most consequential possible outcome: it would mean the entire family of
  ask-a-human-directly strategies is weak in this pipeline regardless of framing, upending an
  assumption both surviving strategies share, and would argue for revisiting the design space
  itself rather than choosing between the two remaining elicitation shapes.
- **Forced Classification produces materially more answers (or more honest deferrals) than Direct
  Question.** Confirms the Policy Discovery analogy transfers to this fact type — the enforced-cost
  mechanism generalizes beyond business-policy resolution to identity classification, strengthening
  rather than merely extending an existing validated principle.
- **Forced Classification performs no better than Direct Question.** An equally informative,
  genuinely surprising negative result: it would mean the enforced-cost mechanism's specific
  benefit doesn't transfer to a fact type with no external, checkable source to cite against —
  suggesting whatever made the Value Experiment's facts succeed (concreteness) is doing the real
  work independent of how forcefully the question is asked.
- **The ADR-recorded copy resolves `authorization` citably, exactly as in the controlled
  experiment.** Confirms the proven channel holds up under live, human-sourced input, not just
  pre-authored facts — closing the one remaining gap in that mechanism's evidence.
- **The `Role::Described` copy is later read and rendered into a real second story's context,
  visibly.** Would show `Role::Described` is a genuinely viable second channel, not merely a
  plausible one — broadening rather than narrowing future channel choice, and reducing reliance on
  the ADR channel as the only proven path.
- **The `Role::Described` copy is populated but never actually reaches the second story's
  context.** Would confirm this is a wiring gap, not a fact-quality gap — directly distinguishing,
  per the wiring-vs-value distinction already established in this chain, "nothing downstream knows
  to use this yet" from "this kind of fact doesn't matter," and leaving the ADR channel as the only
  currently-reliable path until that wiring gap is separately addressed.
