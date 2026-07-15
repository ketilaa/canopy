---
title: Pre-Behavior Planning Review
status: active
origin: contract-driven implementation investigation (Stage 6, composition work)
date_discovered: 2026-07-15
related_principles: []
related_narratives: [the-road-to-contracts]
related_reports: [manufacturer-001]
related_design_docs: [contract-composition-assessment.md, pre-behavior-planning-review.md, pre-behavior-planning-reproducibility-sweep.md]
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

- ~~Whether the observed stack-recommendation variance is real and reproducible, or an artifact of
  one or two anecdotal runs.~~ **Answered 2026-07-15**: real and reproducible as a *finding* — Low
  reproducibility, confirmed by a 5-run sweep. See Resolution below.
- How technology recommendations propagate through skills, planning, contracts, and
  implementation — never traced end to end. **Still open.**
- Whether stack selection should remain a recommendation, become an explicit human Decision Point,
  depend on organization defaults, or follow some other model entirely. **Still open** — now with
  concrete evidence (Low reproducibility) rather than an anecdote to reason from.
- What information is actually available to the model before any behavior exists for a story —
  answered structurally by `docs/design/pre-behavior-planning-review.md`; not repeated here.
- **New**: whether the order-dependent-prompt-content variability source (existing ADRs/services
  rendered in stored `Vec` order, not sorted) contributes to the measured variance — the
  reproducibility sweep isolated model-sampling variance only, by design, and did not test this.

# Why Deferred

*(Historical — investigation has since started; see Resolution.)* Originally deferred as
upstream of the composition/Stage 6 frontier, to avoid splitting attention across two fronts
before either settled. The Roadmap Reassessment (2026-07-15) subsequently concluded the evidence
favored promoting this above composition's remaining questions, not the reverse — see
`docs/design/roadmap-reassessment.md`.

# Possible Experiments

- ~~Run service discovery N times against the same story input, holding everything else fixed, and
  check whether the recommended technology stack is stable.~~ **Done** — see Resolution.
- Trace one real technology recommendation end to end — from the ADR proposal through
  `services.yaml` through `skill_for_technology` through a generated file — to confirm (or refute)
  that propagation is direct and traceable, the same grounding standard Stages 1–6 already apply to
  contracts. **Still open.**
- A second reproducibility sweep varying input *order* (not content) to isolate the
  order-dependent-prompt-content variability source separately from model sampling.
- Whether technology recommendation should become a Decision Point — a design question, not an
  experiment; informed by, but not answered by, the reproducibility result alone.

# Exit Criteria

The reproducibility sub-question is answered (see Resolution) — this entry stays `active`, not
`resolved`, since most of the original questions (propagation tracing, Decision-Point-or-not,
order-dependence) remain open. Fully resolve, and consider promoting the durable lesson to a
principle, once those are answered too.

# Resolution

**Partially resolved, 2026-07-15.** The Pre-Behavior Planning Reproducibility Sweep
(`docs/design/pre-behavior-planning-reproducibility-sweep.md`) ran `identify_architectural
_questions` 5 times against frozen pre-spec state for `manufacturer-001`. Verdict: **Low
reproducibility.** Backend tech (Spring Boot) and database (PostgreSQL) were perfectly stable;
frontend tech and event-broker choice each diverged once, independently; and the domain-event
proposal's own presence and topic-convention compliance was the least stable element of all —
present in 3 of 5 runs, convention-compliant in only 1 of those 3. Full results, classification,
and implications: the sweep design doc's own "Results" section, and
`docs/reports/manufacturer-001.md`'s matching entry.

This answers the *reproducibility* question this entry opened with. It does not answer whether
technology recommendation should become a Decision Point, how recommendations propagate
downstream, or whether order-dependence (a separate variability source from model sampling)
also contributes — all still open, which is why this entry's `status` is `active`, not
`resolved`.
