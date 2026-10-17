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
  - "commit 1d0e3a4 — Wire canopy implement to consume contracts.yaml (Stage 4): contract-driven file discovery triggers on contracts.yaml's mechanical presence, not an opt-in flag"
  - "Planner plan-only sweeps, widgets, 2026-10-10 (docs/reports/widgets-planner.md): commits 05ba2e6 (step kind), 7999338 (create or modify), 99f1b8a (read-only files), 071aeb4 (storage line)"

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

- Commit `2ff1ae3`: an earlier, narrower instance of the same realization — instead of asking the
  model to decide whether a given gap finding should block downstream work, severity is computed
  deterministically from the gap's own already-known category ("MissingScenario/UnresolvedQuestion
  are always blocking, AmbiguousOutcome is always non-blocking... one less degree of freedom [the
  model] can get wrong, consistent with computing anything that has a mechanical answer instead of
  asking for it").
- Commit `98c1783`: a check comparing a generated entity name against already-established project
  vocabulary, computed entirely outside the model, added specifically because a live run showed the
  model's own generated entity could silently diverge from vocabulary already shown to it in the
  same context.
- Commit `ea3e1b9`: the sibling check for event names, same shape, same non-model computation.
- Commit `9061e34`: the most direct confirmation. "The prior instruction asked the model to scan a
  list and judge a match itself — exactly the [enumeration-over-holistic-review] failure mode this
  pipeline's own house style now explicitly calls out... [a mechanical function] computes the
  answer mechanically instead... The result is injected as a single... fact." A controlled
  reproducibility sweep after the fix showed 0 duplicates across 3 runs, down from the previously
  quantified ~2/3 incidence with the same underlying data available to the model either way.
- Commit `1d0e3a4` (2026-07-15): the same shape applied one level up, to a *routing* decision
  rather than a content-generation one. Whether `canopy implement` uses contract-driven file
  discovery or the legacy LLM-driven planner was designed, deliberately, as a check on a
  mechanical fact — does `contracts.yaml` exist for this story — rather than an explicit opt-in
  flag a human or a future session could simply forget to pass. The choice was explicit and
  discussed before implementation, not incidental: an opt-in flag was considered and rejected
  specifically because it "avoids mode drift caused by forgotten flags," the exact failure mode
  this principle already predicts for any judgment left to be re-made by hand instead of computed
  once from already-available data.

## Extension: fields of the model's output, not only questions (2026-10-10)

> A plan should describe intent, not bookkeeping. Do not ask the model for information the harness can
> derive deterministically.

The same principle holds for the *shape of what the model is asked to write*. Building the Planner
(story + Gherkin in, a plan for the Implementer out), each field the model was asked to fill that the
harness could work out failed at a measurable rate, and removing the field worked where instructing the
model did not:

- **Step kind** (`refactor` exempts a step from the test rule): in the first live run the model labelled
  all 8 steps `refactor`, so production code with no test passed the verifier. The harness now sets every
  drafted step to a behaviour change; a draft that names a kind is rejected (`05ba2e6`).
- **Create or modify** for each file: the model listed a file in both lists and did not track which file an
  earlier step had made; 9 of 10 batch-4 drafts failed on it, and retries naming the lists changed nothing.
  The harness decides from the repository and the earlier steps (`7999338`): valid plans went from 1/10 to
  4/10, 3 on the first draft.
- **Read-only files** a step depends on: the model listed the files it changes there too (38 of 38
  duplicates). The harness now gives each step every other production class its instruction names as a
  whole word (`99f1b8a`). Measurement in `docs/reports/widgets-planner.md`.
- **Which storage to use**: the Implementer wrote JPA classes the project does not have in 9 of 9 plans that
  reached an entity or repository step, with the dependency list in its context. A line computed from the
  classpath ("no database or JPA library is available; keep data in memory") and a prompt rule asking for
  the concrete mechanism went with JPA in 0 of the next 4 plans (n=1 and n=3, the two changes stacked, so
  not separable).
- The contrast that makes the rule useful: a plain instruction for something the model *should* decide did
  not hold either (every step carries its test: ignored in 8 of 10 drafts, retry included). There the shape
  carried it instead, a required non-empty `tests` field (0 missing tests afterwards). The model keeps
  intent: what to build, which tests prove it, which scenarios it serves.

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

A second limit from the Planner work: deriving a field can drop information the model had. The read-only
set is computed from the class names an instruction mentions, so a dependency the instruction does not name
is missed (the Implementer's own `glob` and `read_file` are the fallback), and the match is approximate
rather than an import graph. Derive only what is genuinely derivable, and say in the code what the
derivation does not see.

# Applicability

- Bookkeeping fields of a model-written artifact: kinds, scopes, create-or-modify, dependency lists

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

# Update 2026-10-17: a counter-example on "let the model act on facts"

The harness can compute a true fact mechanically and the model can still fail to act on it. Evidence from the
Implementer on plain Java (`canopy-implementer/docs/prompt-and-plan-changelog.md`, C8 to C10, one model, n=10 per cell):

- A computed retry hint ("`0` and `0.00` are the same number at different scales and `BigDecimal.equals` compares the
  scale") rescued the one-line order plan: 8 of 13 retries recovered, against 1 of 15 without it, and the share of
  retries identical to the failed answer fell from 14 of 15 to 5 of 13.
- The same hint on loans rescued 1 of 19 retries (identical share 9 of 19, against 20 of 20 without it); a variant naming
  which value the code returned and which the test expected rescued 0 of 20; the same fact stated up front in the
  project context left loans at 0 of 10 first attempts, while for order it cut the step's first-attempt failures from
  7 of 10 to 1 of 10.
- So the principle holds in its first half (computing the fact is cheap and correct) and is conditional in its second:
  whether the model acts on it depends on the mistake. The order failure was a forgotten `setScale`; the loans failure was
  a test and an early return that both used `BigDecimal.ZERO`, and no sentence about scale moved it. What did move it
  was a plan that listed the tests with their concrete values (loans 0 of 10 to 10 of 10). Evidence level: two stories,
  one library fact, one model; the confidence of the principle is unchanged.
