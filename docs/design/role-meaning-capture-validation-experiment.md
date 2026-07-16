# Validation Experiment: Does Role Meaning Capture Deserve to Be Built?

Status: experiment design only. No implementation, no prompts, no code. Answers one question:
before any of that work begins, what is the smallest experiment that tells us whether real users
will actually answer a role-meaning question, and whether their answer changes anything downstream
— as opposed to whether the *idea* is coherent, which `docs/design/domain-exploration-mvp-
design.md` and `docs/design/role-classification-stability-test.md` already established.

Date: 2026-07-16

Scope, per explicit constraint: validates role meaning capture only. Does not touch ownership,
bounded contexts, glossary enrichment, hot spots, or forward-reference detection — none of those
are tested, referenced as future work, or smuggled back in below.

**Redesign note (2026-07-16, same day)**: this document's central emphasis — distinguishing genuine
engagement from mechanical filling from skipping — was set aside, not because it stopped being a
real question, but because a later instruction correctly identified it as premature: whether the
answer changes anything downstream matters more than whether users answer at all, and should be
resolved first. `docs/design/role-meaning-value-validation.md` redesigns the experiment around that
value question, assuming an answer exists rather than testing whether one is given. This document's
own §5/§6 material (the response-behavior signals, and the first version of the downstream-impact
comparison) is not wrong, only sequenced after a more important question — read the value validation
first; this document's response-rate question becomes relevant again only once that one succeeds.

---

# Why This Cannot Be a Simulated-Persona Test

Stated first because it bounds every design choice that follows. This project has already run,
and already had to correct, an experiment that mistook a single evaluator's structured self-critique
for independent evidence (`docs/design/unestablished-referent-hypothesis-review.md`'s finding on
the Product-Owner Perspective Experiment's own persona-convergence claim). The question this
validation exists to answer — *will real users actually provide this information* — is specifically
a claim about real human behavior. An LLM playing five Product Owner personas cannot answer it, no
matter how well-differentiated the personas are; it would only tell us whether the same model that
proposed the question also answers it, which is circular. **This experiment must involve real
people responding to the actual question, not simulated ones.** Everything below follows from that.

---

# 1. The Smallest Experiment

**A Wizard-of-Oz probe, not a built feature.** Since no implementation exists yet and none should
be built to run this validation, the smallest experiment is one where a real person manually
presents the bounded role-meaning question to a real user at the moment a role is actually
introduced, and records the response — the same well-established technique of simulating a feature
by hand to observe real behavior before investing in building it. No CLI change, no prompt, no
stored mechanism; the question and its recording happen outside the tool entirely.

**Two phases, cheapest first:**

- **Phase A — organic occurrence.** The next time a real dogfooding session naturally introduces a
  new role (through `intent`, in this or any other real project), the question is asked at that
  exact moment, by a person, and the response is recorded alongside which role prompted it. This is
  the cheapest possible version — it reuses the existing dogfooding workflow exactly as is, adds
  nothing to it structurally, and respects `structure-emerges-from-behavior`'s own logic by staying
  anchored to a real, already-emerging role rather than a manufactured one.
- **Phase B — a small realistic batch, only if Phase A is too slow to produce enough data.** This
  project's own real history has produced exactly one role, ever — Phase A alone may take a long
  time to accumulate a useful sample. If so, the same real tester(s) already doing this project's
  dogfooding are shown a small batch of short, realistic story fragments — reusing the same twenty
  role names already assembled in `docs/design/role-classification-stability-test.md`, since that
  set was already built, already spans multiple domains, and already has a known difficulty
  profile (clean / strained / fails) to check real responses against (see §5). Each fragment
  introduces one role; the question is asked once per fragment, the same way it would be asked once
  per role in real use.

Both phases test the same thing — real response behavior to the same bounded question — Phase B
only trades a slower, more organic sample for a faster, still-real one.

---

# 2. What Would Count as Success

- The question is **actually answered**, not skipped, at a meaningfully higher rate than the one
  directly comparable precedent this project already has: `init`'s existing optional role-
  description prompt, answered 0 of 1 times in this project's real history. Even a modest positive
  delta (answered in most, not all, presentations) would be a meaningfully different result from
  that baseline.
- Answers are **differentiated** across roles of different real difficulty — confident, fast
  classifications for the roles the stability test already found clean (`customer`,
  `administrator`), and either a considered `unresolved` or genuine hesitation for the roles it
  already found to fail (`supplier`, `auditor`, `franchise partner`). A response pattern that
  tracks the stress test's own predicted difficulty is a strong, independently-checkable signal of
  real engagement.
- The captured answer, fed into the relevant downstream step, produces a **measurably different**
  output than the same step run without it (see §6) — not just capture for its own sake.

# 3. What Would Count as Failure

- The question is skipped at a rate similar to the existing optional-description precedent —
  direct evidence a bounded question, on its own, isn't enough incentive, generalizing the Policy
  Discovery lesson (an escape hatch alone doesn't change behavior without an enforced cost) past
  business policy into meaning-level questions too.
- Answers are given but **uniform regardless of role** — the same classification chosen for an
  obviously-external `customer` and a genuinely-ambiguous `supplier` alike — indicating mechanical
  compliance with a UI element, not engagement with the actual question.
- The captured answer, even when genuinely given, makes **no observable difference** to the
  downstream artifact it was theorized to ground — telling us the value chain this MVP was built on
  doesn't hold in practice, independent of whether capture itself works.

# 4. What We Would Learn, Per Outcome

- **Full success** (answered, differentiated, and downstream-impactful): real, not just structural,
  evidence that "ask a bounded question, preserve the answer" generalizes past business policy —
  justifies real implementation investment, and gives the domain-exploration vision its first live
  data point rather than only argued ones.
- **Failure by skipping**: tells us to stop before implementing a simple optional-prompt shape at
  all — the mechanism needs an enforced cost for non-answers, mirroring Policy Discovery's own
  fix, and that redesign question — not this MVP shape — is what would need resolving before any
  implementation work, if the idea proceeds at all.
- **Failure by mechanical/uniform answering**: tells us the question, even though structurally
  sound, doesn't connect to something users naturally reason about in the setting it was asked in —
  worth distinguishing (see the validity caveat in §5) between "the question itself doesn't work"
  and "a hand-run probe outside the tool doesn't create enough context for genuine engagement,"
  since the second is a limitation of this experiment's method, not of the underlying idea.
- **Failure by no downstream difference, despite genuine answers**: tells us role meaning capture
  might be a real, well-answered thing that simply isn't load-bearing for authorization reasoning
  the way theorized — worth knowing before investing in wiring it there, and a genuinely different,
  more useful negative result than "nobody answers."

# 5. Distinguishing Genuine Value, Mechanical Filling, and Skipping

Three separate, observable signals, not one:

- **Skipping** is directly observable — no answer, or an explicit decline. Requires no inference.
- **Mechanical filling** shows up as a *response pattern*, not a single answer: near-uniform
  response speed across roles of very different real difficulty; the same classification chosen
  regardless of how ambiguous the role actually is; no elaboration even when it's invited; and, most
  directly, a lightweight follow-up question after each response ("what made you choose that") that
  comes back generic or unconnected to the specific role, rather than engaging with what's actually
  ambiguous about it.
- **Genuine value** shows up as the mirror image: response time and hesitation that track the
  stability test's own difficulty ranking (clean roles answered quickly and confidently, failing
  roles answered slowly, with `unresolved` chosen or genuine reasoning volunteered); classifications
  that vary sensibly across the batch rather than clustering on one default; and, on the same
  lightweight follow-up, a reason that specifically engages with the role's own ambiguity (e.g.,
  recognizing that a "supplier" could be the supplying company's own staff or an internal buyer,
  unprompted).

This reuses, rather than re-derives, work already done — the twenty-role stability test's own
clean/strained/fails classification becomes the ground truth this experiment checks real responses
against, rather than needing a new difficulty model built for this purpose.

# 6. Measuring Downstream Impact

A controlled comparison, matching the same before/after shape this project already used for the
Policy Discovery citation fix and the Stage 5 contract-scoped-vs-production A/B test — not a new
methodology invented for this purpose.

**Method**: take the existing, unmodified business-policy checklist step (the one that already
enumerates `authorization` as a fixed area) and run it twice against the same story — once with no
change from today's real behavior, and once with the captured role classification made available as
additional context, exactly the way an existing ADR is already made available as context to that
same step. No new prompt is authored for this comparison; the existing step runs unchanged, only
what it's given to read differs between the two runs.

**Success signal**: the `authorization` area's output differs in an observable, meaningful way
between the two runs — most notably, whether a `resolved` classification (which, per
`unresolved-decisions-become-explicit-decision-points`'s own citation requirement, needs a named,
checkable source) becomes reachable in the with-context run where it had no basis to be reached
without it. A change in the `evidence` field pointing at the newly-available role classification
would be the clearest possible confirmation.

**Failure signal**: no observable difference between the two runs — the captured fact is present
but never referenced, or the same output results regardless of which classification (internal /
external / affiliated / unresolved) was actually supplied, meaning the information isn't
discriminating anything in this step even when available.

---

# Sample Size and Stop Conditions, Stated Before Running Anything

Consistent with this project's own standing discipline (see the Pre-Behavior Planning
Reproducibility Sweep's own pre-declared thresholds): this experiment's realistic sample size is
small, and that constraint should be named honestly rather than glossed over. This project has one
real, known human tester and one real role in its entire history. A meaningful Phase B batch —
the twenty already-classified roles — gives twenty response data points from that same one tester,
which supports observing a *response pattern* (§5) even though it cannot support any claim about
how a *different* human would behave. Any conclusion drawn from this experiment should be stated
with that scope explicitly attached, the same "sample of one, one session" caveat every other
investigation in this project's recent history has had to carry.

**Stop and proceed to implementation-design only if**: the question is answered at a clearly higher
rate than the 0-of-1 baseline, the response pattern tracks the stability test's own difficulty
ranking, and at least one downstream comparison (§6) shows an observable difference.

**Stop and reconsider the MVP's shape (not proceed to implementation) if**: skipping dominates, or
answers are uniform regardless of role — per §4, the next step in either case is a design question
about incentive structure, not an implementation task.

**Stop and reconsider the value chain (not the capture mechanism) if**: answers are genuine and
differentiated, but no downstream comparison in §6 ever shows a difference.

---

# What This Does Not Test

Restated explicitly, per the stated constraints: this experiment says nothing about ownership
questions, bounded contexts, domain-glossary enrichment as its own initiative, hot-spot capture as a
general mechanism, or forward-reference detection. A positive result here would be evidence for the
narrow claim "role meaning capture, this specific shape, is worth building" — not evidence that the
broader domain-exploration vision should proceed on every front at once.
