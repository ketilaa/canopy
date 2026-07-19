# Human-Insight Inventory — Rerun Against the Full Accumulated Review History

Status: measurement only, per the Backlog Execution Plan's Iteration 2
(`docs/design/roadmap-reassessment.md`, 2026-07-18 update) — the counting method from
`docs/design/human-insight-inventory.md` applied unchanged, against every real review-gate outcome
this project has produced since, not a redesign of the method or a new mechanism proposal.

Date: 2026-07-19

Reviewed: `docs/design/human-insight-inventory.md` (the original method and its single-story
finding), `docs/reports/manufacturer-001.md`, `docs/reports/backlog-discovery-vocabulary-check.md`
(Runs #1–#3), `docs/reports/product-010-customer-vertical-slice.md`, the real dogfooding project's
`.canopy/review-log.yaml`, `.canopy/stories/product-010/{decisions,decision-audit}.yaml`,
`.canopy/decisions/adr-00{5,6,7,9,10,11}-*.yaml`.

---

## 1. What actually grew, and what didn't

The original inventory had exactly one story's worth of spec-stage data (`manufacturer-001`,
reconstructed from `llm-debug.log` and persisted ADRs, since `review-log.yaml` didn't exist yet —
it shipped 2026-07-15, after that story's `spec` ran). Since then:

- **`review-log.yaml` now exists and has accumulated real, mechanically-logged entries** for a
  second real story (`product-010`) and nine intent-stage sessions (`product-001` through
  `product-008`, `product-010`; `product-009` was never created).
- **Not every category grew.** `product-001` through `product-008` never went past `intent` —
  only `product-010` reached `spec`/`behaviors`. So the spec-stage and Decision-Point sample size
  grew from 1 story to 2, not to 10.
- **A data-provenance split matters here, the same discipline the original inventory applied to
  the loose pty transcripts.** `docs/reports/backlog-discovery-vocabulary-check.md` ran three
  rounds against the vocabulary-discrepancy check specifically: Run #1 (`catalog-001`, scripted
  default answers), Run #2 (`warehouse-001`/`support-001`/`finance-001`/`delivery-001`, judged by
  this agent acting as reviewer), and Run #3 (`product-001`–`product-008`, explicitly disclosed as
  "the project owner's own real dogfooding session... driven entirely by the project owner in
  their own terminal, with no scripting or agent involvement in any answer"). Only Run #3 is
  genuine human-insight evidence. Run #1 and Run #2 test the *mechanism* (does it fire, does it
  log correctly), not human review behavior, and are excluded from every count below.
- **`product-010`'s spec-stage review is not disclosed as project-owner-driven** the way Run #3
  was — its report (`product-010-customer-vertical-slice.md`) states what was accepted, not who
  drove the session or under what scrutiny. Per the original inventory's own stated limit
  (`manufacturer-001` was reviewed "under the accept-every-default pattern the interactive
  dogfooding driving convention itself documents as the norm"), the same caveat applies here by
  default rather than being asserted away: these are real, mechanically-logged Accept actions, not
  narrative reconstruction, but not confirmed deliberate human scrutiny either.

## 2. Aggregated counts, by category, real human-provenance data only

### Intent stage — story accept/reject

| Story | Outcome | Provenance |
|---|---|---|
| `manufacturer-001` (as `account-001`) | **Reject**, hand-corrected (wrong actor, `want` didn't name the entity) | Narrative, `manufacturer-001.md` Session 1 — predates `review-log.yaml` |
| `product-001` through `product-008`, `product-010` (9 stories) | Accept ×9 | `review-log.yaml`, real gate |

**9 accept, 1 reject** across 10 story reviews. The one reject is the only non-Accept outcome
anywhere in this entire inventory, at any stage, across both real stories' full histories.

### Intent stage — vocabulary-discrepancy (meaningful / not-meaningful / not-sure)

| Outcome | Count | Share |
|---|---|---|
| meaningful | 1 | 5% |
| not-meaningful | 20 | 95% |
| not-sure | 0 | 0% |

(21 judgments, Run #3 only — `product-001` through `product-008`. `manufacturer-001` predates this
check; Run #1/#2's 24 judgments excluded per §1.)

### Spec stage — ADR-style proposals (accept / modify / reject), by category

| Category | `manufacturer-001` | `product-010` | Total | Accept | Modify | Reject |
|---|---|---|---|---|---|---|
| structural-service-ownership | 1 | 1 | 2 | 2 | 0 | 0 |
| tech-stack-frontend (UI) | 1 | 2 | 3 | 3 | 0 | 0 |
| tech-stack-backend | — | 1 | 1 | 1 | 0 | 0 |
| infrastructure-database | 1 | 1 | 2 | 2 | 0 | 0 |
| infrastructure-event-broker | 1 | 1 | 2 | 2 | 0 | 0 |
| domain-event (structural) | 1 | 0 | 1 | 1 | 0 | 0 |
| **Total** | 5 | 6 | 11 | 11 | 0 | 0 |

`manufacturer-001`'s backend-tech recommendation rode along on the ownership proposal's
`technology` field rather than logging as its own category (per the original inventory's finding);
`product-010` logged it as a separate `tech-stack-backend` entry — a real difference in how the two
runs' proposals were shaped, not a gap in this count.

**The domain-event category's sample did not grow.** `product-010` is the only other story to
reach `spec` since the original inventory, and it never produced a domain-event proposal at all —
defensibly, since browsing a catalog is a read-only query with no state change to raise an event
about. The category this inventory was originally built to scrutinize most closely (tier-4,
least-reproducible, per the reproducibility sweep) still has exactly one real data point, same as
before.

### Behaviors stage — Decision Points (Stage 2)

| Story | `decisions.yaml` | `decision-audit.yaml` |
|---|---|---|
| `manufacturer-001` | `[]` | `[]` |
| `product-010` | `[]` | `[]` |

Zero decision points have ever been generated for either real story that reached `behaviors`. The
Stage 2 "unresolved-decisions-become-explicit-decision-points" mechanism has never once produced a
reviewable item across this project's entire real dogfooding history — not "zero modify/reject,"
zero *opportunities* to modify or reject. `record_review("behaviors", ...)` in
`canopy-cli/src/commands/behaviors.rs:174-188` only fires inside the per-decision loop, so this
absence is mechanically exact, not a logging gap: `review-log.yaml` has no `command: behaviors`
entries at all, which is the correct, expected output of zero decisions, not a missing feature.

## 3. Does "too early to tell" resolve?

Two different questions were bundled under that phrase across this project's history, and the new
data resolves them differently — collapsing them again here would repeat exactly the kind of
question-blurring CLAUDE.md's Diagnosing Dogfooding Runs section warns against.

**Question A — does the review gate differentiate scrutiny by category at all, across the
categories that now have real data?** **Resolves, and the answer strengthens the original
finding rather than changing it.** Across 11 spec-stage proposals (6 categories), 21
vocabulary-discrepancy judgments, and 10 story reviews — 42 real review actions in total — exactly
one deviated from uniform acceptance, and it happened at the most mechanically obvious point
possible (a wrong actor and a `want` field that didn't name the entity being registered), in the
project's very first session, before any of the categories this inventory tracks even existed as a
distinguishable class. Every subsequent domain-judgment-shaped recommendation — service ownership,
UI/tech-stack choice, database/broker infrastructure, domain-event determination, vocabulary
discrepancy calls — was Accepted without a single Modify or Reject, across two different stories,
two different domains, two different points in this project's timeline. The larger sample sharpens
the original single-story finding into a real pattern: **the review gate reliably catches
mechanically obvious defects and has, so far, never differentiated scrutiny by category among
proposals that require actual domain judgment.**

**Question B — should domain-event determination specifically become a Decision Point (the
question `domain-event-decision-point-criteria-comparison.md` called "too early to tell")?**
**Still genuinely too early to tell, for a sharper reason than before.** This isn't unresolved
because the evidence is thin and ambiguous — it's unresolved because the category's sample size
didn't move at all. The one new story to reach `spec` since the original pass produced zero
domain-event proposals. Answering this question needs a story that actually raises a domain event,
reviewed by someone applying real scrutiny — neither of which this rerun's new data supplies.

**A related, now-doubly-confirmed absence**: the Decision Point mechanism itself (Stage 2) has
never fired in any real story. Whatever `unresolved-decisions-become-explicit-decision-points`
predicts about how a human would treat a flagged decision, this project has zero real observations
of that mechanism actually presenting one — a stronger and more specific gap than "no data," since
it names exactly what evidence is missing and why (no story so far has produced an `unresolved`-
classified item for `behaviors` to surface).

## 4. What this does and doesn't justify concluding

**Justified**: across every category with more than one real data point, review behavior has been
uniform Accept, with the sole documented exception being a defect obvious enough that a human
caught it without any of this project's tooling. This is now based on two stories, two domains,
and 42 real review actions — not a single-story anecdote.

**Not justified**: any claim about how a domain-event proposal specifically would be reviewed
(still n=1, unchanged), any claim about Decision-Point review behavior (n=0, no mechanism firing
observed at all), or any claim generalizing beyond this project's own dogfooding sessions — which,
per §1's provenance caveat, mix genuinely human-driven judgment (Run #3's vocabulary-discrepancy
calls, the intent-stage reject) with sessions whose driving method isn't disclosed
(`product-010`'s spec/behaviors review) and are likely to have run under the accept-every-default
convention this project's own CLAUDE.md documents as the interactive-dogfooding norm.

## 5. Recommended next step

Per this document's own scope (measurement, not design): the Roadmap Reassessment's existing
priority order already covers what comes next and doesn't need revision by this rerun — Question A
is answered as far as this project's real history can answer it without a differently-designed
study (a real "Modify"/"Reject" path has still never been exercised on a domain-judgment-shaped
proposal, only on the one mechanically-obvious defect); Question B stays blocked on the same
missing ingredient it always was — a real story that raises a domain event, reviewed under
disclosed, genuine scrutiny rather than the accept-default convention. Iteration 3's
already-recorded open question
(`docs/open-questions/story-readiness-vs-backlog-evolution.md`) remains the sharper, better-
evidenced thread to pull next; this rerun doesn't surface a new one to compete with it.
