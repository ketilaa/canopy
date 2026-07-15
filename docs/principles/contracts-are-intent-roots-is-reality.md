---
title: "Contracts Are the Source of Intent; Roots Is the Source of Reality"
status: draft

confidence: medium

maturity: emerging

themes:
  - contract-driven-implementation
  - ai-assisted-code-generation
  - system-design

evidence_strength: medium

source_artifacts:
  - "docs/contract-readiness-assessment.md"
  - "docs/design/contract-driven-implementation-experiment.md — Stage 1 Results, Stage 2 Implementation and Results, Stage 5 Results, Stage 6"
  - "docs/design/roadmap-reassessment.md"

related_principles:
  - implementation-ownership-requires-full-file-scope-visibility
  - compute-facts-mechanically
  - structure-emerges-from-behavior

cluster: "Intent vs. Reality"
---

# Principle

When implementation needs to decide *what* to build, prefer an explicit, narrowly-scoped
statement of intent (a contract, or the behaviors it owns) over inferring intent from broader
repository context (a Roots-driven search or exploration). Reserve repository exploration for
grounding an already-decided intent in code that already exists — symbols, exports, call sites,
impact — not for discovering or substituting for the intent itself.

Working summary: **contracts are the source of intent; Roots is the source of reality.**
Implementation is `Intent + Reality → Code`, not `Reality → Infer Intent → Code`.

# Problem That Revealed It

Early in the contract-driven implementation investigation, it was plausible that implementation
would lean on Roots-driven discovery: search the repository, infer what needs to change, then
generate. The evidence pushed in the opposite direction. Repeatedly, generation quality improved
when the model was shown a narrowly-scoped, explicit statement of intent rather than broader
context — culminating in Stage 5's direct A/B test: the same real file, same story, same harness,
comparing production's fuller-context prompt (story + spec + scenarios + ADRs) against
contract-scoped generation (six narrow contracts, nothing else). Production failed 0/3; contracts
won 3/3 — and production's failures traced specifically to *invention* (unauthorized fields and
sibling classes, ad hoc constructor arities) driven by having more context available, not less.

# Evidence

- **Stage 1** (a single contract shown, no full-file visibility): 2 of 3 runs invented unauthorized
  JPA identity annotations belonging to a *different*, unshown contract — more surrounding
  plausibility (a familiar "JPA entity" shape) produced invention, not caution.
- **Stage 2** (full authorized scope restored — every contract sharing the file, still nothing
  beyond contracts): 3 of 3 clean, confirming the fix was showing the *right* scope, not a broader
  one; nothing Roots-shaped was added.
- **Stage 5** (contract-scoped vs. production's real, fuller-context prompt, same file/story/
  harness): 0/3 vs. 3/3 — decisively, contracts alone outperformed a prompt with substantially more
  context, including a full scenario list and architecture skill.
- **Stage 6**: the dependency edges that correctly drove a real, multi-file, dependency-aware plan
  came from contracts' own `dependencies` field — mechanically derived — not from a Roots symbol
  lookup or repository search.
- **Relationship to `implementation-ownership-requires-full-file-scope-visibility`**: that
  principle is about seeing the *full authorized scope* of one file (every contract sharing it,
  not a partial slice) — a claim about *completeness within the intent artifact*. This principle
  is about *where that intent should come from* in the first place — explicit contracts, not
  inferred repository context. The two are complementary, not overlapping: both push toward
  explicit, bounded intent as the fix for invention, from different angles.

# Counter-Evidence / Caveats

- Rests substantially on one A/B experiment (Stage 5), one entity, one story, one file — the same
  single-case generalization caveat `implementation-ownership-requires-full-file-scope-visibility`
  already carries. Rated `medium`, not `high`, for the same reason: a clean, decisive result,
  not yet reproduced across an independent case.
- **This does not mean Roots matters less.** It's an architectural clarification of Roots' role,
  not a demotion — the investigation didn't test Roots and find it wanting; it never put Roots in
  the intent-discovery role in the first place; this principle just makes that choice explicit
  rather than leaving it implicit.
- **Untested case**: every stage so far is greenfield-shaped — a new entity with no pre-existing
  implementation to reconcile with. Whether "contracts as sole intent source" still holds for
  *modifying* an existing system, where reconciling with what already exists is part of the
  intent itself, is genuinely untested. This is exactly the shape of case where Roots' grounding
  role and a contract's intent role would need to interact, and neither this principle nor the
  investigation behind it has exercised that interaction yet.

# Applicability

- **"What should be built?"** — answered by behaviors, contracts, and decision points. Do not
  substitute repository exploration for a missing intent artifact.
- **"What already exists?"** — answered by Roots: files, symbols, exports, references, call sites,
  impact analysis. Do not ask a contract to carry this; it was never designed to.
- Likely high-value future uses of Roots, following directly from this split: symbol discovery,
  impact analysis, dependency/change analysis, locating affected code, implementation grounding,
  and safe modification of existing systems — none of which is "deciding what to build."

# Confidence Assessment

Medium. One clean, causally isolated, decisive experiment (Stage 5's 0/3 vs. 3/3), conceptually
reinforced by Stages 1, 2, and 6, but not yet independently reproduced across a second entity,
story, or — most importantly for this specific principle — a case involving an existing
implementation Roots would need to ground against. The concrete next trial that would move this
from `medium`: the same contract-scoped-vs-fuller-context comparison, but for a *modify* operation
against a real pre-existing file, where Roots-derived reality and contract-derived intent both
have something to contribute and can be checked against each other.

# Generalization

The same shape — prefer an explicit, bounded statement of intent over inferring intent from
broader context — is worth watching for anywhere Canopy is tempted to widen a generation prompt
with "more repository context" to fix a generation-quality problem. This investigation's own
evidence suggests that impulse is often backwards: the fix for invention has repeatedly been a
narrower, more explicit input, not a broader one.
