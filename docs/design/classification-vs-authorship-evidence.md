# Guided Classification vs. Open-Ended Authorship — What the Evidence Actually Shows

Status: evidence reassessment only, in response to a reframing of the surviving uncertainty. No
mechanism, UX, or implementation proposed. Tests a specific hypothesis — that humans provide
meaning more effectively through guided classification than through open-ended authorship —
against the completed chain, and draws a precise distinction the hypothesis's framing risks
collapsing.

Date: 2026-07-17

---

# The Distinction This Reassessment Has to Hold Onto

Before answering the three posed questions, one fact about the evidence base has to be stated
plainly, because it changes what can honestly be concluded: **no fact in this entire investigation
chain — classification-shaped or prose-shaped, successful or unsuccessful — was ever produced by a
live human.** Every fact tested (Role Meaning's `internal`/`external`/`affiliated`, all five
Phase 2/3 persona facts) was authored by the experimenter and injected programmatically. This
chain has extensive, repeated evidence about which *fact shapes* the pipeline consumes reliably.
It has almost no evidence about how real humans actually behave when asked to produce a fact of
either shape. The three questions below are answered against that limitation, not around it.

---

# Which Successful Outcomes Relied on Classification?

- **Role Meaning Value Experiment.** The cleanest, most direct case: a closed-set, four-value
  classification (`internal`/`external`/`affiliated`/`unresolved`), consumed citably in 2 of 3
  tested conditions, with the `evidence` field reproducing the classification's own text verbatim.
- **`risk_averse`'s fact** (Phase 2/3). Worth naming precisely rather than folding into the same
  bucket uncritically: this was not a closed-set classification in the enum sense — it was a single,
  concrete, quotable declarative sentence naming one checkable artifact ("the original order/
  purchase confirmation number, verified against our records"). It succeeded for the same
  underlying reason a classification succeeds (nothing to extract or interpret, the whole fact is
  the answer) without actually being a classification. This matters: the evidence supports
  **boundedness and concreteness**, not narrowly "classification" as a format — classification is
  the cleanest instance of that property in this chain, not the only one that worked.

# Which Unsuccessful Outcomes Relied on Prose Authorship?

- **`compliance`'s fact**, twice independently (Phase 2's original run and Phase 3's regeneration).
  Multi-clause discursive prose ("must be honored within a legally mandated minimum window
  regardless of the item's condition, using the original payment method for any refund") with no
  single nameable artifact — left no trace in either regenerated spec, in either instance.
- **`customer_experience`'s and `compliance`'s resolved policies more broadly** (Phase 3): even
  where concrete content existed, citations pointed at the generic user story or the wrong ADR
  rather than the supplied fact — consistent with open-ended, multi-concern prose being harder for
  the citation mechanism to selectively extract from than a closed-set or single-clause fact.
- **A necessary qualification, not a clean binary**: `growth_retention`'s fact was also broad,
  discursive prose, and it *did* leave a strong, direct trace — but in scenarios, not in any cited
  policy resolution. Prose authorship's failure in this chain is not absolute; it correlates with
  *policy-citation* consumption specifically failing more than with every downstream artifact
  category failing uniformly. This nuance would be lost by stating the pattern as a flat rule.

# Does the Evidence Support "Humans Answer Operational Questions" More Strongly Than "Humans Volunteer Operational Facts"?

**Not yet — and this is the precise place the reframing needs sharpening rather than confirming
outright.** The evidence strongly supports a claim about **fact shape and pipeline consumption**:
bounded, concrete, single-purpose content is consumed more reliably than broad, principle-level
prose, regardless of who or what produced it. It does **not** yet support a claim specifically
about **human behavior** — because the one place in this entire chain where a real human could
have volunteered open-ended content (`init`'s optional role-description prompt) was **skipped**,
not attempted and produced poorly. Silence is a different observation from bad prose. The evidence
cannot currently distinguish between two live possibilities: (a) humans avoid *open-ended
authorship* specifically, or (b) humans avoid *anything optional and skippable*, regardless of
whether the expected answer is a classification or free text — because the one real data point
confounds "open-ended" and "skippable" together and has never varied either independently.

Stated as precisely as the evidence allows: **the chain shows classification (or any sufficiently
concrete, bounded fact) survives the pipeline better than prose does, once supplied.** It does
**not yet show** that humans are more willing or able to *supply* a classification than to author
prose — that would require observing a real human doing one or the other, which has not happened.
The surviving uncertainty from the prior audit — can a real user reliably provide *any* operational
fact — is not resolved or superseded by this reframing; it is narrowed by one useful, evidence-
backed detail: **if** a real user does engage, the evidence gives good reason to expect a
classification-shaped answer to survive the pipeline better than a prose one would. Whether the
user engages at all remains exactly as untested as before.
