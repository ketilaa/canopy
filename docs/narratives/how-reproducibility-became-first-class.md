---
title: "How Reproducibility Became a First-Class Concern"
status: draft
narrative_type:
  - process-evolution
  - methodology-evolution

time_span:
  start_date: 2026-07-13
  end_date: 2026-07-14

related_principles:
  - cross-artifact-consistency-audits-prevent-drift
  - deterministic-audits-vs-compensation

related_retrospectives:
  - 2026-07-13
  - 2026-07-14

related_blog_posts:
  - every-example-noun-is-a-candidate-answer
  - policy-discovery-vs-policy-invention

confidence: high
---

# Summary

A single dogfooding session, run once, produced a structured audit report with real findings — and
one line of quiet doubt: "Stage 0's gap-finding is non-deterministic run-to-run against an unchanged
specification." That doubt is what turned into the project's most consequential process change of
the following two days: not a fix to any single bug, but a habit of running the same input multiple
times before trusting any single result.

# Initial Vision

The first live dogfooding session against the new behavior-first pipeline (2026-07-13) was run once:
one intent, one spec, one pass through Stages 0 through 4, reviewed carefully and written up as a
structured report with six named findings. This was the established method — drive a real session,
observe what happens, fix what's wrong, move on.

# Early Assumptions

The implicit assumption was that a single well-observed run was sufficient evidence: if a bug showed
up, it was real; if a stage completed cleanly, that stage worked. A run's output was treated as
representative, not as one sample from a distribution that might vary.

# Turning Points

The first session's own audit notes contained a finding that didn't fit this assumption: re-running
Stage 0's completeness check against an *unchanged* specification produced a *different* set of
gaps than the previous run — same story, same ADRs, same scenarios, different answer. This wasn't a
bug that needed fixing so much as a crack in the premise that one run tells you what "the" pipeline
does.

The response was to stop trusting single runs. A reproducibility sweep — the same intent statement,
run three times from an identical clean starting state, every interactive gate answered identically
— was run specifically to separate real bugs from sampling noise. It found something far more severe
than the constraint it was checking for: one of three runs produced a fully divergent entity schema
(a user-account schema for a story explicitly about registering a manufacturer), despite the correct
entity being stated twice in the same prompt. That finding — invisible to a single-run methodology,
since two of the three runs looked fine — became the direct trigger for building Entity Continuity.

# Contradictory Evidence

Reproducibility sweeps didn't just find new bugs; they also confirmed which fixes actually worked
and which didn't fully hold. A second, confirmatory sweep after the entity-naming fix found the
divergence gone (2 of 2 clean), but also surfaced that scenario generation was *still* the dominant
remaining source of variance — a finding a single post-fix run could easily have missed, since a
lucky single run might have looked complete by chance. Later sweeps (the third and fourth) repeated
this exact shape: sweep three quantified a duplicate-ADR bug at roughly 2 of 3 runs and caught
policy-question fabrication live; sweep four, after both fixes, confirmed 0 of 6 duplicate ADRs and
a sharp, measured drop in fabricated policy answers (from 5–6 of 6 questions resolved-with-no-
basis, to 1–2 of 6) — numbers that only mean something because they're compared against the
identical measurement taken before the fix, not against a single before/after anecdote.

# Evolution of Understanding

What changed is the unit of evidence the project trusts. A single run answers "did this happen
once." A sweep answers "how often does this happen, and did a specific number change after a
specific fix" — a categorically stronger claim, and one that surfaced problems (the entity
divergence, the ADR duplication rate, the policy fabrication rate) that a single-run methodology
had already run past without noticing, in the very first session.

# Architecture Changes

No pipeline code changed as a direct result of adopting this practice — its effect was entirely on
process: every subsequent significant fix in this period (Entity Continuity, Event Continuity,
Scenario Coverage Enumeration, Policy Discovery's evidence-grounding, the domain-event-ADR
mechanical check) was validated with a before/after reproducibility sweep rather than a single
confirming run, and each sweep's methodology stayed consistent (three identical runs, one variable
held fixed at a time) so results across different sweeps could be compared directly.

# Principles That Emerged

This narrative doesn't produce its own dedicated principle document, but it's the evidentiary method
behind several others: `cross-artifact-consistency-audits-prevent-drift`'s own strongest evidence
(the entity divergence) came from a sweep, not a single run, and `deterministic-audits-vs-
compensation`'s claims about the Policy Discovery fix rest on a controlled three-run comparison, not
an anecdote.

# Current View

A reproducibility sweep — not a single confirming run — is now the standard way to validate whether
a fix for a reliability problem actually changed model behavior, versus changed one specific
output. The habit that started as a response to one unsettling observation ("Stage 0 isn't
deterministic") became the project's default evidentiary bar for every subsequent fix in this
period.

# Why This Matters

The most severe bug found in this entire two-day period — full entity-schema divergence — was
present in exactly one of three otherwise-identical runs. A single-run methodology had a two-in-
three chance of missing it entirely, and the project's first session, run once, did miss it. The
practice that caught it wasn't a smarter single observation; it was refusing to trust a single
observation at all.

# Open Questions

Sweeps in this period were run at a fixed size (three runs), chosen for practicality rather than
derived from any statistical target — whether three runs is enough to reliably catch a problem at a
given incidence rate (the entity divergence was 1 of 3; would a rarer, equally severe problem need
more runs to surface at all) isn't addressed by the evidence reviewed here. It's also not established
whether this practice has been applied consistently to every fix made outside this specific two-day
window, or whether it's specific to the period where reliability was under unusually close
scrutiny.
