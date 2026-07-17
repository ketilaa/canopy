# Critiquing the Candidate Role Meaning Fact Shape

Status: critique of `docs/design/role-meaning-minimal-fact-content.md`'s own three-element
proposal (role identifier, classification value, classification-specific rationale). Re-examines
the actual citation text from every experiment in this chain, not just whether each element was
*present*, to test whether the three-element shape is correctly identified or overstated. No
collection, storage, or UX discussed.

Date: 2026-07-17

---

# Re-Examining the Actual Citation Text

The prior document noted each successful fact included a `title` (naming the role), a `decision`
(the classification value), and a `reason` (a rationale sentence) — and treated all three as at
least plausibly part of the minimal shape, with the rationale's independent contribution left
"not isolated." This critique goes one step further: it re-reads what the resulting `evidence`
fields *actually quoted*, across every instance available, rather than only checking whether each
input field was present.

**Role Meaning Value Experiment**, all three tested conditions:
- `internal`: `evidence: "Role Definition: Manufacturer Representative: Internal"` — reproduces
  the ADR's `title` and `decision` verbatim. The `reason` field
  ("The role registers data as part of this business's own operations, not on behalf of an
  outside party") is **never quoted**.
- `external`: `evidence: "Role Definition: Manufacturer Representative: External"` — same pattern.
  `reason` again never quoted.
- `affiliated`: `authorization` was not resolved at all. No citation of the fact's `title`,
  `decision`, or `reason` occurred — the role fact was not cited in any form.

**Human Insight Process Experiment (Phases 2–3)**, the one clean success (`risk_averse`):
`evidence: "Return Eligibility and Verification Policy: A return request must include the
original order/purchase confirmation number, verified against our records, before it can be
accepted"` — this reproduces the ADR's `title` and `decision` fields verbatim.
`risk_averse`'s own `reason` ("Unverified requests are rejected outright to prevent fraudulent
return claims — loss prevention takes priority over convenience") is, again, **never quoted**.

**Across every other tested persona** (`customer_experience`, `compliance`, `growth_retention`),
where citation succeeded at all, it cited the generic user story or an unrelated ADR — never the
supplied fact's own `reason` field, in any instance, in either experiment.

**The pattern holds without exception across every citation observed in this entire investigation
chain: the `title` and `decision` fields are what get reproduced. The `reason` field has never
once appeared in any resulting `evidence` text.**

---

# Evidence Per Element

## Role Identifier (the `title`, naming which role)
- **Evidence for**: directly consumed — appears verbatim in every successful citation
  (`"Role Definition: Manufacturer Representative: ..."`, `"Return Eligibility and Verification
  Policy: ..."`). This is not inferred; it is the literal quoted text.
- **Evidence missing**: never tested against a project with more than one role, so
  misattribution risk (citing the wrong role's fact) has never been exercised.
- **Assessment**: essential, and the strongest-evidenced of the three candidate elements — its
  contribution is directly visible in the cited text, not merely assumed.

## Classification Value (the `decision`)
- **Evidence for**: also directly consumed, verbatim, in every successful citation. It is also the
  one element whose *value* correlates cleanly with the success/failure split across the three
  tested conditions — `internal`/`external` resolved, `affiliated` did not.
- **Evidence missing**: whether the four-value set itself (`internal`/`external`/`affiliated`/
  `unresolved`) is final is unresolved by the stability test's own findings; `unresolved` as an
  explicit human-selected value (as opposed to no fact supplied at all) was never directly tested.
- **Assessment**: essential — the single element with both consumption evidence and
  differentiating (success-vs-failure) evidence behind it.

## Classification-Specific Rationale (the `reason`)
- **Evidence for**: none found on this re-examination. It was present in every tested condition,
  succeeding and failing alike, and was never once quoted in any resulting citation.
- **Evidence missing/against**: the cleanest available comparison — `internal`/`external` (both
  with a reason, both succeeded) vs. `affiliated` (also with a comparably clear, well-formed
  reason, and it failed) — argues against the reason being what differentiates outcomes. If the
  reason were doing independent causal work, a plausible expectation would be that its quality or
  specificity might explain some of the variance; instead the `affiliated` reason reads as no less
  clear than the other two, yet the classification value alone tracks the result.
- **Assessment**: the weakest-evidenced of the three. Presence does not establish contribution,
  and every direct observation available argues the opposite — it is real, honest counter-
  evidence, not merely an absence of support.

---

# Which Elements May Be Incidental Artifacts of the ADR Format?

Both `reason` and (per the prior document) `alternatives` were present in every tested fact only
because the `Adr` struct used to express them requires those fields — not because either was
independently shown to matter. `alternatives` was already flagged in the prior document as
unevidenced either way. This critique adds a sharper, more direct finding for `reason`
specifically: it isn't merely unevidenced, it is the one element with a **direct, repeated,
zero-exception absence from every citation ever produced**, alongside an evidenced correlation
pattern between success/failure that the classification value alone already fully explains. Of the
two, `reason` has the *stronger* case for being incidental, since the evidence against it is
positive counter-evidence, not just silence.

---

# Strongest Argument the Rationale Is Not Needed

The classification value's own presence/absence perfectly tracks the observed success/failure
pattern across all three tested conditions, with the reason field held roughly constant in
clarity and specificity throughout. No citation, in either experiment, in any instance, has ever
reproduced a reason. Two independent experiments converge on the identical pattern
(title + decision cited; reason never cited) — this is about as strong and consistent a negative
result as this investigation chain has produced anywhere.

# Strongest Argument the Rationale Is Required

**A real, unclosed possibility, stated fairly rather than dismissed**: the model reads the entire
ADR as context before producing its own resolution and citation. It is possible the `reason` field
influences the model's underlying *willingness to resolve the checklist item at all* — supplying
internal justification or confidence — even though it never appears in the literal quoted output.
Citation text reflects what gets reproduced, not necessarily everything that shaped the decision to
reproduce it. This has never been isolated: no experiment in this chain has tested a classification
value with **no** accompanying reason at all, so "the reason contributes nothing" and "the reason's
contribution is simply invisible in the citation text" remain both consistent with the data
gathered so far.

---

# Could the Successful Experiments Be Explained by the Classification Value Alone?

**Yes — this is the best-supported reading of the evidence as it stands.** The classification
value and the role identifier are the only two elements with direct, repeated, positive evidence
of being consumed; the rationale has a direct, repeated pattern of *not* being consumed, alongside
evidence its presence or quality doesn't track the success/failure split either. Nothing in the
data gathered requires positing an independent contribution from the rationale to explain what was
observed.

**Is there evidence the rationale independently contributes to consumption?** None has been found
on this re-examination. What remains is a real, named gap rather than a settled negative: no
ablation test (classification value alone, reason field entirely absent) has ever been run, so the
rationale's contribution to the model's internal judgment — as opposed to its contribution to what
gets literally quoted — cannot be fully ruled out, only shown to leave no visible trace in every
instance examined so far.

---

# Revised Assessment of the Candidate Fact Shape

The three-element shape proposed in `docs/design/role-meaning-minimal-fact-content.md` is not
wrong, but it is **overstated** in one specific respect: **role identifier** and **classification
value** are both directly, positively evidenced as necessary and consumed. The **rationale**'s
place in that list should be downgraded from "included, independent contribution unproven" to
"likely incidental to the ADR format used in testing, with active counter-evidence against
independent contribution, and one specific, nameable gap (no ablation test) standing between
that and a fully settled conclusion."
