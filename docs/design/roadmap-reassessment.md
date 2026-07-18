# Roadmap Reassessment

Status: assessment only — no implementation, no redesign proposal. Answers "what should Canopy
investigate next," grounded in the accumulated evidence from the contract-driven implementation
investigation (Stages 1–6), the Contract Readiness Assessment, the Contract Composition
Assessment (including its §8 correction), and the new Pre-Behavior Planning Review — not
intuition about what seems important.

Date: 2026-07-15

Reviewed: `docs/contract-readiness-assessment.md`, `docs/design/contract-driven-implementation-
experiment.md` (Stages 1–6), `docs/design/contract-composition-assessment.md` (incl. §8),
`docs/design/pre-behavior-planning-review.md`, `docs/open-questions/pre-behavior-planning-
review.md`, `docs/principles/implementation-ownership-requires-full-file-scope-visibility.md`,
`docs/principles/compute-facts-mechanically.md`, `docs/principles/unresolved-decisions-become-
explicit-decision-points.md`, `docs/narratives/the-road-to-contracts.md`, `docs/narratives/the-
evolution-of-canopys-stated-purpose.md`, `docs/reports/manufacturer-001.md`.

---

## 1. Executive Summary

The contract architecture has moved from "can this work at all" to "is this the highest-value
place left to look." Six stages of adversarial testing (Stages 1–6) answered every foundational
question this investigation opened with — contracts can drive discovery in production
(Stage 4), full-file-scope visibility fixes ownership invention (Stage 2, reconfirmed Stages 3–4),
contract-scoped generation beats production's real prompt on a single-entity case (Stage 5), and
the mechanical dependency rule works end to end against real, non-synthetic data (Stage 6). Every
bug found along the way — and there were several, right through Stage 6 — was narrow and
mechanical (a missing directory convention, an id-namespace mismatch), never a conceptual failure
of the `Behavior → Cluster → Contract` model itself. That is a meaningfully different, more
stable, position than this investigation started from.

At the same time, the new Pre-Behavior Planning Review exposes a part of the system that has
never been evaluated at all: service discovery and technology recommendation, which happen once,
early, upstream of every contract this whole investigation has tested. Zero reproducibility runs
have ever been performed there — the entire evidence base is one anecdotal observation. Given
that composition's remaining open questions are now narrower extensions of an already-validated
model, while pre-behavior planning is completely unexamined and sits upstream of everything else,
**the evidence favors promoting pre-behavior planning reproducibility above composition's
remaining open questions** — not abandoning composition, but no longer treating it as the
default next move by inertia.

A second, low-cost opportunity emerges directly from the Pre-Behavior Planning Review's own
Decision Classification table: a **Human-Insight Inventory** — auditing which of the pipeline's
many recommendation/decision points get accepted unchanged versus modified or rejected — is
answerable entirely from artifacts Canopy already generates, requires no new Product Owner
studies, and directly tests whether the project's original goal ("take specification boilerplate
off Product Owners") is actually being achieved, or only relocated from writing to reviewing.

## 2. Updated Architectural Frontiers

Ranked by learning value per unit of effort, not by how long a thread has been open.

### 2.1 Pre-behavior planning reproducibility — highest learning value

**Why it matters.** Every stage of this investigation validated what happens *after* a story's
architecture is already decided. Nothing has validated the decision that determines which
tech-stack skill renders for every one of those later stages in the first place. If service
discovery or technology recommendation varies meaningfully across runs, that variance propagates
into everything downstream — a strictly larger blast radius than any single composition question,
because composition questions are scoped to one already-decided architecture, while this one
determines the architecture.

**What evidence exists.** One anecdotal observation (different tech stacks recommended for
comparable services across runs), not investigated. `docs/design/pre-behavior-planning-review.md`
establishes the mechanism precisely: technology recommendation happens inside one LLM call
(`identify_architectural_questions`), with no temperature control, no isolation from other
stories' already-decided context, and prompt content ordered by story-processing history rather
than any fixed order — three independent, already-identified structural variability sources, none
of which have ever been measured.

**What evidence is missing.** Whether the recommendation is actually reproducible across repeated
runs on the same input; whether variance, if any, is large enough to matter; whether it's driven
by model sampling, by prompt-content ordering, or by something else entirely.

**Next experiment.** A reproducibility sweep — the same methodology already used four times in
this project's own history (`docs/reports/manufacturer-001.md`'s Reproducibility Sweeps 1–4, for
Stage 0 completeness/policy discovery) — applied to `canopy spec` instead: run the architectural-
questions call N times against the same accepted story and fixed prior context, and compare the
proposed service ownership and technology choices. This is cheap: it reuses the existing
dogfooding fixture and an already-proven sweep methodology, not a new harness.

### 2.2 Human-Insight Inventory — new track, high alignment, low cost

**Assessment (per the explicit question asked): yes, this should become a track**, and
specifically as an artifact-audit, not a process study. The idea reframes "is Canopy helping
Product Owners" (currently unanswerable — no real PO studies are available) into "where does
Canopy currently require domain insight" (answerable today, from artifacts already on disk).

The Pre-Behavior Planning Review's own Decision Classification table is effectively the seed of
this inventory already: it enumerates every Recommendation, Explicit Decision, Human Decision, and
Implicit Decision point in the pre-behavior pipeline. What's missing is the second half — for
each of those points, how often does the real dogfooding history show Accept vs. Accept-with-edit
vs. Reject? A "Modify" on a tech-stack proposal, or a rewritten `want`/`so_that` field, is a direct
signal of where the model's output didn't match the human's actual domain knowledge.

**Honest caveat, stated plainly**: the only real dogfooding history that exists today
(`manufacturer-001`) is a single story. An inventory built from it now would establish the
*method* — what to count, how to categorize a "Modify," how to read the resulting signal — not a
statistically meaningful rate. That's still worth doing: it's a zero-cost way to have the counting
mechanism ready and validated before a second or third real story accumulates enough history to
say something with any confidence. This is explicitly a precursor to eventual PO validation, not a
substitute for it — it can tell you *where* domain insight is currently required, not whether
Canopy's current requests for it are well-calibrated.

### 2.3 Composition's harder open questions — still real, now narrower

Stage 6 answered composition's most basic question (does the mechanical dependency rule and
multi-file plan generation work against real, non-synthetic data — yes). What remains, per the
Composition Assessment, is narrower: multiple entities in one story, dependency chains deeper than
one edge, and multi-service/route-layer composition. Each of these is an increment on an
architecture that has now survived six rounds of adversarial testing without a conceptual failure
— worth continuing, but with correspondingly lower expected surprise per experiment than either
2.1 or 2.2 above.

### 2.4 Content-generation quality for a composed contract group — a natural Stage 5+6 combination

Stage 5 tested generation quality on one isolated file with zero dependencies. Stage 6 tested
planning (not generation) for a file group with a real dependency edge. Neither has tested whether
contract-scoped *generation* stays clean once a model is shown a contract with a real dependency
on another contract's file (e.g. does an `EventShape` file correctly reference `Manufacturer`'s
already-established shape, or invent its own). This sits between 2.1/2.2 and 2.3 in priority — a
natural next question once either 2.1 or 2.3 is underway, not an independent frontier.

### 2.5 Legacy planner retirement — still explicitly premature

The Composition Assessment's own five retirement conditions (§5) remain unmet: multi-entity/
dependency composition is now partially exercised (Stage 6, one edge) but not to the bar of
"exercised," the single-backend-service restriction is unlifted, the fallback path has never fired
for a real story, and every claim in this entire investigation — pre- and post-behavior alike —
still rests on one story. If anything, the new pre-behavior uncertainty raises the bar further:
the legacy planner is currently the only path proven to handle every story shape, including
whatever pre-behavior variability turns out to produce.

### 2.6 Contract schema work — lowest priority, unchanged

Six stages of adversarial testing (Contract Readiness Assessment's four, plus Stages 5–6) have now
found real bugs every time, and not one has traced back to the `Contract`/`Behavior` schema itself
— every defect was in file-target resolution, dependency matching, or prompt content. The evidence
against touching the schema again is now stronger, not weaker, than when the Contract Readiness
Assessment first made this call.

## 3. Original Vision Assessment

The project's own stated identity, unchanged since day four
(`docs/narratives/the-evolution-of-canopys-stated-purpose.md`), is "an incremental planning and
implementation engine" that "enforces discipline: behavior is specified before code is written."
That document doesn't use the phrase "take specification boilerplate off Product Owners"
verbatim — that framing is this reassessment's own restatement of the goal, offered as context —
but it's consistent with the "Everything emerges" design table and, concretely, with what the
Pre-Behavior Planning Review found: the system exists to turn freeform specification authorship
into structured generation-plus-review.

**What's already being achieved:**
- "Architecture decisions are made story by story, never all at once" — true today, mechanically
  enforced: `spec` operates on one accepted story at a time, and `behaviors`/contracts inherit
  that same per-story scope.
- "Behavior is specified before code is written" — true for the wired discovery path: Stage 4 put
  a real, gated boundary between specification and implementation-file discovery, and Stage 0
  (completeness) blocks entry into behavior extraction on a real gap.
- Turning freeform authorship into structured review — real and observable: the Pre-Behavior
  Planning Review's own inventory shows every major pre-behavior output (candidate stories,
  candidate ADRs, candidate services, candidate technology choices) arrives as something to
  review, not something to write from a blank page.

**What remains unvalidated:**
- Whether reviewing is actually less effortful than writing. No timing, effort, or satisfaction
  data exists anywhere in this project's history — the entire evidence base for "helps Product
  Owners" is architectural (the pipeline structurally routes decisions through review points), not
  behavioral (no measurement of what happens at those points).
- Whether the recommendations arriving at those review points are good enough that a human mostly
  clicks Accept, or whether they're frequently Modified or Rejected — exactly the Human-Insight
  Inventory's own open question (2.2 above), currently answered by zero evidence beyond the single
  anecdotal tech-stack observation that started this whole line of inquiry.
- Whether the recommendations themselves are even stable across runs (2.1 above) — a
  precondition for "is this recommendation good" being a well-posed question at all; an
  unreproducible recommendation can't be meaningfully rated good or bad by a single review.

**What evidence would be needed before claiming success:** at minimum, (a) Human-Insight
Inventory data across more than one story, showing accept/modify/reject rates at each review
point; (b) some proxy for reviewer effort at each point (even a rough one, like how much of a
"Modify" edit's text actually changed); and (c) reproducibility evidence for the
recommendation-generating steps themselves, since a recommendation that changes every run
undermines any claim about its quality before quality is even the question. None of these three
currently exist.

## 4. Recommended Priority Order

| Rank | Item | Why it matters | Evidence that exists | Evidence missing | Next experiment |
|---|---|---|---|---|---|
| 1 | Pre-behavior planning reproducibility | Upstream of every later stage; determines the architecture every contract-driven experiment so far has assumed as given | One anecdotal observation; the mechanism is now precisely documented (`pre-behavior-planning-review.md`) | Whether variance is real, how large, and what drives it | Reproducibility sweep on `canopy spec`'s architectural-questions call, same methodology as prior Sweeps 1–4 |
| 2 | Human-Insight Inventory | Directly tests the project's own stated purpose; answerable now, no PO study needed | The Decision Classification table already names every review point to audit | Actual accept/modify/reject counts and rates, even for one story | Categorize `manufacturer-001`'s real dogfooding history against the existing Decision Classification table |
| 3 | Composition's harder questions (multi-entity, deeper chains, multi-service) | Real remaining gaps, but narrower and lower-surprise than #1/#2 given six clean stages so far | Stage 6 proved the single-edge case; Stages 1–4 proved single-entity ownership | Multi-entity behavior, chain depth beyond one edge, route-layer composition | A second real entity/story, or a deeper synthetic-then-real dependency chain |
| 4 | Content-generation quality for a composed group | Natural combination of Stage 5 (generation quality) and Stage 6 (real dependency edge), neither tested together | Stage 5: generation beats production on an isolated file. Stage 6: planning works for a dependent group | Generation quality specifically for the dependent files Stage 6 planned | Re-run a Stage-5-style A/B, this time on the `EventShape`/`Publication` files Stage 6 planned |
| 5 | Legacy planner retirement | None of the five stated conditions are met; premature by the project's own criteria | — | All five conditions in Composition Assessment §5 | Not a near-term experiment; revisit only once #1 and #3 mature |
| 6 | Contract schema work | Zero evidence across six adversarial stages points at a missing fact | Six stages of active attempts to break something, none schema-shaped | A concrete, evidence-backed missing fact — none identified | None recommended; revisit only if a future experiment names one |

## 5. Suggested Next Investigation

**Pre-Behavior Planning Reproducibility Sweep**, using the same sweep methodology already proven
in this project four times over (`docs/reports/manufacturer-001.md`, Sweeps 1–4), applied for the
first time to `canopy spec`'s architectural-questions call instead of Stage 0's completeness/
policy-discovery calls. Concretely: run `canopy spec` N times (N ≥ 3, matching this
investigation's own standing reproducibility bar) against the same accepted story with identical
prior context, and compare the proposed service ownership and technology recommendations across
runs — not their acceptance, just their content. This is scoped well below "redesign how
technology gets chosen"; it only measures whether today's mechanism is stable, exactly as every
prior stage in this investigation measured mechanism behavior before ever proposing a change.

Run this in parallel with — not instead of — a first-pass **Human-Insight Inventory** against
`manufacturer-001`'s existing history: it costs almost nothing beyond re-reading artifacts already
on disk, and its output (which review points show real correction activity) will directly sharpen
what the reproducibility sweep should look for (a recommendation type the human never corrects is
lower-priority to test for stability than one that gets corrected often).

## 6. Knowledge-Capture Recommendations

Per the trigger-based cadence (CLAUDE.md's Knowledge Capture Cadence): assessed, not executed —
consistent with this document's own "no implementation" scope.

- **Principle update — `implementation-ownership-requires-full-file-scope-visibility`**: Stage 6
  produced the first real, non-synthetic cross-contract dependency, which is progress toward the
  confidence-limiting caveat this principle already names ("a real story with more than one
  entity, or real (non-empty) cross-contract dependencies... neither of which exists yet"). One
  half of that caveat is now resolved; the other (multiple entities) is not, and Stage 6 tested
  planning, not the ownership-visibility question itself (no content was generated for the
  dependent files). **Recommendation: extend the evidence list, do not change the confidence
  rating** — per this project's own explicit rule that extending evidence without inflating
  confidence is a valid, expected outcome, not an oversight.
- **Blog draft — Stage 6's falsified-prediction arc**: the Composition Assessment stated a
  concrete, checkable prediction ("cheap to close... from data already in hand"), which was then
  checked and found false, surfacing two named blockers plus two more found while fixing them.
  This has a genuine prediction that could have gone the other way, a real test, and a surprising
  result — exactly the blog-drafts trigger bar. **Recommendation: strong candidate**, not yet
  drafted.
- **Narrative update — `the-road-to-contracts.md`**: already updated during Stage 5/6 work this
  session (targeted, not a full regeneration) — current, no further action needed right now.
- **Narrative check — `the-evolution-of-canopys-stated-purpose.md`**: reviewed against this
  reassessment's own findings. Its stated Open Questions (why the project started; whether the
  06-23 statement was ever reconsidered) aren't touched by anything in this reassessment.
  **Recommendation: no trigger fired**, no update warranted yet.
- **Open-question update — `docs/open-questions/pre-behavior-planning-review.md`**: this
  reassessment recommends *promoting* this investigation's priority, but promoting it in a
  roadmap document is not the same as starting it. **Recommendation: leave `status: deferred` as
  is until you actually decide to launch the reproducibility sweep in §5** — updating it now would
  presume a decision this document only recommends.

---

## Update (2026-07-15): Roadmap Adjustment After the Reproducibility Sweep

The sweep this reassessment recommended (§5) ran and produced a sharper, more specific result than
anticipated: not generic "recommendations vary somewhat," but a clean split — backend technology
and database choice perfectly stable (5/5 each), service/frontend naming only tier-2 (equivalent,
not materially different), and the domain-event proposal's own presence and topic-convention
compliance the single least stable output measured (present in 3/5 runs, convention-compliant in
only 1 of those 3) — genuine tier-4 architectural divergence, not just tier-3 wobble. Full results:
`docs/design/pre-behavior-planning-reproducibility-sweep.md`'s "Results" section;
`docs/reports/manufacturer-001.md`'s matching entry. This update reassesses the roadmap against
that specific result, following the same observe → classify → understand → decide discipline the
contract investigation itself used — no fixes proposed here.

### 1. Does the priority order still hold?

Mostly, with one real adjustment. The original order promoted pre-behavior planning above
composition on the strength of an *anecdote*; it's now promoted on the strength of a *measured,
specific* result, which changes what should happen next more than it changes the ranking itself.
Two concrete shifts:

- **Human-Insight Inventory moves from #2 to the immediate next investigation** (see §2 below) —
  the sweep gave it a sharp, well-motivated starting point it didn't have before.
- **Composition's harder questions move down further**, not because composition itself lost
  value, but because the sweep's evidence bears directly on Stage 6's own foundation: the real
  dependency edge Stage 6 produced rested on exactly the category (domain-event proposals) the
  sweep just showed is the least reproducible thing in the whole pre-behavior pipeline. Extending
  composition further before understanding *that* better risks building on the specific piece of
  ground just shown to be softest.

Legacy planner retirement and contract schema work are unaffected — no new evidence points at
either.

### 2. Should Human-Insight Inventory become the next investigation?

**Yes**, and more clearly than when originally proposed. The original case was "cheap and
complementary." The sweep adds a real reason, not just a convenient one: it shows some
recommendation categories are highly stable (tech/database) and at least one is highly unstable
(domain event) — which means the review gate a human sees today looks structurally identical
regardless of which kind of recommendation is behind it. A human reviewing a domain-event proposal
has no way to know, from the review screen alone, that the answer they're looking at is one of
several the model could just as easily have produced.

**What it should measure first**: not a broad sweep of every review point (the original scope),
but specifically — for `manufacturer-001`'s real dogfooding history — how domain-event/structural
proposals were actually reviewed (Accept/Modify/Reject), compared against how tech-stack/database
proposals were reviewed. Two possible findings, both informative and neither assumed in advance:
if domain-event proposals were *also* frequently modified or rejected historically, that's
convergent evidence — instability and human correction agreeing on the same weak spot. If they
were mostly just Accepted despite being the least reproducible category measured, that's a
different, arguably more important finding — it would mean the review gate currently gives no
signal that a low-reproducibility recommendation is being rubber-stamped.

### 3. Domain Event Recommendation Variability — symptom, root cause, special case, or broader pattern?

**Evidence of a broader pattern already established elsewhere in this project, landing on the one
case that combines two of its ingredients most sharply — not a new, isolated phenomenon.**

Reasoning: the stable categories (backend tech, database) share a trait the unstable one lacks —
each has a small set of extremely well-known, training-data-dominant defaults ("Spring Boot" for a
Java backend, "PostgreSQL" for a relational store), so the model has strong external convention to
lean on regardless of story specifics. Domain-event determination has neither: whether an event
should be raised *at all* is a judgment call with no universal default, and the exact required
shape (`"<EventName> on topic <topic>"`) is a bespoke, Canopy-specific convention with no strong
analog in general training data. That combination — a genuine judgment call, layered with a
bespoke formatting requirement — is exactly the shape `unresolved-decisions-become-explicit-
decision-points` already describes at `high` confidence (a different sweep, 2026-07-14, policy
discovery: 5/6 confidently-resolved-with-no-basis before that principle's fix). This sweep is a
new, independent instance of the same underlying pattern, not a fresh discovery requiring its own
theory.

**Consequence for scoping**: this does *not* need to become its own freestanding investigation
track — spinning one up would risk exactly the "five concurrent architectural investigations" the
Morning Planning Review's own priority rules warn against, for a question an existing high-
confidence principle already explains the shape of. Instead: fold "should domain-event
determination become an explicit Decision Point" into the Human-Insight Inventory's scope as its
first, most concrete item (§2), rather than opening a seventh independent frontier.

### 4. Original Vision Alignment — where does domain-event recommendation fit?

Offered categories: boilerplate generation / domain insight / architecture judgment / something
else. **Domain insight / architecture judgment — not boilerplate** — and the sweep's own stability
split may be tracking this distinction empirically, for the first time, not just conceptually.

Backend tech and database choice are boilerplate-shaped in exactly the sense the original vision
meant: any competent engineer would default to nearly the same choice regardless of the specific
story, so a human reviewing the proposal is genuinely just rubber-stamping a low-insight decision
— and the sweep confirms these are the stable categories. Whether a domain event should be raised,
and what it represents, requires understanding the business process well enough to know whether
downstream systems care about this state change — a genuine domain judgment, the kind of thing a
Product Owner or domain expert should weigh in on, not a default a model should confidently invent.
The sweep's instability here may not be simply "the model is bad at this specific task" — it may
be a symptom that this was always the kind of decision requiring human domain insight, being asked
of the model as if it were boilerplate. **Stated as a hypothesis worth testing further, not a
proven law** — one sweep, one story, is suggestive, not conclusive; but it's a clean, non-obvious
answer to a question this document has asked in the abstract twice now (§3 originally, again here)
and can finally ground in a specific, measured result rather than architectural description alone.

### 5. Updated Priority Order

| Rank | Item | Current evidence | Expected learning value | Suggested next experiment |
|---|---|---|---|---|
| 1 | Human-Insight Inventory, domain-event/structural review outcomes first | Sweep shows a sharp stable/unstable split with no matching human-review signal yet measured | High — directly tests whether the review gate can currently tell a human which recommendations to scrutinize | Categorize `manufacturer-001`'s real review history against the Decision Classification table, domain-event and tech-stack proposals specifically |
| 2 | Should domain-event determination become a Decision Point? | `unresolved-decisions-become-explicit-decision-points` (`high` confidence) already describes this shape; this sweep is a new instance, not new evidence for a new claim | Medium-high — a design question informed by #1's results, not a separate experiment | Read #1's results against the existing principle's own applicability criteria before deciding anything |
| 3 | Composition's harder questions (multi-entity, deeper chains, multi-service) | Stage 6 proved the single-edge case, but on exactly the category now shown least reproducible | Medium, lower than before this sweep — real, but riskier to build on before #1/#2 land | Unchanged from the original reassessment; simply reordered behind #1/#2 |
| 4 | Content-generation quality for a composed group | Unchanged | Medium | Unchanged |
| 5 | Legacy planner retirement | Still premature per Composition Assessment §5 | Low near-term | Not a near-term experiment |
| 6 | Contract schema work | Zero evidence across six-plus adversarial stages points at a missing fact | Low | None recommended |

### 6. Morning Planning Review Support

**Current architectural frontier**: the Human-Insight Inventory, specifically scoped around
domain-event/structural-proposal review outcomes — the sharpest, most concrete open thread this
project has right now, directly motivated by hard evidence rather than an anecdote.

**What a future "What should we do today?" should likely recommend as the primary task**: start
the Human-Insight Inventory against `manufacturer-001`'s real dogfooding history, comparing review
behavior on domain-event/structural proposals against tech-stack/database proposals — not a broad
audit of every review point, the narrower, sweep-motivated version of it. A reasonable optional
secondary task, if there's a natural small follow-up that day: reading the Inventory's first
results against `unresolved-decisions-become-explicit-decision-points`'s own applicability
criteria, to see whether the Decision-Point question in §2 above is answerable yet without a
separate experiment.

---

## Update (2026-07-15): Items 1 and 2 Closed for Now

Both ran. **Item 1** — `docs/design/human-insight-inventory.md`: every proposal in
`manufacturer-001`'s one real review session, across every reproducibility tier, was Accepted
verbatim with no differentiated scrutiny; Stage 2's own decision-point mechanism didn't catch it
either. **Item 2** — `docs/design/domain-event-decision-point-criteria-comparison.md`: comparing
domain-event determination's three bundled sub-decisions (existence, naming, topic-clause
formatting) against the Decision Point mechanism's actual origin criteria found two of three
already explained by an instruction-compliance gap and a missing-upstream-fact/sequencing gap, not
by Policy Discovery's fabrication-with-no-basis pattern. **Verdict: too early to tell** — not a
hedge; it rules out the simplest reading for two of three sub-decisions, but whether inconsistent
compliance with the "always propose an event" rule masks a real judgment call remains open on a
sample of one story.

**Decision, given that outcome**: no Decision Point mechanism proposed; no separate domain-event
investigation opened. `docs/open-questions/domain-boundary-explicitness.md` (a related, more
speculative hypothesis raised alongside this work) stays `active`, untouched, not folded into any
near-term plan. Both items are closed *as investigations* — this reassessment's own priority table
above is otherwise unchanged; item 3 (composition) is not being promoted back up by this update,
only recorded as no longer blocked by an *unexamined* domain-event foundation, since the shakiest
part of that foundation now has a specific, narrower explanation rather than an open question mark.

---

## Update (2026-07-16): New Evidence for Item 1 — The Product-Owner Perspective Experiment

`docs/design/product-owner-perspective-experiment.md` adds independent evidence to item 1's own
closed finding, via a different method: five simulated Product Owner personas reviewing
`manufacturer-001`'s real generated artifacts, rather than reconstructing historical review
outcomes. Four gaps surfaced, two of them independently reached by different personas reasoning
in entirely different ways — a global duplicate-name rule with no distinguishing identifier
(domain-expert and customer-outcome angles), and an undefined relationship to a `Product` entity
that the story's own `so_that` field depends on but the domain vocabulary has never captured
(product-portfolio angle, directly corroborating `domain-boundary-hypothesis-assessment.md`'s more
abstract reasoning with a concrete instance). A third — total absence of any authorization model,
despite "authenticated" appearing in every scenario's `given` clause — was noticed by exactly one
of five personas, the sharpest illustration in either experiment of how persona-dependent this
category of gap is. A fourth — ambiguity in whether "manufacturer representative" denotes an
external or internal actor — sits logically prior to the authorization gap.

**Why this counts as independent evidence, not a restatement**: the Human-Insight Inventory asked
"how was this actually reviewed" (a historical, single-session reconstruction). This experiment
asks "what would an engaged reviewer notice" (a simulated, multi-lens re-reading of the same
artifacts). Both converge on the same underlying conclusion — Canopy's output looks fully resolved
regardless of whether it actually is — reached by two different methods against the same real
story. That convergence is stronger than either finding alone.

**A shared pattern across the four gaps, worth naming precisely**: all four are cases where
generated language — a business rule's implicit equivalence criterion ("same name" standing in for
"same manufacturer"), a role label ("manufacturer representative"), a precondition word
("authenticated"), a purpose clause ("so that products can reference them") — reads as though it
already denotes a specific, settled real-world concept, when the concept itself was never
separately established anywhere in the pipeline's own artifacts (`domain_registry.yaml`,
`roles.yaml`, an ADR, a Decision Point). This is a narrower and, in three of the four cases, a
*prior* failure mode to the one `unresolved-decisions-become-explicit-decision-points` already
names: that principle covers a model *recognizing* a question exists and then silently fabricating
an answer to it (duplicate-name handling fits this shape well — it is a genuine business-policy
question the model answered with an unstated assumption). Role semantics, the authorization gap,
and the `Product`-relationship gap are different in kind: nothing here indicates the model ever
registered a question at all, because the specification's own fluent phrasing already presupposes
a meaning for the term in play. `open_questions: []` and Stage 0's `gaps: []` are both accurate
under this pipeline's current definition of "gap," and both miss all three — because none of the
three ever took the shape of a flagged, unresolved question in the first place.

**Working name for this shared class, offered here as a description of the pattern, not a proposed
mechanism**: an *unestablished referent* — a term or relationship the specification uses fluently,
that determines correctness downstream, whose actual real-world meaning was never confirmed against
this project's own established vocabulary. This is distinct from a missing mechanical fact
(computable, no ambiguity) and distinct from an unresolved policy decision in the existing
Decision Point sense (a recognized question with no supporting basis) — it sits one level further
back, at whether the term itself was ever checked against what this project already knows a
`Manufacturer`, a `manufacturer representative`, or "products referencing them" actually means.

**No mechanism proposed here** — matching this update's own evidence-before-redesign discipline
and the Product-Owner Perspective Experiment's explicit charter. This is filed as additional,
independently-derived evidence for item 1's already-closed finding, not a reopening of item 1 or a
new item 7.

**Correction (2026-07-16, same day)**: the "unestablished referent" synthesis above was run
through this project's own observation → hypothesis → evidence → counter-evidence discipline in
`docs/design/unestablished-referent-hypothesis-review.md` and does not survive as a new concept.
Three of the four gaps are better explained as an existing-mechanism coverage gap (duplicate-name
fits `unresolved-decisions-become-explicit-decision-points` directly; role-semantics/authorization
are most likely one gap, not two, and best explained by `exhaustive-enumeration-over-holistic-
review`'s already-validated finding that Stage 0/2's checklists don't yet enumerate these specific
items); the fourth (`Product` relationship) already has a home in `domain-boundary-
explicitness.md`. The experiment's underlying observations (§1 of that document) stand; the shared-
class synthesis drawn from them does not. Read that document for the full counter-evidence pass,
not this paragraph's summary of it.

---

## Update (2026-07-18): Backlog Execution Plan

Status: decided, not further analysis. Closes the long chain of investigation this document has
tracked since 2026-07-15 (Role Meaning, in full, via `docs/design/role-meaning-collapses-to-
classification.md`; vocabulary-discrepancy, in full, via `docs/reports/backlog-discovery-
vocabulary-check.md`'s Runs #1–#3) and translates the resulting reassessment directly into
execution. No new investigation opened here — only what to build next, in order.

### Active Tracks

- **Backlog Evolution** — what capabilities does Canopy already know about (entities, events,
  roles) that have no story yet. Concept-level signals only, primary: entity-with-no-story.
- **Story Readiness** — does *this* story's own spec/behaviors/contracts hold together. The
  existing Stage 0/2/continuity-audit machinery, now named as answering a different question than
  Backlog Evolution rather than the same one. The Human-Insight Inventory rerun belongs here.
- **Vertical-slice implementation from a real discovered gap** — `Customer`, the first candidate
  Backlog Evolution has actually found, carried through the real pipeline rather than left as a
  finding.
- **Parked, unaffected, low priority** (real, not retired, just not next): composition's harder
  questions (multi-entity, deeper chains, multi-service), content-generation quality for a composed
  contract group, legacy planner retirement, contract schema work.

### Retired Tracks

- **Vocabulary-discrepancy (word-level) detection** — concluded; operates on the wrong unit
  (tokens, not concepts). Shipped code stays; no further precision work planned against it.
- **Role Meaning / Role Semantics investigation** — concluded; collapsed to one narrow, local fact
  (role identifier + classification value feeding `authorization`). No further investigation.
- **Operational-fact collection-strategy analysis** — concluded; its only output was the Role
  Meaning finding above. No second operational fact is queued for the same treatment.
- **Canopy-Assisted Domain Exploration vision / Domain Exploration MVP** — superseded by the
  concept-level Backlog Evolution framing, which is narrower and better evidenced.
- **Domain-event Decision Point question** — closed 2026-07-15 ("too early to tell"); no new
  evidence since.
- **Pre-behavior planning reproducibility, as an active investigation** — the sweep ran; its
  finding (domain-event proposals are the least reproducible category) is already absorbed into the
  closed Human-Insight Inventory / Decision-Point items above. Not a track to keep running.

### Next 3 Iterations

**Iteration 1 — Entity-with-no-story check**
- *Objective*: a small, standalone, mechanical check comparing `domain_registry.yaml` entity names
  against every story's `as_a` value (cross-checked with `roles.yaml`), surfacing entities that
  have never been the subject of any story.
- *Expected learning*: whether a concept-level signal reliably surfaces real, worthwhile backlog
  gaps the way the retired word-level check did not — the first direct test of today's core
  conclusion.
- *Implementation effort*: low. A set-difference over data Canopy already produces; no LLM call;
  same tier as the already-shipped mechanical checks (`classify_proposal_category`,
  `find_vocabulary_discrepancies`).
- *Completion criteria*: run against the real dogfooding project and correctly surface all 8
  already-confirmed uncaptured entities (`Order`, `Customer`, `Cart`, `Payment`, `Supplier`,
  `Review`, `Discount`, `Notification`) via the `as_a`-only method, with the `product-008`
  false-negative trap (the word "customers" appearing in `so_that` text) correctly avoided; built,
  tested, committed, reinstalled.

**Iteration 2 — Human-Insight Inventory rerun**
- *Objective*: re-run the existing Human-Insight Inventory counting method, unchanged, against the
  full `review-log.yaml` history now accumulated across every real dogfooding session this project
  has run (`manufacturer-001`'s original review plus every product-*/warehouse-*/support-*/
  finance-*/delivery-* session since) — not the single-story sample the original inventory used.
- *Expected learning*: whether "too early to tell" resolves into an actual pattern now that a much
  larger real sample exists — specifically whether some review categories get real scrutiny
  (modify/reject, or meaningful/not-sure) while others are uniformly rubber-stamped.
- *Implementation effort*: near-zero. No new code — reuse the counting/categorization method
  already validated in `docs/design/human-insight-inventory.md` against data that already exists on
  disk.
- *Completion criteria*: a written comparison of accept/modify/reject and meaningful/not-meaningful/
  not-sure rates across categories and sessions, with an explicit verdict on whether the original
  "too early to tell" finding still holds or is now answerable.

**Iteration 3 — Customer vertical slice**
- *Objective*: carry `Customer` — the first real candidate Iteration 1 identifies — through
  `intent → spec → behaviors → implement` as a real story in the dogfooding project.
- *Expected learning*: whether a Backlog Evolution finding actually converts into a safely
  implementable story end to end — the first real test of discovery-to-implementation for a
  genuinely discovered, not synthetic, gap.
- *Implementation effort*: medium — a real story through the existing pipeline, same cost class as
  any other story; no new mechanism required.
- *Completion criteria*: a `Customer`-facing story (e.g. "browse the catalog") accepted, specified,
  and carried at least through `behaviors`/contract generation, with results recorded at the same
  bar as prior dogfooding sessions in `docs/reports/`.

### What specific question is each iteration answering?

1. **Entity-with-no-story**: does a concept-level signal (known entity, no story) reliably surface
   real, worthwhile backlog gaps, where word-level detection did not?
2. **Human-Insight Inventory rerun**: now that real review data spans many more sessions, does the
   review gate actually differentiate scrutiny by category, or is everything still rubber-stamped
   regardless of category?
3. **Customer vertical slice**: does a capability discovered by the entity-with-no-story signal
   actually make it safely into implemented software?
