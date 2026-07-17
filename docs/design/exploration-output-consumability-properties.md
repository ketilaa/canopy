# What Makes an Exploration Finding Consumable Downstream?

Status: evidence synthesis only, extending `docs/design/exploration-phase-first-principles-
reassessment.md`'s surviving findings. No mechanism, UX, or stage proposed. Answers two narrow
questions: what characteristics made a supplied fact or surfaced finding operationally useful
downstream in this chain's evidence, and what characteristics made it disappear.

Date: 2026-07-17

---

# What Makes a Finding Operationally Useful Downstream

Grounded only in instances from the surviving findings where something was actually consumed —
cited in a resolved policy, reflected in a generated scenario, or otherwise measurably present in a
later artifact:

- **Contains something concrete enough to quote verbatim.** Every citation-backed consumption in
  this chain traces to a supplied fact containing a specific, nameable thing — "the original
  order/purchase confirmation number, verified against our records" (risk-averse, Phase 2); a role
  classification of exactly `internal`/`external` (Role Meaning Value Experiment). In each case the
  resulting `evidence` field lifts language recognizably close to the source. No instance of a
  fact stated only as a stance or priority — with nothing to point at — was ever cited this way.
- **Names a concrete mechanism, artifact, or action, not an abstract justification.** "Trigger an
  offer to exchange for a replacement or store credit" (growth-retention) produced scenarios
  mirroring that exact action. "Driven by external obligation, not internal preference"
  (compliance) named no mechanism at all and produced nothing traceable, twice, independently.
- **Maps onto a checklist area the pipeline already enumerates and resolves** (uniqueness,
  defaults, retention, authorization, idempotency, consistency). Every clean, citation-backed
  consumption in this chain landed inside one of these six areas. Nothing in the evidence shows a
  finding outside this fixed set ever being resolved and cited the same way — there is no observed
  instance of consumption for a concern the checklist doesn't already have a named slot for.
- **Reaches the pipeline through the same channel an already-accepted fact of that shape uses.**
  The Role Meaning Value Experiment's fact was injected through `existing_adrs`, the identical
  channel a real, accepted ADR already occupies — and was consumed. Nothing in this chain shows a
  fact reaching consumption through any other path.
- **Is scoped to one question, not bundled with several.** The single cleanest, most reliable
  citation result in the whole chain (Role Meaning Value Experiment) came from a fact narrowly
  about one thing — a role's identity. The broader, multi-concern facts used in Phases 2–3 (one ADR
  title covering verification *and* eligibility *and* rationale) produced inconsistent citation
  even for the personas whose facts were otherwise concrete — narrower framing correlates with
  more reliable consumption in every comparison this chain made.
- **Is supplied after a concrete story already exists, not before one.** Every consumed instance in
  this chain was anchored post-acceptance; this is a structural precondition established by
  `structure-emerges-from-behavior` and never violated by any experiment that produced a
  consumption result, so it cannot be separated from the other properties above as an independent
  variable — but its absence was never tested to produce a positive result either.

---

# What Makes a Finding Disappear

Grounded in the chain's own repeated non-results and pipeline-reliability findings — several
distinct causes, not one:

- **Principle-level framing with nothing concrete to quote.** The single most-replicated
  disappearance pattern in the entire chain — four independent instances (compliance's fact in
  Phase 2's own run, in the Role Meaning Value Experiment's `affiliated` condition, and twice more
  in Phase 3's two separate regenerations) all shared this shape and all left zero trace in any
  downstream artifact.
- **Bundling multiple concerns under one supplied item.** Reduces citation reliability even when
  the bundled fact contains genuinely concrete content — Phase 2's broader multi-concern facts
  were cited far less consistently than the Value Experiment's single-question fact, holding
  operational specificity roughly constant.
- **No existing enumerated slot for the concern at all.** Distinct from citation failure — this is
  structural absence, not miscitation. Role semantics and the story-vocabulary-vs-domain-registry
  discrepancy (the `Product`/`Order` forward-reference gap) both have no checklist area, Decision
  Point category, or any other existing mechanism that would ever surface them — they don't fail to
  be cited, they are never checked for at all, confirmed by direct code reading in both cases.
- **Pipeline mechanical failure unrelated to the finding's own content.** Phase 3's Findings #1–#3
  show a finding can disappear for reasons that have nothing to do with what was supplied or how
  well-shaped it was — Stage 1 scenario-derived behavior extraction failing to consume scenarios
  that were genuinely present destroyed downstream content regardless of any fact's quality. This is
  attrition from the pipeline's own mechanics, a separate failure category from anything about the
  finding itself.
- **Model-sampling variance across regeneration, independent of content.** The clearest single
  instance: Phase 2's original productId-vs-customerId structural divergence — which looked, at the
  time, like a real consumed effect — vanished under Phase 3's regeneration of nominally identical
  input. This means apparent presence in one run is not sufficient evidence of durable consumption;
  some of what looks like a finding "surviving" is itself sampling noise that happened to align with
  the fact's content once, not proof the fact caused it.
