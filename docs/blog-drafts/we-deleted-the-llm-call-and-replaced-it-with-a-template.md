---
title: "We Deleted the LLM Call and Replaced It With a Static Template"
date: 2026-07-14
status: draft

learning_type:
  - design-evolution
  - failure-analysis

topics:
  - ai-assisted-code-generation
  - determinism
  - small-language-models

key_principles:
  - "Reserve the model for genuine interpretive ambiguity; once a mapping from input to output is fully enumerable, replace generation with deterministic code."

source_artifacts:
  - "commit 1c759bf — Replace LLM scaffold generation with static Rust templates"
  - "13+ scaffold bug-fix commits, 2026-06-21 to 2026-06-22"
  - "docs/retrospectives/2026-06-21-to-06-22-reconstructed.md"

story_ids: []

evidence_strength: high

commits:
  - 1c759bf

initial_assumption: >
  Turning a decided software architecture into the actual shell commands to scaffold it — npm
  init, Spring Initializr, ng new, and their many flags — was itself a task worth asking a
  language model to do, the same way architecture derivation itself was.

final_understanding: >
  Two tasks that looked similar (both "given an architecture, produce something concrete") had
  completely different amounts of real ambiguity in them. Deriving architecture from a story is
  genuinely underdetermined. Turning a known component type into its scaffold command is a lookup
  table. Asking a model to do the second one bought nothing but hallucination risk.
cluster: "Compute, Don't Ask"
---

# Summary

In two days, we fixed 13 separate scaffold-generation bugs. A non-existent Vite template. An
invented `npm init` package that doesn't exist on the registry. Wrong Angular CLI flag order. A
Maven archetype version that doesn't match what Spring Initializr actually serves. Each fix patched
one specific hallucination. None of them addressed why the model kept hallucinating in the first
place — because the task itself didn't need a model at all.

# Original Assumption

We'd built architecture derivation as an LLM task, and it made sense: given a vague idea, deciding
what services exist and how they relate is genuinely underdetermined — a model earns its keep
there. Scaffolding — turning that decided architecture into the actual commands to generate a
runnable project skeleton — got built the same way, on the same assumption: give the model the
architecture, let it produce the right `npm create vite`, `ng new`, or Spring Initializr
invocation.

# What Happened

The bugs came fast and from every direction. `91cb0bc`: the model invented a Vite template name
that doesn't exist and got the flag order wrong. `99a17ea`: it invented an `npm init` scaffolder —
"express-auth-service" — that has never existed on the npm registry. `ae72758`: the Angular
template it picked can't actually be scripted the way it assumed. `38831a0`: wrong Maven archetype
version, wrong Spring Boot initializer source. `15484ab`, `eaa6212`, `5dddd78`, `254a5d4`,
`7e5f237`, `aa04ecd`, `e4311d6`, `9dc84b7`, `78f51e5` — nine more, across the same two days, each a
different flavor of the same thing: the model producing a plausible-looking but wrong invocation of
a real external tool.

That's when the actual difference between the two "given an architecture, produce something"
tasks became visible. Architecture derivation has no fixed answer — the same vague idea can
reasonably become different valid architectures depending on judgment calls a model is well-suited
to make. Scaffold commands don't have that property. Once you know a service is "Spring Boot" and
a frontend is "React with Vite," the shell commands to scaffold them are the same every single
time. There was no decision left for a model to make — only an exact, external CLI syntax to
reproduce byte-for-byte, which is exactly the kind of task language models are worst at when the
correct answer has to match a real tool's actual interface rather than a plausible one.

We deleted the LLM call. `1c759bf`: "Scaffold commands are fully deterministic given component
type — the LLM added latency, cost, and hallucination risk. Replace with static templates." Static
Rust functions (`arch_needs_jvm`, `vite_template_for`) now map a component type directly to its
scaffold commands. No model call in that path at all.

The fix didn't make scaffolding perfect — `9dc84b7`, right after the rewrite, fixed three more bugs
in the new deterministic templates themselves (a Vite prompt-handling issue, a Spring Boot
line-continuation problem, incorrect Node `--prefix` usage). Trading hallucination risk for
template bugs was a real trade, not a free win — but template bugs are the kind you fix once and
they stay fixed, not the kind that reappears in a new disguise on the next generation.

# Evidence

- 13 distinct scaffold-generation bug-fix commits across 2026-06-21 and 2026-06-22, each patching a
  different hallucinated or malformed external-tool invocation (Vite, Angular CLI, Spring
  Initializr, npm/Node) — no single fix addressed more than one specific case.
- `1c759bf`'s own stated rationale: the mapping from component type to scaffold command is "fully
  deterministic," and the LLM call bought "latency, cost, and hallucination risk" with no
  offsetting benefit.
- Architecture derivation itself — a task retained as LLM-driven throughout this same period, and
  even made more flexible (schema-free) rather than more constrained — was not implicated in any
  of these bug reports, consistent with it being a genuinely different kind of task.
- 3 further bugs in the new deterministic templates (`9dc84b7`), confirming the rewrite traded one
  class of problem for a smaller, more tractable one rather than eliminating bugs outright.

# Evolution of Understanding

We believed "given an architecture, produce X" was one kind of task, and if a model was good enough
for one instance of it (deriving the architecture itself), it should be fine for another instance
(turning that architecture into commands).

The bug volume said otherwise. Two tasks can share a surface shape — "transform structured input
into output" — while having completely different amounts of real ambiguity underneath. Architecture
derivation has many valid answers depending on judgment. Scaffold-command generation, once the
component type is fixed, has exactly one correct answer, and that answer has to match a real
external tool's actual syntax rather than a plausible-sounding one.

We now ask, before building any "given X, produce Y" step as an LLM call: is Y actually
underdetermined by X, or is there a lookup table hiding in here that we're asking a model to
approximate instead of just writing down.

# Engineering Principle

Reserve the model for tasks with genuine interpretive ambiguity. Once the mapping from input to
output becomes fully enumerable — the same input always has the same correct output — replace
generation with deterministic code, even if an LLM *could* technically produce that output most of
the time. "Most of the time" isn't good enough when the output has to match an external system's
exact interface.

# Why It Generalizes

Any AI-assisted system that chains multiple "transform this into that" steps together is likely
mixing genuinely ambiguous tasks with enumerable ones inside the same pipeline, without having
separated them explicitly. Code generation systems, infrastructure-as-code generators, CLI wrapper
tools — anywhere a model is asked to reproduce the exact syntax of a real external system rather
than make a judgment call — are candidates for the same fix. The tell is the same one that showed up
here: bug reports that are each a different specific hallucination, with no single prompt fix
closing the whole category, because the category itself shouldn't have been a model's job.

# Remaining Questions

We don't have a general test for "is this mapping actually enumerable" beyond noticing it after the
fact, once bug volume made it obvious. A more useful version of this lesson would include a way to
check *before* building a step as an LLM call whether its output space is small and fixed enough to
enumerate directly — we don't have that check yet, only the retrospective pattern-match. We also
don't know whether the same distinction (ambiguous derivation vs. enumerable mapping) holds up as
cleanly in domains further from software scaffolding, where "external tool syntax" isn't the shape
of the enumerable part.
