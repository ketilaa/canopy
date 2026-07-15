# Human-Insight Inventory — First Pass

Status: observation only, scoped narrowly per the Roadmap Reassessment (2026-07-15) — no fixes
proposed. Answers one question: for `manufacturer-001`'s real dogfooding history, did the human
review gate treat the least-reproducible recommendation category (domain-event) any differently
from the most-reproducible ones (backend tech, database)?

Date: 2026-07-15

Reviewed: `docs/design/pre-behavior-planning-review.md` (Decision Classification table, the
categories reused below), `docs/design/pre-behavior-planning-reproducibility-sweep.md` (the
reproducibility result this document is responding to), `docs/design/roadmap-reassessment.md`
(§2, which scoped this investigation), the real dogfooding project's
`.canopy/logs/llm-debug.log`, `.canopy/decisions/adr-00{1..6}-*.yaml`,
`.canopy/stories/manufacturer-001/{decisions,decision-audit,cluster-review}.yaml`, and
`docs/reports/manufacturer-001.md` (Stage 6, for the domain-event ADR's later correction).

**A data-provenance caveat, found during this pass and worth stating up front.** Two loose pty
session transcripts (`02-spec.raw.log`, alongside `01-intent.raw.log`/`03-behaviors.raw.log`,
living outside any `.canopy/` directory) exist on disk and show a `spec` review session for this
same story — 6 "Accept this ADR?" prompts, all answered `Accept`. It is tempting to treat that as
the record of this story's review. It is not: its proposed decision text (`manufacturer-
registration-service`, `product-manager-portal`, a separate "Tech Stack for Product Manager
Portal" ADR) does not match any ADR actually persisted to `.canopy/decisions/` for this project
(`manufacturer-service`, `manufacturer-registration-portal`, no standalone tech-stack ADR — tech
stack is a field on the ownership/UI proposals instead). `llm-debug.log`'s real, timestamped spec
call (`2026-07-13T18:37:25Z`) matches the persisted ADRs exactly. The raw log is a rehearsal or a
separate attempt whose output was never the one that shipped; it is excluded from the analysis
below. Anchoring on log content before checking it against what's actually persisted would have
produced a confidently wrong inventory — the same "anchor first" discipline CLAUDE.md's Diagnosing
Dogfooding Runs section already prescribes for a different purpose applies here too.

---

## 1. The real review history, reconstructed

`llm-debug.log` records exactly one `identify_architectural_questions` call for this story
(`2026-07-13T18:37:25Z`, log lines 213-278), against a frozen state of zero existing ADRs and an
empty services registry — the same frozen state the reproducibility sweep later replayed 5 times.
It returned 5 proposals. Comparing each proposed `decision`/`reason`/`alternatives` against the
persisted ADR content:

| # | Proposal title | Category (per Decision Classification table) | Proposed decision | Persisted decision | Review outcome |
|---|---|---|---|---|---|
| 1 | Service Ownership for Manufacturer Registration | Structural | `manufacturer-service` | `manufacturer-service` | Accept — byte-identical `decision`/`reason`/`alternatives` |
| 2 | UI for Manufacturer Registration | UI | `manufacturer-registration-portal` | `manufacturer-registration-portal` | Accept — byte-identical |
| 3 | Database for Manufacturer Service | Infrastructure (tech stack) | `PostgreSQL` | `PostgreSQL` | Accept — byte-identical |
| 4 | Event Broker for Event-Driven Architecture | Infrastructure (tech stack) | `Redpanda` | `Redpanda` | Accept — byte-identical |
| 5 | Domain Event for Manufacturer Registration | Structural (domain event) | `ManufacturerRegistered` | `ManufacturerRegistered` *(at spec-review time)* | Accept — byte-identical |

Two proposal categories CLAUDE.md's own prompt documentation names as always required —
"Tech stack questions... MANDATORY, never omit" as a *separate* ADR — don't appear as standalone
entries in this run; backend and frontend technology instead ride along as a `technology` field
on proposals #1 and #2. This is a real run detail worth recording, not an error: `apply_adr_proposal`
writes `ServiceEntry.technology` from whichever proposal supplies it (`adr_merge.rs:77-84`), and
both of these proposals did. Out of scope for this pass to judge whether folding tech stack into
the ownership/UI proposal, versus a standalone ADR, has review-quality consequences — noted for a
possible future item, not resolved here.

**Every single proposal, across every category, was reviewed identically: Accept, no edit.** This
is an inference from content match, not a witnessed keystroke — no pty transcript exists for the
real run (see the provenance caveat above) — but it's a strong one: the "Modify decision text" gate
(`pre-behavior-planning-review.md` row 8) only lets the human retype the `decision` field itself,
and all five proposals' `decision`, `reason`, and `alternatives` are byte-identical between the raw
LLM output and the persisted ADR. A modified `decision` field with an untouched `reason` (which the
prompt generated to justify the *original* decision) would be a visible tell; there is none.

### The domain-event ADR's later change is a separate event, not a review outcome

`adr-006`'s persisted decision today reads `ManufacturerRegistered on topic
manufacturer.registered` — different from what `llm-debug.log` shows was proposed and (per the
above) accepted. This change did not happen through `spec`'s review gate. Per
`docs/reports/manufacturer-001.md`'s Stage 6 entry (2026-07-15): the topic clause was added by a
deliberate, disclosed, out-of-band hand-correction, backed up first, specifically to exercise the
mechanical dependency rule for the Contract Composition investigation — not a "Modify decision
text" action inside a `spec` session. The file's mtime confirms this: `adr-001` through `adr-005`
share one mtime (`2026-07-14 09:00:10`, the real spec run); `adr-006` alone carries a later one
(`2026-07-15 09:33:58`, Stage 6). Folding this hand-correction into "how was the domain-event
proposal reviewed" would overstate how much scrutiny the review gate actually gave it — the
correction happened *because* of the reproducibility sweep's finding, days after the fact, not as
part of reviewing the original proposal.

## 2. Comparing against the sweep: did the review gate carry any signal?

The reproducibility sweep classified per-category stability into four tiers; database and backend
technology were tier 1 (identical across 5 runs), domain-event presence and topic-convention
compliance were tier 4 (architectural divergence — present in 3/5, convention-compliant in only
1/5).

The real review history above shows **zero difference in review behavior across that same
spread**: tier-1 categories (database `PostgreSQL`, structural service ownership) and the tier-4
category (domain-event, missing its topic clause) were all Accepted, unmodified, in the same
session, with no visible difference in how much scrutiny either got.

This matches the second of the two possible findings the Roadmap Reassessment named in advance
(§2) as informative either way: **"if they were mostly just Accepted despite being the least
reproducible category measured, that's a different, arguably more important finding — it would
mean the review gate currently gives no signal that a low-reproducibility recommendation is being
rubber-stamped."** That is what happened here, for this one story. The domain-event proposal's
missing topic clause is not a subtle defect a careful reviewer would need to catch — the prompt
itself explains why it's missing ("If no Topic Naming Convention ADR exists, name the event only"),
so a reviewer with no visibility into that upstream state has no way to know the recommendation in
front of them is the single least-reproducible thing the pipeline produces. Nothing on the review
screen (`title`, `decision`, `reason`, `alternatives` — see `prompts/spec.rs:197-210`) distinguishes
a tier-1 recommendation from a tier-4 one.

## 3. Did downstream stages surface it as a Decision Point instead?

No. `stories/manufacturer-001/decisions.yaml` — the Stage 2 "unresolved-decisions-become-explicit-
decision-points" mechanism's output for this story — is `decisions: []`; `decision-audit.yaml` is
`findings: []`. The domain-event determination was never flagged anywhere downstream either. It was
resolved once, in `spec`, with the same undifferentiated Accept as every other proposal, and never
revisited by any later checkpoint until Stage 6's unrelated, sweep-motivated hand-correction.

This is the fact the Roadmap Reassessment's §2 asked to check before deciding §3's Decision Point
question: **the review gap this document confirms is not currently caught by the existing Decision
Point mechanism either.** Whatever `unresolved-decisions-become-explicit-decision-points` already
says about this shape of problem, it hasn't fired here — because Stage 2 operates on a story's
*behaviors*, generated after `spec` has already closed, not on `spec`'s own proposals.

## 4. What this does and doesn't justify concluding

**Justified, for this one story:** the human review gate, as it actually operated on
`manufacturer-001`, gave no differentiated signal between a perfectly-reproducible recommendation
and the single least-reproducible one measured. Every proposal was Accepted verbatim, in one
sitting, regardless of category. No downstream mechanism caught it either.

**Not justified:** any claim about reviewer behavior in general, about a real human user's future
behavior on a differently-styled review screen, or about how a reviewer with the reproducibility
sweep's own findings in hand would review the same proposal today (they might well scrutinize a
domain-event proposal harder now, precisely because this investigation exists). This is a sample
of one story, reviewed once, by whoever was driving that dogfooding session, under the
"accept-every-default" pattern the interactive dogfooding driving convention itself documents as
the norm — not a study of review behavior under realistic incentives. `manufacturer-001` also
never went through an "Accept with edit" or "Reject" path at all, at any stage, so this pass cannot
say anything about whether those paths behave differently when they are actually exercised.

## 5. Recommended next step

Per the Roadmap Reassessment's own item #2, the next question is whether this finding is
sufficient — on its own, or combined with `unresolved-decisions-become-explicit-decision-points`'s
existing applicability criteria — to justify proposing that domain-event determination become an
explicit Decision Point. That is a design question, not a further measurement; this document
supplies the evidence input to it and stops here, per the Roadmap Reassessment's own framing of
this as a narrowly-scoped investigation, not a broad audit.
