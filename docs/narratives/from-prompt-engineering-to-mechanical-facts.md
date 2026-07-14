---
title: "From Prompt Engineering to Mechanical Facts"
status: draft
narrative_type:
  - methodology-evolution
  - process-evolution

time_span:
  start_date: 2026-06-19
  end_date: 2026-07-14

related_principles:
  - reserve-the-model-for-genuine-ambiguity
  - compute-facts-mechanically
  - deterministic-audits-vs-compensation
  - cross-artifact-consistency-audits-prevent-drift

related_retrospectives:
  - 2026-06-19-reconstructed
  - 2026-06-21-to-06-22-reconstructed
  - 2026-07-02-to-07-03-reconstructed
  - 2026-07-13
  - 2026-07-14

related_blog_posts:
  - we-deleted-the-llm-call-and-replaced-it-with-a-template
  - the-same-fix-rediscovered-two-weeks-apart

confidence: high
---

# Summary

Four times, in four unrelated parts of the pipeline, roughly three weeks apart from first to last,
the project reached the same conclusion from a different starting bug: something the system already
knew was being asked of the model instead of being told to it. Each time it looked like a local fix.
Only in hindsight — reading all four together — does it read as one methodology quietly replacing
another: prompt engineering as the default response to a bad output, giving way to mechanical fact
computation and deterministic auditing as the default instead.

# Initial Vision

The earliest instinct, visible from day one, was that a bad model output meant a prompt problem.
Day 0's `Architecture` schema failures were patched three times by loosening field types
(`60f6f12`, `6dd50cf`, `c4b8fc8`) before the schema itself was replaced — each patch an attempt to
let the prompt's output fit better, not a question of whether the model should be answering that
question at all.

# Early Assumptions

Through the project's first six weeks, most reliability problems were treated as wording problems: a
clearer instruction, a better example, a more explicit rule, a more prominent section header. This
wasn't unreasonable — many problems genuinely were wording problems, and got fixed that way. The
assumption became load-bearing, though, in that it was reached for first, by default, even for
problems that turned out not to be about wording at all.

# Turning Points

**Turning point one (2026-06-21/22): scaffold generation.** At least 13 hallucinated or malformed
scaffold-command bugs accumulated over two days — invented package names, wrong CLI flags, mismatched
tool versions — each patched individually, until `c4c7035` stopped patching and asked a different
question: is this mapping actually ambiguous, or is it a lookup table the model is being asked to
approximate? It was the latter. Scaffold generation was deleted and replaced with static templates.
This is the earliest clear instance of the pattern, though it wasn't yet named as one.

**Turning point two (2026-07-02): frontend ordering.** A prompt-only fix for frontend/backend file
ordering (`e3ca836`) didn't hold; the actual ordering logic needed to be enforced in code the same
morning (`9281a2a`). This commit states the emerging methodology directly, for the first time, in
its own message: "prompt guidance for humans, code enforcement for machines." At the time, this read
as a note about one ordering bug, not a manifesto.

**Turning point three (2026-07-13): Entity Continuity.** A reproducibility sweep found a generated
data schema fully diverged onto an unrelated entity, despite the correct entity name being stated
twice, verbatim, in the same prompt. The fix (`98c1783`) wasn't a better prompt — it was a plain
string comparison between the generated entity and already-known project vocabulary, run after
generation, failing the whole operation on mismatch. The same day, this pattern was named directly
as a standing rule (`4fc8d28`): prefer exhaustive enumeration and mechanical fact-computation over
holistic model judgment, and treat a deterministic audit (compare and reject) as encouraged, while
silently rewriting model output stays forbidden.

**Turning point four (2026-07-14): domain-event-ADR detection.** A duplicate architecture-decision
bug, quantified at roughly 2 of 3 reproducibility-sweep runs, got the identical treatment: instead of
asking the model to scan a list of existing decisions and judge a match, a mechanical function
computes the answer and states it as a fact the model just acts on (`9061e34`).

# Contradictory Evidence

The methodology wasn't cost-free once adopted. The Entity Continuity fix and the domain-event-ADR
fix both required real correctness work of their own — the latter went through four independent
review rounds, each catching a genuine gap in the mechanical logic itself (an operation-blind match
that could wrongly suppress a legitimate new event, a case-sensitivity gap, a substring false
positive, a duplicated rule statement). Moving a judgment from prompt to code does not make the
computation automatically correct — it moves the correctness burden to a place code review and tests
can actually reach it, which is different from eliminating the burden.

# Evolution of Understanding

None of the four turning points references an earlier one. Each was arrived at independently, from
its own bug, by whoever was working on that part of the system at the time. That independence is
itself the strongest evidence for treating this as a real, generalizable methodology rather than a
single clever fix that happened to get reused: four different problems, four different points in
time, one answer.

What changed, concretely, is where a fix for "the model got something wrong" is assumed to belong.
Early on, the default assumption was: improve the prompt. By the fourth instance, the default
assumption had become: check whether this is actually a fact the system already knows, and if so,
stop asking.

# Architecture Changes

- Scaffold command generation removed entirely, replaced by static templates (`c4c7035`,
  2026-06-22).
- Frontend/backend step ordering enforced in code after generation (`9281a2a`, 2026-07-02).
- `check_entity_continuity` and `check_event_continuity` added as mechanical, non-LLM gates in
  `cmd_spec`, run immediately after generation, failing loudly on mismatch (`98c1783`, `ea3e1b9`,
  2026-07-13).
- `find_existing_domain_event_for_story` computes domain-event-ADR existence mechanically and
  injects it as a stated fact (`9061e34`, 2026-07-14).
- The methodology itself codified into house style: "prefer exhaustive enumeration over holistic
  review," and an explicit distinction between deterministic audits (encouraged) and Rust-side
  compensation — silently rewriting model output (forbidden) (`4fc8d28`, 2026-07-13).

# Principles That Emerged

This narrative is the connective tissue behind four separate principle documents:
`reserve-the-model-for-genuine-ambiguity` (turning point one), `compute-facts-mechanically` (turning
points three and four), `deterministic-audits-vs-compensation` (the encouraged/forbidden distinction
sharpened at turning point three and tested again during Policy Discovery's own fix), and
`cross-artifact-consistency-audits-prevent-drift` (the specific shape of turning point three).

# Current View

Mechanical fact computation and deterministic auditing are now the default response to "the model
got this wrong" whenever the correct answer is something the system can determine on its own.
Prompt improvement is still the right first response when the task genuinely requires judgment the
system cannot compute — the methodology didn't replace prompt engineering, it narrowed the set of
problems prompt engineering is asked to solve.

# Why This Matters

The project rediscovered the same fix three times over roughly two weeks before naming it as a
standing rule, and a fourth time (scaffold generation) three weeks before that, unrecognized as
related until this reconstruction connected it. That gap — between a fix existing and a fix being
recognized as an instance of something general — is itself worth noticing: an early, narrowly-scoped
version of a good idea isn't wasted, but it also doesn't generalize itself. Someone has to notice the
pattern across instances before it becomes a rule other people can apply without rediscovering it
from scratch.

# Open Questions

Whether this methodology has a boundary — a case where "compute it mechanically" was tried and
turned out to be the wrong call, costing more in code complexity than it saved in prompt
reliability — hasn't been found in the evidence reviewed for this narrative. Either no such case has
occurred yet, or one exists and hasn't been looked for specifically. A dedicated review for
counter-examples (not just confirming instances) would strengthen or bound this narrative further.
