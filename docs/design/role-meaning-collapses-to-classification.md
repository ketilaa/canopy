# Role Meaning Has Collapsed Into a Classification Problem

Status: final conclusion update, following directly from
`docs/design/role-meaning-fact-shape-critique.md`'s finding that the rationale element carries no
positive evidence. Updates the investigation's standing conclusions to reflect the reduced fact
shape: **Role Identifier + Classification Value**, nothing else. No collection, implementation, or
UX discussed.

Date: 2026-07-17

---

# Reassessing the Surviving Uncertainty's Wording

The uncertainty should be restated, and this is not a cosmetic change. "Can a human provide a Role
Meaning fact" left the shape of that fact open — it could, in principle, have turned out to require
authored prose, a classification, or something else. The investigation chain has since closed that
question: every element with positive supporting evidence is classification-shaped
(the identifier and the value); the one element that might have required authorship (the rationale)
has been shown, on direct re-examination of the actual citation text, to carry no positive evidence
anywhere it was tested. Continuing to speak of "a Role Meaning fact" in the abstract would risk
quietly re-admitting an authored-content possibility the evidence no longer supports.

**The surviving uncertainty is now precisely: can a human provide a Role Meaning classification —
a selection from a bounded set, not composed content.** This also narrows what Link 2 is actually
asking. It was never truly a question about authorship in general; the only fact shape ever shown
to survive the pipeline was classification-shaped from the start. The reduction makes explicit what
was implicit, and in doing so removes one live possibility (that the missing piece might be
authored elaboration) rather than merely rewording an unchanged question.

---

# Comparative Evidential Support: Role Description vs. Role Meaning Classification

| | Role Description (`Role::Described`'s free-text field) | Role Meaning Classification (identifier + value) |
|---|---|---|
| Built and real today? | Yes — exists in `canopy-core`, reachable via `init`'s bootstrap flow | Not built as its own field; demonstrated via the ADR channel |
| Direct evidence of downstream consumption | **None.** `stories_from_intent_prompt`'s rendering is confirmed wired by code reading, but never confirmed to produce any measured effect on anything | **Yes** — the only controlled, causally-demonstrated result in this entire chain (Value Experiment, 2 of 3 conditions, verbatim in citations) |
| Real-world human engagement | **0 of 1** — the one real opportunity to supply one (`init`'s optional prompt) was skipped every time it has ever been offered | Untested with a live human in any instance — this is exactly Link 2, unresolved for either fact shape |
| Structurally analogous evidence from elsewhere in the chain | The `reason` fields inside the successful Role Meaning facts are free-text, open-ended content of the same basic shape a description would be — and were **never once quoted** in any citation, across every instance in two separate experiments | Not applicable — this row *is* the classification value, the element shown to work |
| Broader-shape analogy | Directly comparable to the persona-policy facts (Phases 2–3) — open-ended, discursive content — which repeatedly failed to leave a trace (`compliance`, twice, independently) | Directly comparable to the one fact shape shown to succeed everywhere it was cleanly tested |

The asymmetry is not close. Role Description has zero positive evidence of consumption and one
direct negative engagement data point. Role Meaning Classification has one clean positive
consumption result and an untested — not negative — engagement question.

---

# Does Any Argument for Role Description Survive?

Three candidate defenses considered, each tested against the evidence rather than assumed:

**"Free text is needed to capture nuance a fixed classification can't."** This has real, evidence-
grounded footing — the Role-Classification Stability Test directly proved a rigid classification
*can* miss real cases (supplier, auditor, franchise partner, contractor all failed the original
internal/external binary). But the chain's own response to that finding was not "fall back to free
text" — it was to **expand the classification set** (adding `affiliated` as a fourth value). The
evidence supports "classifications sometimes need more values," which is a narrower, different
claim that doesn't rescue open-ended description as a fact type. When this chain has actually
encountered classification insufficiency, its own remedy has been more classification, not less
structure.

**"Role descriptions serve human documentation value, independent of machine consumption."** This
is the one candidate defense with a genuine, honest gap behind it rather than direct counter-
evidence: **nothing in this entire investigation chain ever tested whether a human reading a role
description later finds it useful.** Every experiment measured machine consumption (citation in a
resolved policy, rendering into a downstream prompt) — never human comprehension or glossary value.
This is not a surviving argument *for* description; it is an acknowledged blind spot the evidence
simply does not speak to, in either direction.

**"The 0-of-1 engagement failure might be about skippability, not content shape — descriptions
might work fine if a human actually wrote one."** A fair point, already raised in the
classification-vs-authorship analysis, and it is not fully closed here either. But it only applies
to the *human-production* half of the picture. It does not touch the separate, content-shape-
specific evidence that free-text elaboration — once supplied, by anyone, engagement aside — goes
unconsumed: the `reason` fields were always present, always experimenter-supplied (no engagement
question at all), and never once quoted. That evidence is independent of whether a human would
choose to write one.

**No argument for Role Description survives with positive evidence behind it.** One candidate
(documentation value) survives only as an untested possibility, not a supported one.

---

# Final Verdict

**Yes — Role Meaning has effectively collapsed into a classification problem.** The investigation
chain, run end to end, never produced positive evidence for open-ended role description as a fact
type; it produced positive evidence for exactly one shape (a bounded classification tied to an
unambiguous role identifier) and a repeated, cross-experiment absence of evidence for everything
resembling free text alongside it. The remaining open question is narrower than where this
investigation started: not "what should a role fact contain," but "will a human make one bounded
selection when asked" — the same Link 2 uncertainty as before, now correctly scoped to the one
fact shape this chain has actually shown to matter.
