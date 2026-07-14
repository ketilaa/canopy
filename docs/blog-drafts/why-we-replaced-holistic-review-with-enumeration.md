---
title: "Why We Replaced Holistic Review with Enumeration"
date: 2026-07-14
status: draft

learning_type:
  - principle-discovery

topics:
  - prompt-engineering
  - small-language-models
  - specification-completeness
  - ai-assisted-verification

key_principles:
  - "Exhaustive enumeration over a known, bounded set outperforms holistic review for coverage-critical tasks."
  - "A small model does not fail because it can't reason — it fails because open-ended review doesn't force it to visit every item."

source_artifacts:
  - "commit 1b295df — Implement Stage 0 (Specification Completeness)"
  - "commit e61043f — Record Decision Points and the enumeration-over-holistic-review principle"
  - "commit f0aaa74 — Add enumeration-over-holistic-review rule; distinguish audits from compensation"
  - "commit 7553d07 — Split spec generation into schema/policy and coverage-driven scenario calls"
  - "reproducibility sweep, manufacturer-001, 2026-07-14"

story_ids: []

evidence_strength: high

commits:
  - 1b295df
  - e61043f
  - f0aaa74
  - 7553d07

initial_assumption: >
  A capable enough prompt, asking a model to "review the specification and find any gaps,"
  should be sufficient — the model just needs the right context and a clearly worded question.

final_understanding: >
  For any review task over a bounded, enumerable set of items, holistic phrasing reliably misses
  items even with perfect context — not because the model can't reason about any single item, but
  because open-ended review doesn't force it to visit every one. Converting the same task into an
  explicit walk over a known checklist, one item at a time, closed the gap completely and
  reproduced this result across four independent areas of the same system.

publish_recommendation: yes
cluster: "Enumeration Over Holistic Review"
---

# Summary

We had a data schema with 9 known constraints and asked a language model to check whether our test
scenarios covered all of them. It found 4. The interesting part wasn't the miss rate — it was
which 5 it missed. One of them was the exact same kind of constraint the model had just correctly
caught, three times in a row, on other fields in the same response. That single detail changed how
we thought about the problem. We weren't looking at a reasoning failure. We were looking at a
coverage failure — and it kept reproducing, the same way, in three more parts of the system once we
knew what to look for.

# Original Assumption

We assumed a well-worded holistic prompt was the right way to ask for this. Show the model the
schema, show it the scenarios, ask it to report what's missing. That's how you'd brief a person —
give them the full picture and trust their judgment across it.

# What Happened

4 out of 9. We went looking for the pattern in what it missed, expecting maybe a category of
constraint the model didn't understand well. Instead: one of the missed constraints was a
field-length limit, and the model had correctly flagged the identical constraint type on three
other fields earlier in the exact same response. Same reasoning, same task, three correct answers
in a row — and then a miss, a few lines later, on a case that wasn't any harder.

That ruled out "the model can't reason about this." Our new hypothesis: this was a coverage
failure, not a reasoning failure — the holistic framing never forced every item to actually be
visited, so an item could get silently skipped even when the model could reason about it correctly
in isolation. So we changed the framing instead of the wording. Rather than one holistic question,
we broke it into three explicit checklists the model had to walk item by item: one entry per
field-and-constraint pair, one per scenario, one per open question. Same context, same model, same
underlying judgment per item — the only thing that changed was that nothing let an item pass
unvisited.

9 out of 9 on the re-run.

If the coverage hypothesis was right, we had a testable prediction: the same fix should work
anywhere else in the pipeline that asked a model to check a bounded set for completeness, not just
here. We wrote the result up as a standing principle rather than a one-off prompt fix specifically
to make that prediction explicit and go looking for confirming or disconfirming cases.

We found three more.

The next place: scenario generation. A later stage writes test scenarios covering a schema's
constraints, and we'd built that the same holistic way — "write scenarios covering this entity's
constraints." Running the identical request repeatedly, only the model's sampling varying, the
scenario count drifted run to run, and so did which constraints actually got a scenario. No error,
no warning — just a silently different answer each time.

We applied the same fix. Before any scenario gets written, ordinary code walks the schema and
produces an explicit numbered list: one item per constraint, one for missing-mandatory-field
handling, one per resolved business rule. The model then writes exactly one scenario per listed
item — no inventing extra coverage, no skipping anything on the list. Running the request three
times from an identical starting point, the number of scenarios generated matched the size of the
computed checklist exactly, every time.

A fourth data point tested the hypothesis harder, and complicated it usefully. If enumeration was
the whole story, a task that was *already* fully enumerated should be immune to this failure. We had
one: a separate step asks a model to classify six fixed business questions — does a field need to
be unique, does an action need authorization — into one of three named buckets: resolved, not
applicable, unresolved. Six named items, one at a time, three named output options. Already
enumerated, by our own definition.

In a live run, the model still put half of them into a fourth bucket the prompt never offered.
That's a genuine counter-case, not a confirmation — enumerating the input didn't stop the model
from inventing an output category nobody sanctioned. Fixing it took a stricter output shape,
forcing exactly six named entries with no way to add or skip one, not another round of the same
enumeration fix. Which told us "did every item get considered" and "did the answer land somewhere
valid" are two separate questions. Enumeration alone only answers the first.

# Evidence

- 4 of 9 constraint gaps found on the first holistic pass, with the model correctly catching the
  identical constraint type (a field-length limit) on three other fields in the same response
  before missing it on a fourth — ruling out a comprehension failure. Restructured into three
  explicit, item-by-item checklists, the re-run found 9 of 9. (Commit `1b295df`.)
- The result was generalized into a named principle before its later confirmations happened — the
  design note predicted it should reapply to behavior extraction, cluster review, and coverage
  verification, and it did. (Commit `e61043f`.)
- Scenario generation, rebuilt the same way: a mechanically-computed coverage checklist followed by
  a one-scenario-per-item generation pass. (Commit `7553d07`.) In a controlled reproducibility test
  — three runs, identical starting state, only sampling varying — the number of scenarios generated
  matched the size of the independently-computed checklist exactly in every run.
- The pattern was formalized as a standing rule only after it had already reproduced independently
  across four separate areas: constraint coverage, behavior extraction, dependency review, and
  clustering review. (Commit `f0aaa74`.)
- Counter-case: a policy-classification prompt that was already fully enumerative (six named items,
  three named buckets) still let a model invent an unlisted fourth bucket for half the items in one
  run — evidence that enumerating the input set doesn't, by itself, constrain the output shape.

# Evolution of Understanding

We believed a review task's quality was mostly a function of context and instruction clarity — the
right information, a clearly worded question, and the model should catch what matters.

The evidence said otherwise, directly: identical context, identical model, identical judgment
required per item — and the only thing that moved the result from 4/9 to 9/9 was whether the task
was phrased as "review this and find gaps" or "walk this fixed list and answer for each entry." The
failure wasn't reasoning about any one item. It was that open-ended review never guaranteed every
item got visited at all.

We changed the architecture, not just the wording: wherever the pipeline needs a model to check a
bounded set for completeness, that set now gets computed mechanically and shown as an explicit
checklist first, with generation or verification happening one item at a time against it.

We now default to exhaustive enumeration for any bounded, knowable set, and reserve holistic
phrasing for tasks that genuinely have no fixed set to walk — open-ended architectural judgment
calls, for instance. We've also learned that enumerating the input doesn't automatically constrain
the output — those are two separate failure modes, worth diagnosing separately before assuming
enumeration alone will fix a given gap.

# Engineering Principle

For any task defined over a bounded, enumerable set of items, walking every item explicitly and
requiring an answer for each one reliably outperforms asking a model to review the set holistically
for gaps — even when the model has full context and the instruction is clearly worded. The failure
being fixed is coverage, not comprehension.

# Why It Generalizes

This isn't specific to specification review, and it isn't specific to one model. Any system that
asks a language model to audit a bounded artifact for completeness — a schema against its
validators, a set of API endpoints against their documented contracts, a test suite against its own
coverage requirements — is exposed to the same failure: correct reasoning on the items attended to,
silent skipping of others, no signal that anything was missed. The fix generalizes cleanly: compute
the bounded set mechanically, in ordinary code, and require an answer against every named item in
it, one at a time. It costs more tokens per call. It doesn't cost more model capability. In our
case, the difference was between silently shipping missing test coverage and catching all of it.

# Remaining Questions

We haven't found where this stops helping. Every set we've tried has been genuinely enumerable in
advance — fields, constraints, scenarios, decision areas — and we don't yet know how the pattern
holds up on a set with hundreds of items, where the checklist itself risks becoming long enough to
reintroduce the same "not everything gets attended to" problem. We also don't have a real theory
for why enumeration works this reliably on a smaller, locally-hosted model — whether it's about
attention allocation across long context, or something more specific to open-ended versus itemized
instructions. A cleaner test, varying only model scale on the same enumerated task, would help
separate "this fixes weaker models" from "this fixes open-ended review prompts, regardless of model
strength."
