# Exploration Phase — First-Principles Reassessment

Status: evidence reassessment only. No mechanism, workflow, UX, prompt, stage, or feature is
proposed anywhere in this document. Answers what today's pre-behavior exploration phase is
*actually for*, evaluated against the full chain of evidence this investigation has produced,
before any question of how it should work is asked again.

Date: 2026-07-17

Reviewed, as the full evidence base: `docs/design/human-insight-inventory.md`, `docs/design/
product-owner-perspective-experiment.md`, `docs/design/exploration-enumeration-gap-investigation.
md`, `docs/design/role-semantics-investigation.md`, `docs/design/role-classification-stability-
test.md`, `docs/design/canopy-assisted-domain-exploration-vision.md`, `docs/design/domain-
exploration-mvp-design.md`, `docs/design/role-meaning-value-validation.md`, `docs/design/role-
meaning-value-experiment-results.md`, `docs/design/human-insight-process-experiment-design.md`,
`docs/design/human-insight-process-experiment-phase2-results.md`, `docs/design/human-insight-
process-experiment-phase3-results.md`, plus the adjacent, earlier evidence this chain built on:
`docs/design/pre-behavior-planning-review.md`, `docs/design/pre-behavior-planning-
reproducibility-sweep.md`, `docs/design/domain-boundary-hypothesis-assessment.md`, `docs/open-
questions/{domain-boundary-explicitness,role-semantics-explicitness}.md`, `docs/design/
unestablished-referent-hypothesis-review.md`, `docs/design/domain-event-decision-point-criteria-
comparison.md`, `docs/principles/{structure-emerges-from-behavior,unresolved-decisions-become-
explicit-decision-points,exhaustive-enumeration-over-holistic-review}.md`.

---

# Original Assumptions

Made explicit, each grounded in where it actually shows up in the current pipeline or its design
history — not invented for this document:

1. **Behavior-first discovery.** Structure (domain vocabulary, roles, architecture) should emerge
   from a concrete, already-accepted behavioral statement, never be elicited abstractly beforehand.
   Grounded in `structure-emerges-from-behavior` (high confidence, validated).
2. **Minimal upfront questioning.** `init` asks one free-text question plus a short wizard;
   `explore`'s own original clarifying questions were removed for "adding friction without value."
   Same principle, same evidence base.
3. **A correctable-but-optional bootstrap suggestion is the one sanctioned exception to (2).**
   `init`'s `Role::Described`/`DomainEntity::Described` bootstrap MultiSelect — LLM-suggested,
   human-curated, opt-out — was reintroduced after fully emergent elicitation was tried and found
   to have its own cost. Same principle's own counter-evidence.
4. **Automatic domain/role registration, no human gate.** `intent`'s per-story role registration
   and domain extraction proceed with zero review step — classified `Implicit Decision` in
   `pre-behavior-planning-review.md`'s own Decision Classification table from the start.
5. **Business-policy discovery belongs late, inside `spec`, with a citation-enforced escape hatch.**
   The business-policy checklist (`entity_schema_prompt`) and its citation requirement
   (`unresolved-decisions-become-explicit-decision-points`) resolve six fixed, enumerated areas —
   uniqueness, defaults, retention, authorization, idempotency, consistency — at spec-generation
   time, not earlier.
6. **Decision Points are a narrow, specific-shape mechanism.** Stage 2 only fires for a
   business-policy question already recognized as unresolved from already-extracted behaviors — not
   a general catch-all for any kind of unresolved meaning.
7. **The existing Accept/Modify/Reject review gates constitute adequate human oversight.** Implicit
   in the pipeline's original design: a human reviewing each ADR/story/decision is assumed to
   supply whatever judgment the model can't.

---

# Assumptions Confirmed

Only claims with direct evidentiary support from this chain, cited specifically:

- **(1) and (2) — behavior-first, minimal upfront elicitation.** Directly reconfirmed, not merely
  left unchallenged: every experiment in this chain (Role Meaning Value Experiment, the Human
  Insight Process Experiment's Phases 2–3) deliberately anchored its fact-injection to an
  *already-accepted* real story, never attempted upfront. `domain-boundary-hypothesis-assessment.md`
  independently used this same principle to explain — correctly, per direct code reading — *why*
  `Product`/`Order` are never extracted from a story's own purpose clause: a deliberate design
  choice with real supporting evidence, not an oversight.
- **(5) — the citation-enforced policy-resolution mechanism generalizes beyond the model's own
  reasoning.** This is *new* confirmation, not a restatement: the Role Meaning Value Experiment
  showed the same mechanism (`authorization`'s `resolved`/`unresolved` classification, gated on a
  named, checkable `evidence` field) correctly consumes a *human-supplied* fact injected through
  the same `existing_adrs` channel an ADR already uses — moving `authorization` from a correctly
  unresolved baseline to a citation-backed `resolved` in 2 of 3 tested conditions. The mechanism was
  validated for the model's own fabrication problem; this chain is the first evidence it holds for
  externally-supplied facts too.
- **A narrow structural claim from the Enumeration Gap Investigation**: uniqueness and
  authorization are *already* enumerated, working checklist areas in current code — confirmed by
  direct reading of `entity_schema_prompt`, not inferred from output. This rules out, rather than
  confirms, a candidate gap the Product-Owner Perspective Experiment's raw findings had seemed to
  suggest.

---

# Assumptions Weakened

- **(4) — automatic role/domain registration with no gate is a low-risk implicit decision.**
  Weakened specifically, not generally: the Role Semantics Investigation found a real, structural
  asymmetry between two code paths writing to the same `RolesRegistry` — `init`'s bootstrap path
  has a working, human-facing description-capture channel (`Role::Described`); `intent`'s automatic
  per-story path, the *only* path that has ever populated a role in this project's real history,
  never reaches it. The "implicit decision, low stakes" framing understates this — it's an
  inconsistency between two paths that both exist for the same artifact, not a deliberate scope
  choice.
- **(7) — existing review gates constitute adequate oversight.** Weakened on two independent
  fronts: the Human-Insight Inventory found the real review gate gave zero differentiated scrutiny
  across reproducibility tiers in the one real session studied (a tier-4/least-reproducible
  proposal reviewed identically to a tier-1/most-stable one); and separately, the Human Insight
  Process Experiment (Phases 2–3) found that even when a human *does* supply distinguishing
  content through the one channel available to them, whether it's consumed at all depends on a
  variable (operational specificity) the review gate itself has no way to signal or enforce.
- **(6) — Decision Points are a clean, narrow-shape mechanism, isolated from other pipeline
  stages.** Weakened by a mechanism-level finding, not a scope finding: Phase 3 showed Stage 2's
  own trigger condition is not robust to upstream corruption — a Stage 1 extraction failure
  (unrelated to any business ambiguity) produced a fabricated, off-topic Decision Point, twice,
  mechanically confirmed as downstream of the extraction failure rather than an independent defect.
  The mechanism's *intended* scope is unchanged; its *actual* trigger surface is broader and less
  reliable than assumed.
- **A hypothesis this chain itself raised and later retracted, worth naming as explicitly
  weakened rather than omitted**: the "unestablished referent" synthesis (a single shared class
  covering role semantics, duplicate-name handling, and the Product-relationship gap) does not
  survive its own counter-evidence review — two of its three instances turned out to be better
  explained by an existing principle (`unresolved-decisions-become-explicit-decision-points`) or an
  existing enumeration-coverage gap, not a new class.

---

# Surviving Findings

Restricted, per explicit instruction, to findings that were never explained away and never failed
replication — each checked against that bar individually:

1. **The role-semantics structural gap** (Role Semantics Investigation). Never contradicted by any
   later investigation; not yet independently re-tested against a second role, but the underlying
   code-path asymmetry it describes is a fact about current code, not a sample-dependent claim.
2. **The forward-reference/domain-boundary gap, now replicated across two independent stories.**
   First found on `manufacturer-001` (Product-Owner Perspective Experiment): the story's own
   `so_that` names `Product`, never extracted. Recurred, unprompted, on `order-001` during Phase 3
   setup — a different story, different domain content, same gap. This is the one finding in the
   chain with genuine cross-story replication, not just repeated observation of the same instance.
3. **Uniqueness and authorization are already enumerated in current code** (Enumeration Gap
   Investigation). A negative-but-solid finding — it survives because it was never contradicted,
   and it correctly ruled out two candidate gaps rather than merely asserting an absence.
4. **The operational-specificity-vs-principle-level-guidance pattern.** The single most-replicated
   finding in the entire chain — observed independently four times, across two different methods:
   Phase 2's risk-averse/growth-retention facts (operational, traced cleanly) vs. compliance's fact
   (principle-level, left no trace) in one uncontrolled real session; the Role Meaning Value
   Experiment's controlled, four-condition comparison (a narrow role-identity fact resolved
   `authorization` citably); and Phase 3's *second*, independent instance of compliance's
   principle-level fact leaving no trace, in a completely separate regeneration run. Never
   contradicted; never elevated to a principle either, consistent with this project's own bar for
   that promotion.
5. **The review gate gives no differentiated scrutiny signal** (Human-Insight Inventory). Survives
   because never contradicted — but scoped honestly: one session, one story, not independently
   re-tested elsewhere in this chain the way finding #2 was.
6. **Stage 0/Stage 1 pipeline-reliability defects** (Phase 3, Findings #1–#3). Reproducible within
   Phase 3 itself (Finding #1: 4/4 runs; Finding #2: 2/2 on one branch, 0/1 on the other, reported
   at that precision) and mechanically traced to a specific causal chain, not merely observed.
   Never contradicted. Kept in their own bucket throughout, per explicit instruction, but they are
   real, surviving findings about the pipeline's own mechanics, independent of any persona-meaning
   question.

**Explicitly excluded**, per the instruction not to include anything explained away or failed to
replicate: the "unestablished referent" shared-class hypothesis (retracted by its own counter-
evidence review); the specific `manufacturer-001`-shape customer-identity-vs-product-identity
structural divergence from Phase 2 (did not survive Phase 3's regeneration — inverted, not merely
weakened); domain-event recommendation instability as an independently new phenomenon (reassessed
as "too early to tell," not a confirmed, standalone finding).

---

# Strongest Evidence

Ranked by evidence quality — methodological rigor and replication — not by how consequential each
finding feels:

1. **The operational-specificity pattern** (surviving finding #4). Ranked first because it is the
   only finding in the whole chain independently confirmed by *two different methods* (a
   controlled, single-variable four-condition experiment, and repeated observation across
   uncontrolled real sessions) without a single disconfirming instance across four separate checks.
2. **The Role Meaning Value Experiment's controlled comparison** (part of the same pattern, listed
   separately for its method). The single cleanest mechanistic result in the chain: same story, same
   unmodified production code, only the injected context varied, with the causal link visible
   directly in the artifact's own `evidence` field quoting the injected fact.
3. **The forward-reference gap's cross-story replication** (surviving finding #2). Strong because
   the second instance was never fished for — it surfaced as an incidental observation during Phase
   3's unrelated setup work, which is a stronger form of replication than a deliberately repeated
   test.
4. **The Enumeration Gap Investigation's code-level ruling-out of uniqueness/authorization**.
   Strong because it is grounded in direct source reading, not inference from model output, and its
   result is a correction of an earlier, weaker inference (the Product-Owner Perspective
   Experiment's raw findings) rather than a first guess confirmed.
5. **Phase 3's Finding #1** (Stage 0 false positives). Strong — 4 independent reproductions against
   directly-verified-correct content, with a specific, falsifiable root-cause hypothesis actually
   tested (candidate-pointer correctness checked directly against the real scenario listing each
   time).
6. **The role-semantics structural gap** (surviving finding #1). Solid — grounded in direct code
   reading — but weaker than the above because it has only ever been checked against one real role
   in one real project's history.
7. **The review-gate scrutiny finding** (surviving finding #5). Weakest of the surviving set:
   real, never contradicted, but a single session, never independently re-tested.

---

# Jobs Exploration Appears Responsible For

Stated as *what problem*, not *how to solve it* — derived only from the surviving findings above:

- **Confirming what an actor/role label actually denotes, specifically for the code path that
  currently never asks** — grounded in surviving finding #1, the one gap with no competing
  explanation anywhere in this chain.
- **Eliciting facts specific and operational enough to be citable, not broad instinct-level
  judgment** — grounded directly in surviving finding #4, the chain's single strongest and most-
  replicated result. This is a constraint on *what kind of input* exploration is responsible for
  producing, not a mechanism.
- **Surfacing a discrepancy between a story's own stated language and what the domain vocabulary
  already contains** — grounded in surviving finding #2's cross-story replication. Stated as
  *surfacing*, not *resolving*: nothing in the evidence supports exploration being responsible for
  deciding what to do about the discrepancy, only for making it visible.
- **Feeding whatever it elicits into the same citation/policy-resolution mechanism that already
  works**, rather than any parallel channel — grounded in the Role Meaning Value Experiment's own
  method (the fact was injected through the existing `existing_adrs` channel, not a new one, and
  that is exactly what let it reach a working, already-validated mechanism).

# Jobs Exploration Should Not Perform

Grounded in `structure-emerges-from-behavior`, this chain's own failed/discarded hypotheses, and its
explicitly named anticipatory-modeling risks:

- **Should not run ahead of, or independent from, a concrete accepted story.** Direct consequence
  of `structure-emerges-from-behavior`'s own validated evidence and its own counter-evidence limit
  (a correctable *suggestion* at bootstrap time survives scrutiny; abstract upfront *questioning*
  does not).
- **Should not attempt to resolve or anticipate cross-story/whole-domain relationships ahead of
  real behavior calling for it.** `domain-boundary-explicitness.md` explicitly declined to
  manufacture a second entity purely to test its own hypothesis, twice, for exactly this reason —
  a standing, evidenced discipline this chain has already applied to itself, not a new constraint
  being introduced here.
- **Should not re-litigate concerns already enumerated and already functioning** (uniqueness,
  authorization). The Enumeration Gap Investigation's own finding rules this out directly — building
  new capability here would duplicate a mechanism already shown to work, the same "same rule
  reaching the model twice" anti-pattern this project's own house style already names as a defect.
- **Should not rely on broad, principle-level judgment prompts as a reliable elicitation shape.**
  This is a discarded concept, not merely a deprioritized one — it failed independently four
  separate times across two different methods (surviving finding #4), the strongest negative result
  in the whole chain.
- **Should not be validated using simulated multi-persona review as a stand-in for independent
  human evidence.** The "unestablished referent" retraction exists specifically because this chain
  once conflated the two; the Domain Exploration Vision's own §Solo And Team Usage names the same
  caution independently. Applies to evaluating exploration's own eventual output too, not only to
  past experiments.

---

# Updated Mental Model

**Before this chain**, the implicit working model was roughly: "Canopy generates boilerplate well;
humans naturally supply judgment at the existing review gates; if something is missing, it's
probably a general elicitation gap — ask more, or ask earlier, and meaning will follow." This was
never stated this baldly anywhere, but it's the assumption the Product-Owner Perspective Experiment
was designed to test, and the assumption the original Domain Exploration Vision was reaching toward
before this reassessment.

**What the evidence now supports is sharper and narrower than that**: humans do not reliably supply
useful meaning just because a gate exists (Human-Insight Inventory; the review-gate finding above).
When meaning *is* supplied, whether it reaches the pipeline at all is governed by a specific,
now four-times-replicated property — operational specificity — not by the presence of a human, a
persona, or a review step in general. Some of what looked like missing elicitation capability turned
out, on direct code inspection, to already exist and already work (uniqueness, authorization) — the
original problem space was partly built on incomplete knowledge of the current pipeline, not a real
gap. And a previously invisible confound — the pipeline's own Stage 0/Stage 1 mechanical
reliability — is now directly evidenced and must be held separately from any future "does meaning
survive" question, since it can produce or destroy apparent divergence for reasons having nothing to
do with meaning at all.

The clearest standing target, unchanged in status across the entire chain and never explained away,
remains the role-semantics gap — but even that must now be understood alongside the specificity
constraint: any future elicitation aimed at it would need to be shaped narrowly and operationally,
not as an open-ended "tell us what this role means" prompt, or it risks reproducing exactly the
non-result this chain already measured four times.

---

# Open Questions

Left genuinely open, not converted into a design task here:

- Whether the specificity pattern holds outside business-policy resolution specifically — every
  confirming instance in this chain involved the same underlying mechanism (the citation-enforced
  policy checklist). Untested against, say, entity-schema field selection or scenario generation
  directly.
- Whether the role-semantics gap would replicate against a second real role — still, after this
  entire chain, checked against exactly one.
- Whether Stage 0/Stage 1's reliability defects (Findings #1–#3) are specific to this story's
  content and length, or a more general property of this pipeline stage under the local model in
  use — Phase 3 tested one story, `return-001`, and one branch each.
- Whether the review-gate scrutiny finding (surviving finding #5) would look different on a story
  that actually exercised a Modify or Reject action — `manufacturer-001`'s real history never has,
  and this chain has not tested a second story's review history either.
