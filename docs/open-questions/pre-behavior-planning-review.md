---
title: Pre-Behavior Planning Review
status: deferred
origin: contract-driven implementation investigation (Stage 6, composition work)
date_discovered: 2026-07-15
related_principles: []
related_narratives: [the-road-to-contracts]
related_reports: []
related_design_docs: [contract-composition-assessment.md]
---

# Question

What happens *before* behavior-based planning begins — service discovery, service boundary
identification, and technology-stack recommendation — and is any of it reproducible or
well-understood? Specifically: does service discovery's technology recommendation (e.g. Spring
Boot/Maven vs. Node.js/Express for the same kind of service) vary depending on what the model
knows at discovery time, and if so, is that variance a problem?

# Why It Matters

Every stage of the contract-driven implementation investigation (Stages 1–6) has validated the
pipeline *after* behavior-based planning starts: behavior extraction, decision points, clustering,
contracts, contract-driven implementation, composition. None of it has examined what happens
upstream of that — and a technology-stack recommendation made early (service discovery) determines
which tech-stack skill every later stage renders against. If that recommendation isn't
reproducible, everything downstream inherits an unexamined source of variance.

# Evidence So Far

An observation made in passing, not investigated: service discovery has been seen to recommend
different technology stacks for what appears to be a comparable service, depending on what the
model knew at the time of discovery. **This is not currently treated as a bug** — it's an LLM-
driven architecture recommendation, and nothing has established whether the variance is real,
how large it is, or whether it matters.

# What We Know

- Technology selection currently happens as an LLM recommendation during service discovery/ADR
  proposal (`spec`), not as a Decision Point.
- Once selected, a service's technology drives which tech-stack skill (`canopy-llm/src/skills/
  tech_stack.rs`) renders for every later prompt — a load-bearing choice made once, early, and
  never revisited mechanically.

# What We Don't Know

- Whether the observed stack-recommendation variance is real and reproducible, or an artifact of
  one or two anecdotal runs.
- How technology recommendations propagate through skills, planning, contracts, and
  implementation — never traced end to end.
- Whether stack selection should remain a recommendation, become an explicit human Decision Point,
  depend on organization defaults, or follow some other model entirely.
- What information is actually available to the model before any behavior exists for a story.

# Why Deferred

Explicitly upstream of the current architectural frontier. Current priority is contract
composition, dependency modeling, and Stage 6 composition work (see
`docs/design/contract-composition-assessment.md`). Investigating pre-behavior planning now would
split attention across two unrelated fronts before either is settled.

# Possible Experiments

- Run service discovery N times against the same story input, holding everything else fixed, and
  check whether the recommended technology stack is stable.
- Trace one real technology recommendation end to end — from the ADR proposal through
  `services.yaml` through `skill_for_technology` through a generated file — to confirm (or refute)
  that propagation is direct and traceable, the same grounding standard Stages 1–6 already apply to
  contracts.
- Determine mechanically which of the "what we don't know" questions above are answerable by
  reading existing code/prompts vs. require a live reproducibility run.

# Exit Criteria

Revisit once composition/Stage 6 work has settled to a stable state (mirroring how this
investigation's own stages complete before moving to the next). Treat as its own standalone
investigation at that point: design → ground in real dogfooding data → decide — not a quick side
inquiry folded into composition work.

# Resolution

(Not yet resolved.)
