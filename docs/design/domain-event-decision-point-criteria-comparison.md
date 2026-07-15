# Is Domain-Event Determination the Same Kind of Problem That Produced Decision Points?

Status: comparison only. Answers one question — does domain-event determination match the actual
criteria that led to the Decision Point mechanism — against evidence already on record. No
implementation change, no new mechanism, and no recommendation is proposed here.

Date: 2026-07-15

Reviewed: `docs/principles/unresolved-decisions-become-explicit-decision-points.md`,
`docs/design/behavior-first-planning.md` (Stage 2's own design rationale — the primary source for
the mechanism's actual origin, not only the principle's retrospective account),
`docs/design/pre-behavior-planning-reproducibility-sweep.md`, `docs/design/human-insight-
inventory.md`, `docs/design/pre-behavior-planning-review.md`, `docs/design/roadmap-reassessment.md`,
`docs/open-questions/domain-boundary-explicitness.md`, `canopy-llm/src/prompts/spec.rs` (the
domain-event proposal rules, lines 154-185).

## 1. What actually caused policy/business-rule questions to become Decision Points?

Grounding this in the mechanism's own design doc (`behavior-first-planning.md`, Stage 2), not just
the principle's retrospective summary, surfaces a criterion the principle alone doesn't state as
sharply:

- **Trigger, verbatim**: "an unresolved business question... gets *recorded* today, but nothing
  stops behavior extraction or clustering from proceeding around it. A small model asked to extract
  behaviors will not stop and ask what 'duplicate product names' should do — it will silently pick
  an interpretation, and that interpretation becomes a hidden business decision baked into a
  behavior, then a cluster, then a contract, with **no record that a choice was ever made**."
- **The stated heuristic for what qualifies**, verbatim: "if answering the question would change a
  validation rule, a persistence rule, an API contract, an event contract, or a test expectation,
  it's a Decision Point." Named recurring categories: duplicate/uniqueness handling, default
  values, retention policies, **event payload contents**, ordering guarantees, authorization rules,
  error message semantics, idempotency, consistency expectations.
- **The empirical evidence that confirmed the prediction** (from the principle doc): a live
  Policy Discovery run asked the model to classify six named business-policy questions as
  resolved/not-applicable/unresolved — an explicit escape hatch existed — and the model
  "confidently resolved" most of them with specific, invented answers present nowhere in its actual
  input (5 of 6, before a fix). A controlled before/after comparison (reproducibility sweep,
  2026-07-14) measured the fix's effect directly: 5/6 fabricated → 1-2/6, with the remainder
  correctly routed to an open question.
- **The refinement the principle itself states as counter-evidence**: an escape hatch alone
  ("offer an unresolved option") did not stop fabrication. What worked was pairing it with a
  **required, checkable citation** — an answer with no citation is rejected as invalid output, not
  accepted as a low-confidence guess.

Extracting the actual, separable criteria from this (not the user's example list restated, but what
the evidence above actually supports):

| Criterion | Grounded in |
|---|---|
| **A downstream artifact depends on the answer** (validation/persistence rule, API/event contract, test expectation) | Stage 2's own stated heuristic, `behavior-first-planning.md:211-212` |
| **The model has no supporting basis for its answer, and confidently invents one anyway** — not merely "gives an answer," but gives a *specific, unsupported* one when a citation-free answer should be impossible | The core fabrication evidence: 5/6 policy questions resolved with invented specifics despite an explicit escape hatch |
| **The failure is silent** — nothing records that a judgment call was ever made, so it looks identical to a genuinely resolved fact downstream | The verbatim trigger quote above; this is the *shape* of harm the mechanism exists to prevent, distinct from the *rate* of occurrence |
| **An escape hatch alone is insufficient; the fix required an external, checkable cost for not deferring** (a citation requirement) | The principle's own Counter-Evidence section |
| **Measured, not anecdotal, evidence of the rate** | A controlled reproducibility comparison, not a single observed instance |

Notably **absent** from this list, despite being tempting to assume: "low reproducibility across
repeated runs" is not, on this evidence, the *originating* criterion. The Decision Point mechanism
was designed and implemented (2026-07-13) two days before the reproducibility sweep existed
(2026-07-15) and was validated by a citation-presence/absence comparison, not a multi-run variance
measurement. The Roadmap Reassessment does connect the two, but as a *symptom pointing at* the
underlying criterion (a genuine judgment call with no strong training-data default), not as an
independent criterion in its own right — worth being precise about, since the domain-event
question below has reproducibility evidence but not, so far, fabrication-with-no-basis evidence of
the same kind Policy Discovery had.

## 2. Domain-event determination against each criterion — side by side

The prompt (`prompts/spec.rs:154-159, 181-185`) actually bundles three distinct sub-decisions the
question separates correctly:

| Sub-decision | Downstream artifact depends on it? | Model has no basis / invents unsupported specifics? | Failure is silent (looks like a settled fact)? | Escape hatch exists / citation required? | Measured rate |
|---|---|---|---|---|---|
| **(a) Should a domain event exist at all?** | Yes — gates `EventShape`/`Publication` behaviors, contracts 007/008, and the real dependency edge Stage 6 produced | **No** — the rule is stated as unconditional: "MANDATORY whenever the Architecture Style ADR is event-driven and this story's action creates/updates/deletes an aggregate." There is a correct answer the model is told to always produce; it isn't being asked to judge whether one is warranted | Yes, in effect — a missing proposal is indistinguishable from "correctly decided no event is needed" unless someone checks against the mandatory rule | No escape hatch and no citation requirement exist for this — it isn't framed as a judgment call at all, mandatory or not | Present in 3/5 sweep runs — a **compliance** rate against a stated rule, not a resolved/fabricated/deferred rate |
| **(b) What should the event be called?** | Marginally — `ManufacturerCreated` vs. `ManufacturerRegistered` name the same contract shape | No — the prompt supplies a deterministic naming formula keyed to the classified operation (`creation → "<Entity>Created" or "<Entity>Registered"`); the model picks between two prompt-offered synonyms, it doesn't invent an ungrounded answer | No — the persisted `alternatives` field even records the road not taken (`ManufacturerCreated`) | N/A — not framed as needing one | Not separately measured by the sweep; this is a wording-level choice within its own category |
| **(c) Should it follow "`<EventName>` on topic `<topic>`"?** | Yes — gates whether `mechanical_event_behaviors` can produce anything at all | **No** — fully deterministic and conditional on an upstream fact: "when a Topic Naming Convention ADR exists... derive from it... if no such ADR exists, name the event only." The real project's own pre-spec state genuinely had no such ADR (`Existing Architecture Decisions: None yet.`), so omitting the clause was the textually correct behavior given that state, not an invented answer | Yes, sharply — this is exactly the failure mode already diagnosed twice (Contract Readiness Assessment, then the sweep): looks like a stale artifact until the sweep showed a *fresh* proposal, today, still omits it 4/5 times | No escape hatch exists; there's nothing to defer — the rule already has a deterministic fallback ("name the event only") | Convention-compliant in only 1 of 3 runs where the event appeared at all (1/5 overall) |

## 3. Evidence supporting "this looks like a Decision Point"

- The domain-event proposal is the single least reproducible output the sweep measured across
  every category — the strongest quantitative signal available, and larger than anything measured
  for tech stack, database, or naming.
- The Roadmap Reassessment already reasons, from the stable-categories-have-strong-training-
  defaults / unstable-category-doesn't observation, that this fits `unresolved-decisions-become-
  explicit-decision-points`'s shape — a genuine judgment call layered with a bespoke, project-
  specific convention with no external default to lean on. That reasoning is sound as far as it
  goes for *why the model is unstable here specifically* (no strong prior), which is a real
  precondition the Decision Point mechanism's target cases share.
- `human-insight-inventory.md`'s finding reinforces the "no record a choice was made" harm
  specifically: the real review gate accepted the domain-event proposal exactly as it would accept
  a stable, high-confidence one — no differentiated scrutiny, and Stage 2's own decision-point
  mechanism (`decisions.yaml`) never caught it either, because it operates on behaviors generated
  after `spec` closes, not on `spec`'s own proposals.
- `behavior-first-planning.md`'s named recurring category list explicitly includes **"event
  payload contents"** — evidence that *something* about domain events was already anticipated as
  Decision-Point-shaped, even though that phrase names a different sub-decision (what fields are
  inside an already-decided event) than the three evaluated here (whether/what/topic).

## 4. Evidence against it — the case for the other side, stated without strawmanning

- **Sub-decision (a) is a compliance gap against an unconditional rule, not an unresolved
  question.** The prompt does not ask the model to judge whether an event is warranted — it states
  the event is mandatory whenever the story's action is creation/update/deletion under an
  event-driven architecture ADR, full stop. A model failing to follow a clear, unconditional
  instruction 2 of 5 times looks like the shape of problem this project's own escalation-order
  principle addresses at tier 1/2 (a tool/fact lookup, or a prompt fix) — not tier 3 (a Decision
  Point is closer to a structural/gated-code response than a prompt fix, but the underlying failure
  here isn't "the model has no basis," it's "the model doesn't reliably comply with a basis it
  already has").
- **Sub-decision (c) has an already-identified, non-judgment-call explanation.** The missing topic
  clause traces to a real, checkable upstream fact — no Topic Naming Convention ADR existed at
  `spec` time — not to the model inventing an unsupported answer. This is closer to
  `compute-facts-mechanically`'s territory (a fact the pipeline should already know, and currently
  doesn't reliably have available at the right time) than to Policy Discovery's fabrication
  problem, where the model had access to no supporting fact at all and invented one anyway.
- **The sample is one story, reviewed once**, the same limitation `human-insight-inventory.md`
  and the sweep design doc both already state explicitly. Nothing here has been checked against a
  second story, let alone a second project.
- **The reproducibility sweep's own stated non-goal**: it isolates model-sampling variance only,
  by design, and explicitly does not test *why* the variance exists. Treating "low reproducibility"
  as proof of "genuine unresolved judgment call" skips exactly the step that sweep declined to take.

## 5. What kind of decision is this, actually?

**A. A business decision?** Weakly, at most. Stage 0 Policy Discovery's named business-policy
areas (uniqueness, defaults, retention, authorization, idempotency, consistency) are about business
*rules governing behavior*. "Should a domain event exist" touches business relevance (does another
context care about this state change) but the *specific* instability measured — existence
compliance and topic-clause formatting — is not itself a business rule question the way "how
should duplicate names be handled" is. Partially true, weakly.

**B. An architectural decision?** Most directly, yes. The question is raised inside
`identify_architectural_questions`, classified `Structural` in `pre-behavior-planning-review.md`'s
own Decision Classification table and again in `human-insight-inventory.md`. Two of its three
sub-parts (existence compliance, topic-clause sequencing) are best explained by architecture-
pipeline mechanics (instruction compliance, missing upstream fact) rather than business judgment.
Strongest fit of the four options.

**C. A consequence of an unstated domain-boundary decision?** Possibly, for sub-decision (a)
specifically, per `domain-boundary-hypothesis-assessment.md`'s own more speculative reasoning: an
event mostly matters if some other bounded context needs to react to it, and this project has never
had a second entity or consuming service to test that against. Genuinely undetermined either way —
not confirmed, not ruled out, because the precondition to check it has never occurred.

**D. Something else?** Yes, and arguably the best-evidenced answer for two of the three
sub-decisions: an **instruction-compliance gap** (a) and a **missing-upstream-fact/sequencing gap**
(c) — categories this project's own escalation-order framework already has names and remedies for
that are distinct from both "business decision" and "Decision Point." Multiple of these can be true
at once for different sub-decisions within the same nominal "domain-event ADR" — the three
sub-decisions do not have to share one classification.

## 6. Decision Readiness

**What we know:**
- The mechanism's actual originating criteria (§1), grounded in the design doc and the principle's
  own before/after evidence, not just its retrospective framing.
- A side-by-side comparison (§2) showing two of the three bundled sub-decisions have a concrete,
  already-identified non-judgment-call explanation (instruction compliance; missing upstream fact),
  and the third (naming) is a low-stakes wording choice with no invented content.
- The real review gate gave zero differentiated scrutiny to this category, and no downstream
  mechanism caught it either (`human-insight-inventory.md`).

**What we don't know:**
- Whether the model's inconsistent compliance with the "MANDATORY" event rule (a) reflects random
  noncompliance, or is quietly sensing a real judgment call ("does this operation actually matter
  to anyone else") that the rule's own unconditional framing suppresses. Nothing in the sweep's
  design (which deliberately isolates model-sampling variance only) can distinguish these two
  readings.
- Whether the topic-clause gap (c) recurs once the upstream Topic Naming Convention ADR reliably
  exists at `spec` time — i.e., whether fixing the sequencing gap (a `compute-facts-mechanically`-
  shaped fix, not a Decision Point) makes this instability disappear on its own.
- Whether any of this replicates on a second story or a second project — the entire evidence base
  is one story, reviewed once.

**Is the evidence already sufficient to make the determination?** No. The evidence is sufficient to
rule out treating this as a straightforward instance of Policy Discovery's fabrication pattern —
two of three sub-decisions have a more specific, better-fitting explanation already on record. It is
not sufficient to rule out sub-decision (a) being a genuine judgment call in disguise, because the
one experiment that could distinguish "compliance gap" from "suppressed judgment call" — testing
whether the sequencing/compliance fixes make the instability disappear, or whether it persists even
once the mechanical gaps are closed — has not been run.

## 7. Conclusion

**Too early to tell** — and not as a hedge. The evidence actively *disconfirms* the simplest version
of "this is the same problem as Policy Discovery" for two of the three bundled sub-decisions
(existence compliance and topic-clause formatting both have concrete, non-judgment-call
explanations already on record), which is itself a real, citable finding, not a non-answer. What
keeps this from resolving to "probably not" outright is sub-decision (a): whether inconsistent
compliance with the "always propose an event" rule is ordinary noncompliance or a hidden judgment
call the mandatory framing is currently suppressing is a genuinely open question this evidence base
cannot settle, on a sample of one story. Resolving it would need to observe what happens to
existence-compliance specifically once the mechanical/sequencing gaps are closed — not proposed
here, per the explicit scope of this comparison, only named as the specific missing piece.
