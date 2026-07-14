---
title: "The Same Fix, Rediscovered Two Weeks Apart"
date: 2026-07-14
status: draft

learning_type:
  - principle-discovery
  - design-evolution

topics:
  - ai-assisted-verification
  - system-design
  - prompt-engineering

key_principles:
  - "A principle discovered once in a narrow form tends to resurface, more generally, once its early instances accumulate."
  - "Compute facts mechanically and let the model act on facts — a lesson that gets rediscovered independently in different parts of a system before it's recognized as one lesson."

source_artifacts:
  - "commit 0cabf3d — Enforce frontend step ordering and inject missing tests in code, not via prompt (2026-07-02)"
  - "commit a254b25 — Add Entity Continuity gate (2026-07-13)"
  - "commit 390b7f6 — Compute domain-event ADR existence as a fact instead of asking the model to check (2026-07-14)"
  - "commit f0aaa74 — Add enumeration-over-holistic-review rule; distinguish audits from compensation (2026-07-13)"

story_ids: []

evidence_strength: high

commits:
  - 0cabf3d
  - a254b25
  - 390b7f6
  - f0aaa74

initial_assumption: >
  Each fix to "the model is asked to judge something it should just be told" looked, at the time,
  like a fix for that specific problem — frontend step ordering, entity naming, ADR duplication —
  not like instances of one underlying lesson.

final_understanding: >
  The same fix shape — stop asking the model to determine something the system can compute, and
  hand it the answer instead — showed up independently at least three times, roughly two weeks
  apart, in unrelated parts of the pipeline, before it was written down as a standing principle. The
  early instances weren't wasted; they're what made the later, more general version recognizable
  when it showed up again.
cluster: "Compute, Don't Ask"
---

# Summary

On 2026-07-02, a fix for a frontend ordering bug included this line: "prompt guidance for humans,
code enforcement for machines." Eleven days later, on 2026-07-13, a completely unrelated bug — a
generated data schema drifting onto the wrong entity — got fixed with a mechanical check instead of
a better prompt. The next day, a third unrelated bug — duplicate architecture decisions — got the
same treatment. Nobody connected these three fixes to each other when they happened. They're the
same fix.

# Original Assumption

Each time, the fix looked scoped to its own problem. On 07-02, frontend files were being generated
in the wrong order relative to backend files, and a prompt instruction about ordering wasn't
sticking — the fix was to enforce the order in code after generation, not to write a clearer
prompt. On 07-13, a generated entity schema had fully drifted onto an unrelated domain despite the
correct entity name being stated twice in the same prompt — the fix was a mechanical check
comparing the generated name against already-known vocabulary. On 07-14, a domain-event
architecture decision kept getting proposed twice for the same story — the fix was computing,
in code, whether that decision already existed, and stating the answer as a fact instead of asking
the model to check a list itself.

Each of these read, going in, as "here's what's wrong with this one prompt."

# What Happened

The 07-02 fix (`0cabf3d`) is the earliest clearly-stated version: "prompt guidance for humans, code
enforcement for machines." At the time, this sat inside a single commit about frontend/backend
ordering — a specific bug, not a stated methodology. The same day's broader pattern (documented
separately) was that YAML list-shaped output kept breaking in new ways no matter how the prompt was
worded, and the fixes that actually held combined prompt clarity with defensive, code-side
handling — never prompt wording alone.

Eleven days passed. A reproducibility sweep on 2026-07-13 found a generated entity schema that had
fully diverged into an unrelated domain — the correct entity name was present, verbatim, in two
separate places in the same prompt, and the model still produced something else entirely. The fix
(`a254b25`) added `check_entity_continuity`: a plain string comparison between the generated entity
and already-established project vocabulary, run immediately after generation, failing the whole
operation on mismatch. Nothing here referenced the 07-02 ordering fix. It didn't need to — the shape
was identical: something the system already knew was being re-derived by the model instead of
checked against, and the fix was to stop asking and start comparing.

The next day, 2026-07-14, a reproducibility sweep found a domain-event architecture decision being
proposed twice for the same story, even with the existing decision shown to the model verbatim. The
fix (`390b7f6`) computed the answer mechanically — does a decision like this already exist for this
entity and operation — and injected it as a single stated fact the model just had to act on, instead
of asking it to scan a list and judge a match itself.

By this point, the pattern had shown up often enough, across different bugs solved by different
people at different times, to state directly rather than rediscover again: `f0aaa74`, the same day,
formalized it as a standing rule — prefer exhaustive enumeration and mechanical fact-computation over
holistic model judgment, and explicitly distinguish a deterministic audit (compare and reject,
encouraged) from silently rewriting a model's output (forbidden). The rule cites its own evidence:
by the time it was written, the pattern had already reproduced across four separate, unrelated parts
of the system.

# Evidence

- `0cabf3d` (2026-07-02): frontend step ordering enforced in code after a prompt-only fix didn't
  hold; commit message states the governing idea directly — "prompt guidance for humans, code
  enforcement for machines."
- `a254b25` (2026-07-13): a live reproducibility sweep found an entity schema fully diverged from
  correctly-stated context; fixed with a mechanical string comparison against known vocabulary,
  not a better prompt.
- `390b7f6` (2026-07-14): a live reproducibility sweep quantified a duplicate-decision bug at
  roughly 2 of 3 runs, even with the existing decision shown verbatim in context; fixed by
  computing the answer in code and stating it as a fact.
- `f0aaa74` (2026-07-13, same day as the entity-continuity fix): formalizes the pattern as a
  standing rule, citing that it had "already reproduced across Stage 0 constraint coverage, Stage 1
  behavior extraction, dependency review, clustering review" by the time it was written down.
- No commit in the 07-13/07-14 work references the 07-02 ordering fix. The connection is
  reconstructed from reading both periods' commit history together, not stated by either at the
  time.

# Evolution of Understanding

At each individual moment, we believed we were fixing a specific bug: ordering, entity drift, ADR
duplication. Each fix was locally correct and shipped as such.

Looking back across both periods together, the evidence says these weren't three unrelated fixes —
they're the same fix, applied three times, to three problems that looked different on the surface
(file ordering, entity naming, ADR proposals) but shared one underlying shape: the system already
had the answer to a question, and was asking the model to reproduce that answer by judgment instead
of being told it directly.

What changed as a result of noticing this wasn't the fixes themselves — each already worked on its
own terms — but the decision to write the pattern down as a standing rule (`f0aaa74`) once enough
independent instances existed to make it recognizable, rather than waiting to rediscover it a fourth
time.

We now believe the early, narrowly-scoped version of a principle (07-02's one-line note inside a
bug fix) isn't wasted effort even if nobody generalizes it immediately — it's raw material. The
principle becomes visible once enough independent instances of it accumulate to be pattern-matched
against each other, and that recognition doesn't have to happen at the moment of the first instance.

# Engineering Principle

A fix that looks locally scoped — solving one bug in one place — may be an early instance of a more
general principle that hasn't been recognized as such yet. Treat a recurring "we told the model to
do X, and it kept not doing X, so we made the system do X directly instead" pattern as a signal
worth revisiting across the whole codebase, not just fixing again in isolation each time it
reappears.

# Why It Generalizes

Any team working on a system for long enough will fix the same underlying problem multiple times
before recognizing it as one problem — this isn't specific to AI-assisted systems, but AI-assisted
systems seem to produce this pattern unusually often, because "ask the model to determine X" is
such a cheap first move that it gets reached for repeatedly before anyone notices X was actually
computable all along. The practical takeaway: periodically look back across a project's own history
of small, locally-scoped fixes for a repeated shape, rather than assuming each fix is fully
independent of the others. The shape is often more visible in hindsight, across several instances,
than it was in any single fix at the time.

# Remaining Questions

We don't have a systematic way to detect this kind of recurrence proactively — this specific
connection was found by deliberately reviewing old commit history for a retrospective, not by any
process that would have caught it at the time the second or third instance happened. Whether a
lighter-weight practice (e.g., periodically re-reading recent fix commit messages looking for
repeated phrases or shapes) would surface these connections sooner, without needing a full
historical-archaeology pass, is untested.
