---
title: "Reserve the Model for Genuine Ambiguity; Go Deterministic Once a Mapping Is Enumerable"
status: draft

confidence: high

maturity: validated

themes:
  - system-design
  - ai-assisted-code-generation
  - determinism

evidence_strength: high

source_artifacts:
  - "commit 1c759bf — Replace LLM scaffold generation with static Rust templates (2026-06-22)"
  - "13+ scaffold hallucination bug-fix commits, 2026-06-21 to 2026-06-22"
  - "commit 8cfabf7 — Add TDD loop, testing skills, validation constraints (2026-06-29 to 07-01), test-file freezing"
  - "docs/blog-drafts/we-deleted-the-llm-call-and-replaced-it-with-a-template.md"

related_principles:
  - compute-facts-mechanically
  - exhaustive-enumeration-over-holistic-review
cluster: "Compute, Don't Ask"
---

# Principle

When the mapping from a task's input to its correct output is fully enumerable — the same input
always has the same correct answer — replace a language-model call with deterministic code, even if
the model is technically capable of producing that output most of the time. Reserve model calls for
tasks with genuine, input-dependent ambiguity that a fixed rule or lookup table cannot resolve.

# Problem That Revealed It

Early in the project, turning a decided software architecture into the shell commands that
scaffold it (`npm create vite`, `ng new`, a Spring Initializr invocation) was built as a model call,
on the same reasoning that made architecture derivation itself a model call: "given X, produce Y."
Over two days, at least 13 separate bugs surfaced — an invented Vite template name, a fabricated
`npm init` package that has never existed, wrong CLI flag ordering, a Maven archetype version
mismatched to what Spring Initializr actually serves, and more. Each bug got its own patch. None of
the patches addressed why the hallucinations kept happening in different forms, because the
underlying task itself — reproducing a real external tool's exact syntax for an already-fully-known
input — never had any real ambiguity in it to begin with.

# Evidence

- 13+ distinct scaffold-command bug-fix commits across 2026-06-21 and 2026-06-22, spanning at least
  5 different scaffold targets (Vite, Angular CLI, Spring Initializr, Node/npm), each fixing a
  different specific hallucinated or malformed invocation.
- The resolving commit's own stated rationale (`c4c7035`): "Scaffold commands are fully
  deterministic given component type — the LLM added latency, cost, and hallucination risk. Replace
  with static templates." Architecture derivation — a genuinely underdetermined task — was
  deliberately *not* touched by this change and, in the same period, was made more flexible
  (schema-free) rather than more constrained.
- A second, independent instance of the same underlying test: `f1d7d65`'s TDD test-file freezing.
  Once a test file is established as the specification for a behavior, whether the implementation
  satisfies it is a deterministic, checkable fact (does the test pass), not something requiring the
  model's ongoing discretion — the fix protected the test from being edited during the fix loop
  rather than trusting the model not to "fix" the test instead of the implementation.

# Counter-Evidence

The static-template replacement was not bug-free — `dfc05fe`, the commit immediately following the
rewrite, fixed three new bugs in the deterministic templates themselves (a Vite prompt-handling
issue, a Spring Boot line-continuation problem, incorrect Node `--prefix` usage). This is not
evidence against the principle — it shows the trade was real, not costless: replacing generation
with code moves the burden from "hallucination risk, unpredictable" to "template correctness,
ordinary code bugs, fixable once and stays fixed" — a better trade, but a trade, not a free win. No
evidence has been found of a case where an enumerable mapping was *correctly* left as a model call
and continued to perform acceptably over time; every enumerable-mapping case observed eventually
accumulated enough bug volume to justify the switch.

# Applicability

- Any step that turns an already-fully-decided input into a fixed, correct output (scaffold
  commands, boilerplate generation, config-file assembly from known values)
- Deciding whether an already-established artifact (a passing test, an accepted decision) should be
  re-derived by the model or simply checked against
- Pipeline design generally: before building a "given X, produce Y" step as a model call, checking
  whether Y is actually underdetermined by X or whether a lookup table is being approximated

# Confidence Assessment

High. The principle is grounded in a directly stated rationale from the team that made the change,
a clearly quantified bug pattern before the fix (13+ commits, at least 5 distinct external tools)
and a much smaller, qualitatively different bug pattern after it (3 ordinary template bugs, not
hallucinations), plus an independent second instance (TDD test-freezing) arrived at separately and
for a different immediate reason, which strengthens confidence this is a real, generalizable
distinction rather than a one-off judgment call specific to scaffolding.

# Generalization

Broadly applicable to any AI-assisted system chaining multiple "transform this into that" steps.
The distinguishing test generalizes cleanly: does the same input always have the same correct
output? If yes, and that mapping can be written down, it doesn't need a model — the model call adds
latency, cost, and a failure mode (confident-but-wrong output) that a lookup table or template
simply cannot have. Code-generation tools, infrastructure-as-code generators, and any system
wrapping a real external tool's exact CLI or config syntax are prime candidates for this same
audit.

# Future Validation

There isn't yet a proactive test for "is this mapping actually enumerable" — every instance found so
far was recognized only after bug volume made it obvious in hindsight. A useful next step would be
a lightweight pre-build checklist applied before any new "given X, produce Y" step is built as a
model call: can Y be derived from X by a fixed rule, without needing judgment that varies by
context? If yes, build the lookup table first and skip the model call entirely, rather than waiting
to discover the same pattern through another bug-fix cluster.
