# Validation Experiment: Does Explicit Role Meaning Improve Downstream Artifacts?

Status: experiment redesign. Supersedes the response-rate framing in `docs/design/role-meaning-
capture-validation-experiment.md` — that document is not deleted, but its central question is set
aside per explicit instruction: assume a user will answer if the question belongs in the workflow.
The uncertainty worth resolving is not "will they answer" but "does the answer matter." No
implementation, no prompts, no code — experiment design only.

Date: 2026-07-16

---

# What Changed, and Why

The prior design spent most of its structure on distinguishing genuine engagement from mechanical
filling from skipping — real questions, but downstream of a more important one. If explicit role
meaning turns out not to change any downstream artifact, whether users answer the question stops
mattering: a well-answered, inert fact is not worth building a capability around. This redesign
assumes the answer exists (drawn from the same realistic role set `docs/design/role-classification-
stability-test.md` already produced) and asks only whether *having* it, as an explicit, available
fact, produces better specifications, policies, reviews, and domain understanding than not having
it — the same shift in emphasis Stage 5 of the contract-driven implementation investigation already
made once before, when generation quality turned out to matter more than generation existence.

---

# Method

**Same real story, same existing mechanism, only the available context varies** — the exact shape
Stage 5's own A/B test already used (production's real prompt vs. contract-scoped context, same
harness, same story). No new prompt is authored for this comparison; every step below runs with its
current, unmodified prompt. The only thing that changes between runs is whether a role-meaning fact
is present in the context that step already receives — the same way an existing ADR is already made
available as context today.

**Subject**: `manufacturer-001`, the only story in this project with a complete, real artifact set
through the specification stage — reused rather than constructed, so every comparison is checkable
against genuine model output.

**Conditions**: not just "with vs. without." Run each comparison under **four** conditions —
no fact available (today's real behavior, the baseline), `internal`, `external`, and `affiliated`
— to separate two different questions that a single with/without comparison would conflate:
does *having any answer at all* change the output, and does *which specific answer* change it
further. If `internal` and `external` produce identical downstream output, that is a materially
different, and less encouraging, finding than if they diverge — it would mean presence, not
content, is doing whatever work gets done.

---

# Artifacts Compared, and What Value / No Value Looks Like For Each

**Specifications** — `entity_schema_prompt`'s scenario-generation output, run unchanged across the
four conditions.
- *Value*: scenario language becomes more specific in a way that tracks the supplied classification
  — e.g., an `external` condition producing a precondition distinguishable from a bare "is
  authenticated" (something that reads as verification- or trust-boundary-aware), while `internal`
  produces no such change, or a different one. An asymmetric result — some conditions changing
  scenario content, others not — would itself be a real, informative finding, not a null result.
- *No value*: scenario output is identical across all four conditions, including the baseline —
  meaning scenario generation doesn't use this fact even when it's made available, a distinct
  finding from "the fact doesn't matter" (see the wiring/value distinction below).

**Policies** — the business-policy checklist's `authorization` area specifically, run unchanged
across the four conditions.
- *Value*: the classification moves from `unresolved` (today's real, correct behavior given no
  supporting evidence) to `resolved` or `not_applicable` — *with* a citation naming the supplied
  role fact as evidence — for at least one non-baseline condition. This is the most directly
  mechanically connected artifact to role meaning of the four, per the Role-Semantics
  Investigation's own dependency analysis (authorization presupposes role identity is already
  settled), so it is the single most likely place to see a real effect if one exists anywhere.
- *No value*: the checklist's classification and reasoning are unchanged across all four conditions
  — the citation-requirement mechanism (`unresolved-decisions-become-explicit-decision-points`'s
  own enforced-cost fix) finds no more basis to resolve `authorization` with the fact present than
  without it.

**Reviews** — a re-run of the same Product-Owner Perspective Experiment methodology
(`docs/design/product-owner-perspective-experiment.md`), applied to the artifacts generated under
the `external` (or whichever condition reflects the most realistic real-world answer for
`manufacturer representative`) condition, using the same five personas and the same review
discipline already validated once.
- *Value*: the Governance-Oriented persona's authorization/PII findings and the Terminology-mapping
  role-semantics finding — the two findings most directly attributable to missing role meaning in
  the original run — are measurably reduced or resolved. This reuses an already-validated review
  instrument as the judge of "better," rather than inventing a new definition of quality for this
  comparison alone.
- *No value*: the same review, re-run against the enriched artifacts, reproduces essentially the
  same findings — meaning explicit role meaning didn't actually change what a considered human-
  style review would still need to ask.

**Domain understanding** — whether the captured fact propagates to a *second*, later story that
reuses the same role, via the one channel this project already has for this: `stories_from_intent_
prompt` already renders a role's description into context when one exists (confirmed directly in
`role-semantics-investigation.md`'s role inventory), it is simply never populated today for the path
that matters.
- *Value*: a second story's own generation (story decomposition, and whatever of the above three
  artifacts that second story produces) differs, in a traceable way, because the first story's
  role-meaning answer is now available as accumulated context — direct evidence the capability
  compounds across stories rather than only paying off once.
- *No value*: a second story reusing the same role shows no observable difference despite the
  enriched context being available — the accumulated fact reaches the prompt (confirmed structurally
  already) but doesn't change anything the model does with it.

---

# What Would Demonstrate Value, Overall

Not a single artifact category succeeding in isolation, but a pattern: **Policies and Reviews are
the two most mechanically connected to role meaning and the most likely to show a real effect if
one exists at all** — a genuine, citation-backed change in the authorization classification, paired
with a measurable reduction in the two review findings role meaning was theorized to address, would
be strong, attributable evidence. Specifications and Domain Understanding showing a difference too
would strengthen the case further, but their absence wouldn't by itself invalidate a positive result
from the other two, since they depend on a longer causal chain (the fact reaching a prompt that
currently has no instruction to use it a particular way) that role meaning capture alone doesn't
control.

# What Would Demonstrate No Value, Overall

**All four conditions producing identical output across all four artifact categories** would be the
clearest possible negative result. Short of that, two *different* negative findings need to be told
apart, because they call for different responses: (a) the fact is available in context but never
referenced by any output — a **wiring** finding, meaning role meaning might still matter but nothing
downstream currently knows to use it; versus (b) the fact is referenced, but the resulting output is
no better by the criteria above — a **value** finding, meaning even correctly-used role meaning
doesn't improve anything a human reviewer would actually care about. The comparison design already
controls for the more trivial version of (a) — every condition makes the fact explicitly available
the same way an ADR already is — so an entirely unreferenced fact under this design points more
specifically at "the existing prompts have no reasoning path that would use this even when
available," not merely "the pipeline never sees it," which is today's real, different starting
condition.

---

# What This Does Not Test

Unchanged from the prior design's own scope discipline: nothing here tests ownership, bounded
contexts, domain-glossary enrichment as an independent initiative, hot-spot capture, or forward-
reference detection. It also, per this redesign's own explicit instruction, does not test whether a
real user would supply the role-meaning fact in the first place — that question is assumed answered
and set aside, not resolved; if this value validation succeeds, the response-rate question from the
prior design becomes relevant again before implementation, not before this one.
