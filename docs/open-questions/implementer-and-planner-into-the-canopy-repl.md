---
title: Implementer capability, a standalone Planner, and how the canopy REPL changes
status: active
origin: the isolated Implementer reaching 10/10 on a five-step vertical slice (2026-10-10)
date_discovered: 2026-10-10
related_principles: []
related_narratives: []
related_reports: []
related_design_docs: [new-course-from-java-harness-and-nexus2.md, planner-first-increment.md]
---

# Question

The Implementer works in isolation against hand-written plans. What comes next, and in what order: more
Implementer capability, a Planner built and measured on its own, or the changes to the `canopy` REPL that
make either of them reachable?

# Why It Matters

Every plan so far is written by hand, and the hand-written text carries knowledge a small model would not
produce (which test inputs are invalid, how a test wires a service, the exact setup line). That is the
Planner's real job, and it is unbuilt. The REPL still runs the older pipeline (`init`, `intent`, `spec`,
`scaffold`, `implement`) with free-text YAML output and no gates. Until the Implementer is wired in, none of
the harness work is used by the product.

# Evidence So Far

- The Implementer passes a five-step vertical slice 10 of 10 on a 14B local model once the harness verifies
  and refuses (`docs/retrospectives/2026-10-10.md`, `canopy-implementer/docs/models/qwen3-14b-q4_k_m.md`).
- What made the difference was in the harness, not the prompt: the import audit, the scope contract, the
  restore-and-retry text. Prompt-side additions measured no gain (the up-front import lines).
- The plan had to state wiring and concrete values in every step that failed before it did. A Planner
  that does not will hand the Implementer the same failures.

# Options

1. **More Implementer capability.** Search/replace edits for large `modify` files (needs a repository with
   larger existing files; trigger criteria are in the retrospective), more JDK and library classes in the
   audit, a language registry (Java's rules are already in one module), another language (TypeScript is the
   likely second), a sceptic and an ImplementationReviewer, Cucumber tests written up front.
2. **A standalone Planner.** Story plus Gherkin in, a plan out, built and measured the way the Implementer
   was: a schema for the plan (a closed set of step kinds or recipes, slots for story-derived values), the
   repository explored with tools, facts computed by the harness (JDK version, classpath, existing test
   style), and a verifier before any model run (the plan check against the repository already exists).
   Probably one more model call than the Implementer uses, structured output with tools or a closing
   schema call. Open: whether a 14B model can fill the slots well enough, which only a Planner sweep shows.
3. **REPL integration.** `canopy-implementer` has no canopy dependencies on purpose, so `canopy-cli` can
   take it as a library when it is ready. The `implement` command would read a plan and run the orchestrator
   instead of the old generate-and-fix loop. Open: what happens to `spec`, ADR proposals, `scaffold` and the
   "Everything emerges" principle (a human-chosen stack profile and an up-front plan sit in tension with
   it), which provider configuration the REPL passes (local only once the Anthropic provider is deprecated),
   and how the REPL shows a run (the typed event stream is ready for it).

# What We Don't Know

- Whether a small model can plan well enough, or whether recipes and slot-filling are needed to reach the
  hand-written plans' quality.
- Whether the Implementer holds on other repositories, other stacks and larger files; every measurement so
  far is one Java stack and small files.
- Which REPL commands survive, and what a story's artifacts (`stories.yaml`, `spec.yaml`, `plan.yaml`)
  become.

# Suggested Order (a proposal, not a decision)

Planner on its own first, because it is the largest unbuilt piece and the one whose difficulty is least
known; keep the Implementer's open items (larger-files repository, a second language) as triggered work, not
scheduled work; design the REPL change once the Planner's output is a stable plan format, since that format
is what the REPL has to read and write.

# Exit Criteria

This entry graduates to a design doc when one of the three options starts as real work (mark
`status: active` there, link it here), and to a principle or narrative when it is answered with a
generalizable result.
