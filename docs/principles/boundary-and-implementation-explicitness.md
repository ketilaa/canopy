---
title: "State Both Sides of a Boundary, and Name What the Implementation May Not Use"
status: draft

confidence: medium

maturity: emerging

update_2026_10_25: "X11 tested negation's generality on a second adversarial boundary (order-exclusive, the \
500.00 discount threshold reversed) and found the second boundary was not actually hard for the model even \
stated plainly (baseline 10/10, not the predicted <=3/10) — the predicted mechanism did not reproduce, because \
the training-prior conflict behind X7/X8 is specific to which idiom is being fought, not a general property of \
\"reverse any boundary.\" Confidence held at medium rather than raised; see Counter-Evidence."

themes:
  - prompt-design
  - implementer
  - plan-authoring

evidence_strength: medium

source_artifacts:
  - "commit (2026-10-25) — X7: negation framing on the exact-90-minute boundary, tickets-exclusive"
  - "commit (2026-10-25) — X8: the same treatment on the later-than-90 case"
  - "commit (2026-10-25) — X9: naming the missing Mockito dependency, stock"
  - "canopy-implementer/docs/prompt-and-plan-changelog.md, S6c (2026-10-20) — the original mock-style failure, drafted rather than hand-written"

related_principles:
  - read-the-prompt-for-contradictions-first
cluster: "Subtract Before Adding"
---

# Principle

Two distinct ways a plan step's wording can be incomplete, each with its own fix, found and measured on the
same day against the same model:

1. **A boundary condition is stated once, positively, as an addition to the "normal" case.** When the correct
   direction is the less common idiom, the model's training prior silently overrides a single positive
   statement, even when shown the exact failing assertion three times in a row. The fix: restate the rule as
   an explicit negation with a concrete worked value on the side that fights the default ("a validation
   exactly 90 minutes after issuedAt is NOT valid and must throw"), not only as an exception clause patched
   onto the positive framing.
2. **A step's checklist describes an interaction with a collaborator, not state.** When a checklist item reads
   as "X returns Y; `service.method()`: `repository.save` is called with Z" rather than "after calling X, the
   result is Z," the model reaches for a mocking framework to write that test — and fails outright when none
   is on the classpath. The fix is not a clearer description of the *behaviour* (the values and the outcome
   were identical between the failing and passing arms); it is naming what the *implementation* may not do:
   stating directly that no mocking framework is available and the test must use a real collaborator.

The second finding generalises the first: a good plan step must constrain the implementation space, not only
describe the behaviour, when the gap being filled is "which approach can the model take," not "what should
happen."

# Problem That Revealed It

`tickets-exclusive` (a deliberately reversed-boundary variant of the tickets story) failed 0 of 10 across five
earlier wording attempts (X1 through X5, 2026-10-17 to 2026-10-20), each one restating the same positive fact
in a different place or with a worked example alongside it. None moved the number. Reframing the same fact as
an explicit negation — tested first on the exact-90-minute case (X7), then on the untouched later-than-90 case
(X8) — took step 1 from 0/10 to 7/10, then to 9/10, each time measured against the exact failing assertion
being resolved and nothing else.

Separately, S6c (2026-10-20) found that a drafted plan's interaction-style checklist led the model to import
Mockito on a classpath that doesn't have it — a different failure shape entirely: not a wrong fact, a wrong
*approach*. X9 (2026-10-25) reproduced this by hand on `stock`, byte-for-byte identical behavioural content in
both arms, and found that naming the missing dependency took the step from 3/10 to 10/10 on its own, with no
change to any value, message or exception.

# Evidence

- **X7 (tickets-exclusive, n=10):** step 1 (the exact-90-minute boundary) 0/10 to 7/10, negation + a concrete
  value, against a plan whose five prior wordings (X1–X5) had all failed the same way.
- **X8 (tickets-exclusive, n=10):** the same treatment applied to the later-than-90 case, which X7 had not
  touched: that specific case reached 0 failures across 10 runs; step 1 overall reached 9/10. The one
  remaining failure was the *other* case, suggesting each boundary's treatment fixes that case largely on its
  own, not as a blanket effect.
- **X9 (stock, n=10 per arm):** a hand-written reproduction of S6c's interaction-style checklist scored 3/10;
  adding one sentence naming the missing mocking dependency, with every value and message unchanged, scored
  10/10.

# Counter-Evidence and Open Scope

- **Every result above is one model (Qwen3-14B), one story each.** No cross-model or cross-story replication
  has been run yet.
- **The boundary-negation fix has only been shown on one deliberately adversarial case, and the one attempt
  to build a second one did not replicate the mechanism it was built to test.** `order-exclusive` (X11,
  2026-10-25) reverses `order`'s 500.00 discount threshold the same way `tickets-exclusive` reverses the
  90-minute window — built with its own hidden test, validated against two hand-written references before
  use (passes the reversed rule 7/7, fails exactly the boundary case against the real rule). The baseline
  (positive statement, worked values, no negation — the same shape as X3, which scored 0/10 on the other
  case) scored **10/10** here. Negation scored 10/10 too: no regression, but no generality claim survives
  either, because there was no failure for negation to fix. **Reading:** the training-prior conflict behind
  X7/X8 is specific to which idiom is fought (inclusive-vs-exclusive on a time window), not a general property
  of "reverse any boundary direction" (here, "X or more" reversed to "strictly more than X"). `order`'s
  non-adversarial boundary was also separately improved by a *different*, earlier technique (B7/B8,
  2026-10-17: concrete values stated positively, not negation — a 7/10 to 8/10 gain, smaller than X7/X8's),
  which is consistent with this model finding `order`'s boundary comparatively easy either way.
- **The implementation-constraint fix needs a specific architecture to even arise.** Of the five established
  stories, only `tickets` and `stock` have a service constructed with an injected repository — the shape that
  makes "mock the collaborator" a plausible choice in the first place. `order` and `loans` hold their state
  directly (a `HashMap` field on the service itself, no constructor-injected collaborator); `shipping` has no
  persistence at all. So "test on a second story" for this fix means `tickets` (the only other story with the
  same shape among the five), not an arbitrary second story.

# Applicability

- Any plan step whose rule names a numeric or categorical threshold: state the case on both sides explicitly,
  with concrete values, and use negation ("is NOT valid") on whichever side is the less obvious default.
- Any plan step whose checklist could be read as describing an interaction with a collaborator rather than a
  resulting state: name the test-writing constraint directly (what libraries exist, what approach is
  disallowed), not just the expected values.
- A deterministic check for the second case is plausible without any model judgment call: flag a checklist
  item whose wording names a method call on a dependency as the *expected outcome* ("X is called with Y")
  rather than a resulting state, the same shape `exceptions_without_ownership`-style audits already use
  elsewhere in this project — not yet built.

# Confidence Assessment

Medium. Both results are clean, single-variable, paired, and reproduce the exact mechanism they were built to
test, not a loose correlation. The ceiling on confidence is scope: one model throughout, one story each, and
in the boundary case, only ever tested on a deliberately constructed adversarial example. Promote to high
once (a) a second adversarial boundary case (new or found) replicates X7/X8's shape, and (b) `tickets` (the
only available second case for the architecture X9 needs) replicates X9's shape.

# Future Validation

- **Done, X10 (2026-10-25):** X9's exact technique on `tickets` — 0/10 to 10/10, replicating X9's mechanism
  on the only other story with the required architecture. The implementation-constraint fix is now `two-story`.
  - **Done, X11 (2026-10-25):** built a second adversarial boundary case (`order-exclusive`, new hidden test,
    validated against two references) rather than the cheap non-adversarial option, per the maintainer's
    stated preference. It did not replicate X7/X8's mechanism — baseline was already 10/10 — so the
    boundary-negation fix is still effectively `one-story` for a confirmed gain, now with one documented case
    (a different kind of reversal) where the predicted difficulty did not appear at all.
- **Still open:** a boundary reversal that *does* reproduce a training-prior conflict on a second story, to
  actually test negation's generality with a real gain to measure (order's "or more" reversal wasn't one;
  `loans`' due-date boundary is confounded by that story's own unrelated 0/10 ceiling and was not attempted).
- Try either fix against a second model, since every result so far is Qwen3-14B only.
