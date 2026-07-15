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
