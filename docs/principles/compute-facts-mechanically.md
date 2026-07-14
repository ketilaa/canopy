---
title: "Compute Facts Mechanically; Let the Model Act on Facts"
status: draft

confidence: high

maturity: validated

themes:
  - prompt-engineering
  - ai-assisted-verification
  - system-design

evidence_strength: high

source_artifacts:
  - "commit 4a728a6 — Compute gap severity from kind instead of asking the model to judge it"
  - "commit a254b25 — Add Entity Continuity gate and fix the want-field example that anchored on it"
  - "commit 0dd7073 — Add Event Continuity gate, the sibling check to Entity Continuity"
  - "commit 390b7f6 — Compute domain-event ADR existence as a fact instead of asking the model to check"
  - "reproducibility sweep, manufacturer-001, 2026-07-14 (0/6 duplicate ADRs across 3 runs, down from ~2/3 incidence)"

related_principles:
  - deterministic-audits-vs-compensation
  - cross-artifact-consistency-audits-prevent-drift
  - exhaustive-enumeration-over-holistic-review
cluster: "Compute, Don't Ask"
---

# Principle

When a question the model is being asked has a mechanically-computable answer from data already
available to the system, compute that answer in ordinary code and give it to the model as a
stated fact — do not ask the model to derive it by reading and judging the same data itself.
Reserve the model's own judgment for questions that are genuinely open-ended or require synthesis
the system cannot compute directly.

# Problem That Revealed It

A recurring shape of bug kept appearing across otherwise-unrelated parts of the same pipeline:
the model was being asked to answer a question that was, in principle, fully determined by data it
had already been shown — "does this already exist," "how severe is this," "does this decision
conflict with an earlier one" — and it kept getting the answer wrong at a measurable, non-trivial
rate, even with the relevant data present verbatim in its own context. The clearest instance: a
step asked the model to check whether a specific kind of decision record already existed for the
current story before proposing a new one, with the existing record shown to it directly in the
prompt. A reproducibility sweep quantified the resulting bug at roughly 2 out of 3 runs producing
a duplicate anyway.

# Evidence

- Commit `4a728a6`: an earlier, narrower instance of the same realization — instead of asking the
  model to decide whether a given gap finding should block downstream work, severity is computed
  deterministically from the gap's own already-known category ("MissingScenario/UnresolvedQuestion
  are always blocking, AmbiguousOutcome is always non-blocking... one less degree of freedom [the
  model] can get wrong, consistent with computing anything that has a mechanical answer instead of
  asking for it").
- Commit `a254b25`: a check comparing a generated entity name against already-established project
  vocabulary, computed entirely outside the model, added specifically because a live run showed the
  model's own generated entity could silently diverge from vocabulary already shown to it in the
  same context.
- Commit `0dd7073`: the sibling check for event names, same shape, same non-model computation.
- Commit `390b7f6`: the most direct confirmation. "The prior instruction asked the model to scan a
  list and judge a match itself — exactly the [enumeration-over-holistic-review] failure mode this
  pipeline's own house style now explicitly calls out... [a mechanical function] computes the
  answer mechanically instead... The result is injected as a single... fact." A controlled
  reproducibility sweep after the fix showed 0 duplicates across 3 runs, down from the previously
  quantified ~2/3 incidence with the same underlying data available to the model either way.

# Counter-Evidence

The fact-computation approach is not free of its own correctness risk, and evidence from the same
fix shows this clearly: the mechanical function that computes "does this decision already exist"
went through four review rounds, each catching a genuine bug in the computation logic itself — an
operation-blind match that could wrongly suppress a legitimately different new decision, a
case-sensitivity gap that could silently reintroduce the original failure, a substring-matching
false positive, and a duplicated restatement of a rule the fact string already carried. This is
not evidence against the principle — computing the fact mechanically and getting it right on the
first attempt are different claims — but it is evidence that "compute it mechanically" shifts the
correctness burden onto the code doing the computing, and that code needs the same scrutiny (tests,
review, edge-case analysis) the original prompt would have needed, rather than being treated as
automatically safe just because it isn't a model call.

# Applicability

- Existence and duplication checks ("has this already been decided/created/proposed")
- Severity and priority classification derived from an already-known category
- Cross-referencing a new artifact against already-established project state
- Any judgment where the inputs are fully known to the system before the model is asked

# Confidence Assessment

High. Every instance is backed by either a direct before/after measurement (the 4a728a6 severity
change, framed against the cost asymmetry of missed vs. over-flagged gaps) or a reproducibility
sweep with a specific quantified incidence rate before the fix and a specific measured rate after
(0/6 duplicates across 3 runs, down from ~2/3). The pattern reproduced independently across at
least four distinct problems in the same system (gap severity, entity continuity, event
continuity, decision-existence checking) rather than resting on a single case.

# Generalization

Broadly applicable outside this project. Any system that combines a language model with structured
application state faces the same choice repeatedly: ask the model to re-derive something the system
already knows, or compute it and hand it over. The general failure this principle addresses — a
model misjudging a fact that was fully present in its own context — is a known reliability
limitation of current language models, not specific to this project's domain. Systems doing
retrieval-augmented generation, agentic tool use, or any AI-assisted workflow layered on top of a
real database or document store face this exact tradeoff, and the same fix applies: push
mechanically-derivable facts out of the prompt-and-judge loop and into ordinary computation.

# Future Validation

The clearest gap in current evidence is measuring how the *correctness of the mechanical
computation itself* trends over time and across new cases — the four-round review history on the
most recent instance suggests these computations are not trivial to get right on a first pass, and
a systematic count of how many mechanical-fact functions across the project needed post-hoc
correctness fixes (versus how many prompt-only fixes needed the same) would sharpen how much net
reliability gain this pattern produces once the cost of getting the computation right is included.
