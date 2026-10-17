# Prompt House Style (legacy, deprecated 2026-10-17)

**Status: deprecated as rules. Kept as recorded knowledge.** Everything below was derived on the
`canopy-llm` pipeline (`intent` → `spec` → `behaviors` → `implement`), mostly on Spring Boot repositories and
a reference model driven through long, multi-stage prompts. Some of it is backed by measurements (for
example the exhaustive-enumeration result and the paragraph-break compliance numbers); some of it is a
reaction to one bad run. None of it has been shown to hold for the standalone Planner and Implementer on a
plain repository, and it was written as MUST/NEVER rules that every new prompt had to follow.

Why it was retired: a 2026-10 experiment series on a plain-Java order story
(`docs/reports/order-plain-java.md`) found that prompts and schemas written under the house style did not
produce working plans or a reliable Implementer. Plan validity went from 4/10 to 10/10 under tightened schemas while the
Implementer pass rate stayed at 0 to 1 in 10, and the largest single Implementer bug was two rules that
contradicted each other, found by reading the prompt for contradictions and fixed by removing text, not by
adding any. The rules had become the default answer, closing off other solutions.

How to use this document now:

- Treat each rule as a **hypothesis with a stated origin**, to be tested against a measured downstream
  outcome (Implementer pass rate), one change at a time, in paired runs. A rule that survives a measurement
  is promoted into a principle in `docs/principles/` with its evidence.
- Nothing here is a review gate. `canopy-prompt-reviewer` is advisory only.
- Other evidence-graded notes stay valid as knowledge: `docs/principles/exhaustive-enumeration-over-holistic-review.md`,
  `docs/principles/deterministic-audits-vs-compensation.md`, `docs/principles/compute-facts-mechanically.md`.

The text below is unchanged from `CLAUDE.md` as it stood on 2026-10-16.

---

## Prompt House Style

Every string sent to the model — skills and prompts alike, across `canopy-llm/src/prompts/*.rs`
and `canopy-llm/src/skills/*.rs` — follows the same rules:

- **ALWAYS/NEVER, not paragraphs.** State a rule as `ALWAYS <imperative>.` / `NEVER <imperative>.`,
  not a multi-sentence explanation mixing the rule with its rationale. Short example fragments
  (1-4 lines of code) are encouraged — this is a "no restated rationale" rule, not a
  "no examples" rule.
- **No duplicate injection.** The same rule must not reach the model twice in one call, even
  worded differently — e.g. a rule stated once via a tech-stack skill's layer section and again
  in a separate IMPORTANT-list bullet in the same prompt. Trace every section a changed string
  feeds into before assuming it's the only copy.
- **Proximity.** A rule sits next to the content it governs. An instruction far from the thing
  it constrains is a known failure mode for the local reference model ("lost in the middle") —
  even a correct, well-worded rule gets ignored if it's positioned far from its subject.
- **Layer-scoping correctness.** A `layer_rules` entry must be keyed to the layer that actually
  needs it. A rule filed under the wrong layer key (e.g. a *route*-file rule accidentally placed
  under `"app"`) is correctly worded but never reaches the file it's meant to constrain — this
  bug produces no error, just silent non-compliance, since layer-scoped rendering only sends the
  matching layer's rules.
- **Prefer exhaustive enumeration over holistic review.** When a prompt asks the model to review
  a known, boundable set of items (fields, constraints, scenarios, behaviors, dependencies,
  clusters) for gaps, ALWAYS have it iterate the set explicitly rather than judge it holistically
  — a small model reliably misses items under open-ended review that it catches under explicit
  enumeration. Only fall back to holistic phrasing when no bounded set exists to enumerate.
  BAD:  "Review the specification and identify missing constraints."
  GOOD: "For each field: for each constraint: determine whether coverage exists."
  GOOD: "For each scenario: determine whether at least one behavior was produced."
  Confirmed live: Stage 0's original constraint audit ("review for gaps") found 4 of 9 real gaps;
  rewriting it as an explicit field × constraint traversal found all 9. The same shift fixed
  Stage 1 behavior extraction, dependency review, and clustering review.
- **Prompts teach; they do not enforce.** The local model cannot be trusted to follow a correct,
  well-placed instruction reliably (a paragraph break moved one compliance probability from 0.458
  to 0.058; three escalating prompt fixes for one failure failed identically). Verify with
  `llm-debug.log` that an instruction reached the prompt before blaming the model, but when
  something must hold, enforce it with the output type, the filesystem, the compiler or the test
  runner — see the escalation order under Principles. A prompt change needs a measured delta, not
  an impression; a longer WRONG/CORRECT example is a last resort. Still forbidden: Rust code that
  rewrites the *meaning* of model output (path injectors, semantic post-processors). Allowed and
  encouraged: **deterministic audits** (compare an artifact against an independently-known fact and
  reject), **restoring state** after a failed step, retrying with specific feedback, and
  **rendering text from validated fields**. The test: does the Rust code change what the model
  meant, or only accept, reject, restore or format it?

Use the `canopy-prompt-reviewer` subagent (`.claude/agents/canopy-prompt-reviewer.md`) to check
prompt/skill changes against these rules before installing.

---

## Structural placeholders instead of example entities (from the Tech-Stack Skills section)

**Prefer structural placeholders over example entities.** Default to bracketed placeholders —
`<entity>`, `<domain object>`, `<aggregate>`, `<event>` — rather than any concrete noun, even the
project's own established `Widget`/`createWidget` convention. Live-verified reason: the reference
model treats an example noun as a candidate answer, not just an illustration — a prompt showing
"register an account" produced literal `account`-vocabulary output for a story about registering
a manufacturer, and a later fix that swapped in `Widget` repeated three times in one example set
was flagged for recreating the same anchoring risk with a different noun. Any concrete noun is a
potential output token; a bracketed placeholder can't be copied literally because it isn't a real
word. Field names: prefer `<fieldName>` over `name`/`optionalField` for the same reason, unless
the concrete name is itself demonstrating a naming *convention* (see below).
Only use a concrete noun (`Widget`/`createWidget`, never a project-specific one like `Product`)
when the structure being taught genuinely requires an actual word — e.g. demonstrating a
camelCase-vs-PascalCase transformation, or a multi-field validation shape a bracket alone can't
convey. When in doubt, remove the noun. The pattern is established in the Models section of
`node_express_skill()` — that file predates this refinement and is a lower-priority candidate for
a future pass, not a template to copy into new work.
