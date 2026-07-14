---
title: "The Evolution of Canopy's Stated Purpose"
status: draft
narrative_type:
  - origin-story

time_span:
  start_date: 2026-06-19
  end_date: 2026-07-14

related_principles:
  - structure-emerges-from-behavior

related_retrospectives:
  - 2026-06-19-reconstructed
  - 2026-06-23-to-06-25-reconstructed

related_blog_posts: []

confidence: medium
---

# Summary

Canopy's own self-description, at the top of CLAUDE.md, changed once — sharply — three days into
the project, then stayed byte-identical for the following three weeks even as nearly everything
underneath it was rebuilt multiple times. This narrative traces that one change and what it reveals,
and is explicit about what it cannot answer: why the project was started in the first place. That
motivation predates the commit history and isn't recoverable from it — this narrative documents how
the project *described itself*, not why it exists.

# Initial Vision

Day one's CLAUDE.md (`b45b167`, 2026-06-19) states: "Canopy is an AI software engineering system...
NOT a code completion tool... NOT a chat interface over a repository... a planning and
implementation engine that operates on structured repository knowledge produced by Roots." Notably,
Roots — a separate repository-intelligence tool — is already referenced as a dependency on day one,
two days before it was merged into the same monorepo (`170dddc`, 2026-06-21), meaning Roots existed
or was being developed before or alongside Canopy itself, not as a later addition.

The original architecture underneath this description was a single-shot pipeline: vague idea →
vision → requirements → domain model → architecture, each a fixed-schema YAML artifact generated in
one LLM call.

# Early Assumptions

The earliest implicit assumption, visible in the original rigid `Architecture` schema (typed
`frontend`/`backend`/`database`/`deployment` fields), was that a project's architecture could be
captured once, upfront, as a fixed set of decisions. This assumption didn't survive its own first
day — three patch commits widened individual fields the same day they shipped, and by day's end the
schema was replaced entirely (`0a20bf6`).

# Turning Points

The sharpest turning point in this narrative is `e9b917c` (2026-06-23), four days after launch:
"Redesign CLAUDE.md: accurate, concise, emergent-first." This added, for the first time, an explicit
"Core Design Insight" table: "Everything emerges. Nothing is decided upfront," listing which
artifact emerges from which command. It also added a new negative Canopy had not stated before: "NOT
a big-bang architecture generator" — alongside the original two negatives (not code completion, not
a chat interface). And it changed the positive description from "a planning and implementation
engine" to "an incremental planning and implementation engine. It enforces discipline: behavior is
specified before code is written, and architecture decisions are made story by story — never all at
once."

This wording — the entire "Project: Canopy" section, verbatim — has not changed since. A direct diff
against the current file confirms this section is byte-identical to its `e9b917c` version, three
weeks and roughly 210 further commits later.

# Contradictory Evidence

The stated purpose stabilizing early doesn't mean the underlying system stopped changing to match
it — if anything, the opposite. The behavior-first planning redesign (2026-07-13, see "From Stories
to Behaviors") replaced the entire planning pipeline underneath this unchanged statement, motivated
by finding that the *existing* pipeline — ADRs and file structure decided before specific behaviors
were known — was itself violating "architecture decisions are made story by story," the very
discipline the 06-23 statement claimed to enforce. The stated purpose didn't just survive
unchanged; at least once, it was the standard the actual implementation was later measured against
and found wanting.

# Evolution of Understanding

What we believed changes here is best read as two layers moving at different speeds. The stated
identity — "incremental, emergent, behavior-before-code" — settled fast (within four days) and has
held for the entire observed history. The mechanisms claiming to embody that identity have been
rebuilt repeatedly: a single-shot generator, then a multi-command planning phase, then an
intent/story lifecycle, then tech-stack skills and a TDD loop, then a full behavior-first pipeline
with its own five gated stages. Each rebuild can be read as a correction toward the same
unchanged statement, not a change of direction.

# Architecture Changes

- Day 0: rigid single-shot schema → deferred, intent-time-derived types (`0a20bf6`).
- 06-21/22: LLM-derived scaffold commands → deterministic static templates (`c4c7035`).
- 06-23-25: upfront explore questions → emergent extraction from stories (`d0766ad`, `f0e8593`,
  `9388a92`, `0cf44ed`).
- 06-29-07-01: single-shot per-intent generation → per-service generation with tech-stack skills and
  a TDD loop (`6bed3f6`, `f1d7d65`).
- 07-13: ADR/architecture-skill-driven planning → the five-stage behavior-first pipeline (Story →
  Behaviors → Decisions → Clusters → Contracts), documented in
  `docs/design/behavior-first-planning.md`.

# Principles That Emerged

`structure-emerges-from-behavior` is the closest existing principle document to this narrative's
subject, though it was extracted from the specific 06-19 through 06-25 mechanics, not from the
stated-purpose text itself. This narrative adds the observation that the *doctrine* (the explicit
"Everything emerges" table) was written down four days after the *practice* (the day-0 pivot) had
already happened once — the stated principle followed the lived experience, not the other way
around.

# Current View

The project currently describes itself, unchanged since 06-23, as "an incremental planning and
implementation engine" that "enforces discipline: behavior is specified before code is written, and
architecture decisions are made story by story — never all at once." Every major rebuild since —
including the most recent one, the behavior-first pipeline — has been justified in terms of getting
closer to that same statement, not replacing it.

# Why This Matters

A stated identity that survives unchanged while its supporting mechanisms are repeatedly rebuilt is
a useful signal: it suggests the mechanisms, not the goal, were wrong each time — which is a
healthier failure pattern than the alternative (the goal itself drifting to match whatever was
easiest to build). Whether this is because the 06-23 statement was well-chosen, or because nobody
went back to revisit whether the goal itself still made sense as the system evolved, isn't
determinable from the commit record alone.

# Open Questions

This narrative cannot answer why Canopy was started, what problem it was originally meant to solve
for its author, or what alternatives were considered before day one — none of that is present in
the commit history, design docs, or any other artifact this reconstruction method can access. A true
origin story would need to come directly from the person who started the project, not from mining
what was already written down. Separately: whether the 06-23 statement has ever been seriously
reconsidered (as opposed to merely never revisited) is not something the evidence here can
distinguish.
