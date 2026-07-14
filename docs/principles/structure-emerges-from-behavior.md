---
title: "Structure Should Emerge From Described Behavior, Not Be Solicited Upfront"
status: draft

confidence: high

maturity: validated

themes:
  - system-design
  - requirements-elicitation
  - ai-assisted-specification

evidence_strength: high

source_artifacts:
  - "commit 2805e03 — Replace Requirements/Architecture with DeliveryIntents/ArchitecturePrinciples (2026-06-19)"
  - "commit 8d53abd, e900514 — schema-free architecture derivation (2026-06-21)"
  - "commit 0e3fa34, 2661da1, 85a7f1b, 470ca03 — roles/boundaries/questions/vision removed from upfront elicitation (2026-06-23)"
  - "commit 30b656b, 7f5efe6 — premature/anticipatory extraction bugs (2026-06-23)"
  - "docs/retrospectives/2026-06-19-reconstructed.md, 2026-06-21-to-06-22-reconstructed.md, 2026-06-23-to-06-25-reconstructed.md"

related_principles:
  - reserve-the-model-for-genuine-ambiguity
  - unresolved-decisions-become-explicit-decision-points
cluster: "Emergent Design"
---

# Principle

Information a system needs (domain vocabulary, user roles, architectural boundaries, technology
choices) is captured more accurately when derived from a specific, concrete described behavior than
when solicited upfront through abstract, categorical questions asked before any behavior has been
described.

# Problem That Revealed It

The project's very first architecture schema (day one) was a fixed set of typed fields —
frontend, backend, database, deployment — filled in from an upfront description of an idea, before
any specific behavior was known. Three same-day patch commits widened individual fields to
accommodate richer model output, before the whole approach was abandoned the same day in favor of
deferring detailed requirements to "intent-start time" rather than capturing them upfront. Four days
later, a parallel pattern showed up in a completely different part of the system: an "explore"
command asked up to three upfront questions (who/what/why) before generating a vision document, and
one by one, user roles, project boundaries, and eventually the questions and the vision document
itself were all removed from this upfront step and replaced with information derived from
behavioral statements typed later.

# Evidence

- `0a20bf6` (day 0): a rigid, upfront-specified architecture schema replaced by deferred,
  intent-time-derived types, on the same day the rigid version was first shipped and immediately
  needed three patches to accommodate model output it hadn't anticipated.
- `2191aab`/`57505ab` (2 days later): architecture derivation itself made schema-free, continuing
  the same direction rather than reverting it.
- `d0766ad`: "Let user roles emerge from intent, not from explore" — roles moved from an upfront
  question to accumulating from story `as_a` fields.
- `f0e8593`: "Drop boundaries from explore questions; allow zero questions" — boundaries deferred to
  later stages; explore may now ask nothing at all.
- `9388a92`: clarifying questions removed from explore entirely, stated as adding "friction without
  value."
- `0cf44ed`: explore renamed to `init`; the vision document dropped completely — `init` now performs
  no LLM call at all.
- Concrete failure evidence for *why* upfront elicitation was actively harmful, not just
  unnecessary: `d4ec54b` found domain extraction pulling from a story's stated benefit rather than
  its actual action, and stories naming implementation details ("in the catalog," "via the API")
  before any architecture had been decided — premature specification leaking into places it hadn't
  been decided yet. `5cfe54b` found event extraction adding a speculative `Updated` event alongside
  a described `Created` event with no textual basis — anticipating structure not yet described.

# Counter-Evidence

The shift away from upfront elicitation was not unconditional, and evidence of a real limit exists
in the same period. Two days after boundaries and questions were being stripped from `init`, a
bootstrap step was reintroduced (`0293570`, `9813334`): LLM-suggested candidate entities and roles,
presented as a pre-selected, human-editable multi-select at `init` time. Fully emergent,
ask-nothing-upfront elicitation was tried and found to have its own cost — starting completely cold,
with nothing to react to or correct, was worse than a seeded-but-editable starting point. The
principle held for *questions* (abstract, categorical, asked before any behavior exists) but not for
all upfront information — a suggested, correctable starting point survived alongside pure
behavior-driven accumulation.

# Applicability

- Requirements elicitation in any system gathering structured information (domain vocabulary,
  roles, technology choices, business rules) through a mix of upfront setup and ongoing usage
- Deciding whether a new piece of information should be asked for explicitly at project start or
  derived from the first concrete instance of behavior that implies it
- AI-assisted specification tools generally, where "ask the user a checklist of setup questions" is
  a natural first design and this evidence suggests it degrades accuracy compared to deriving the
  same information later, in context

# Confidence Assessment

High. The pattern repeated independently across at least four different kinds of information
(architecture structure, user roles, project boundaries, domain vocabulary) within one week of
project history, each time in the same direction (upfront → emergent), and two concrete failure
modes (premature specification leakage, anticipatory over-generation) were directly observed and
tied to the upfront-elicitation design before it was changed. The counter-evidence (the reintroduced
bootstrap step) is itself well-evidenced and sharpens rather than undermines the principle, showing
it was tested against a real limit rather than applied unconditionally.

# Generalization

Likely useful in any system — AI-assisted or not — that elicits structured information from a user
before that user has expressed any concrete behavior for the system to act on. The general risk
being guarded against is well-known outside AI systems too (upfront requirements gathering has long
been criticized in software engineering for producing speculative, soon-stale specifications), but
this evidence adds a specific, AI-system-flavored failure mode: a model asked to extract structured
information from an *abstract* question-and-answer exchange appears more prone to premature
specificity and anticipatory over-generation than the same model extracting the same information
from a concrete, already-described action.

# Future Validation

The reconstructed evidence here comes entirely from commit messages and diffs, not from a live
side-by-side comparison of upfront-elicited vs. emergent-elicited information quality on the same
input — a more rigorous test would run the same underlying idea through both an upfront-question
version and an emergent version of the same pipeline and compare the resulting artifacts directly,
rather than inferring the comparison from sequential commits made two days apart. It's also not yet
established how far "ask nothing upfront" can be pushed before the reintroduced-bootstrap-step
counter-evidence becomes the dominant consideration rather than the exception.
