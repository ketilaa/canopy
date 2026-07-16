# Does Canopy's Exploration Phase Fail to Enumerate Certain Classes of Questions?

Status: evaluation only. No workflow change, no new stage, no new prompt, and no fix is proposed
here. The question this document answers is narrower than "how do we fix this" — it is whether
"the exploration phase never explicitly checks for some categories of question" is a correct,
evidenced explanation for what the Product-Owner Perspective Experiment found, as distinct from
other explanations (the artifact reviewed being stale relative to current code; the concern being
a deliberate design exclusion, not an oversight; the concern belonging to a different, already-
tracked question).

Date: 2026-07-16

Reviewed: `docs/design/product-owner-perspective-experiment.md`, `docs/design/human-insight-
inventory.md`, `docs/design/pre-behavior-planning-review.md`, `docs/open-questions/domain-
boundary-explicitness.md`, `docs/principles/exhaustive-enumeration-over-holistic-review.md`,
`docs/principles/unresolved-decisions-become-explicit-decision-points.md`, `docs/blog-drafts/
policy-discovery-vs-policy-invention.md`, and — going beyond the document set, since several of
this document's conclusions turn on it — the actual current prompt code:
`canopy-llm/src/prompts/spec.rs` (`entity_schema_prompt`'s business-policy checklist,
`bucket_policy_checklist`, `identify_architectural_questions`), `canopy-llm/src/prompts/
behaviors.rs` (`identify_specification_gaps`'s three checklists), `canopy-core/src/lib.rs`
(`IntentSpec`), and the real dogfooding project's own `stories/manufacturer-001/spec.yaml` and
`.canopy/logs/llm-debug.log` timestamps.

---

# Executive Summary

Two of the four headline concerns the Product-Owner Perspective Experiment raised —
**uniqueness/duplicate-name** and **authorization** — are, per the actual current prompt code,
**already explicitly enumerated**, as two of six fixed items in a business-policy checklist that
runs during `canopy spec`. The real `manufacturer-001` artifact the experiment reviewed was
generated hours **before** that checklist mechanism existed in the codebase (confirmed by comparing
`llm-debug.log`'s timestamp for the real `generate_story_spec` call against the git commit that
introduced the checklist). The experiment's finding for these two concerns is real, but its
explanation is closer to **stale-artifact** than **enumeration gap** — the same shape of finding
this project already reached once before, for the domain-event topic clause.

**Role semantics**, by contrast, genuinely is **not currently enumerated anywhere** — not by the
policy checklist (which presupposes the actor's identity is already settled and only asks about
*additional* permission requirements beyond it), not by any story-generation step, not by the roles
registry (which stores a bare string with no definition). This one concern is the strongest surviving
instance of a genuine enumeration gap.

The **`Product`-relationship** concern is different again: it is not an oversight in an otherwise-
complete enumeration — the domain-extraction step's own prompt *explicitly and deliberately*
excludes entities named only in a story's purpose clause, for reasons this project has independent,
already-validated evidence for (`structure-emerges-from-behavior`'s anticipatory-over-generation
finding). Calling this an "enumeration gap" understates a real design tension: adding this check
would cut against a principle this project already has good reason to hold.

**"Enumeration gap" is a useful, evidenced explanation for a real subset of the Product-Owner
Perspective Experiment's findings — but not for its two most-corroborated ones, and not uniformly
across the rest.** Recommendation at the end: preserve a narrowly-scoped version of this as an open
question, explicitly excluding the two concerns better explained by staleness and the one better
explained by an existing design principle.

---

# Product-Owner Findings Review

Every concern raised in `product-owner-perspective-experiment.md`, extracted individually rather
than only at the four-category level the prior roadmap update used — several of the four headline
categories bundle multiple distinct concerns, and bundling was itself part of what the earlier
"unestablished referent" review found insufficiently scrutinized.

| # | Concern | First appears | Only discoverable holistically? | Enumerable in principle? |
|---|---|---|---|---|
| 1 | Duplicate-name uniqueness scoped to name alone, no distinguishing identifier | Domain-Expert PO (data-modeling angle) | No — see below, a fixed checklist item exists for exactly this | Yes — already enumerated |
| 2 | Duplicate-name failure has no user-facing resolution path | Customer-Outcome PO (usability angle, same underlying rule as #1) | Holistic today | Yes, but a distinct question from #1 (policy resolution vs. UX consequence) |
| 3 | `address` unstructured (single string, not composed fields) | Domain-Expert PO | Holistic today | Yes, loosely |
| 4 | Is `name` legal name, trading name, or free text? | Domain-Expert PO | Holistic today | Weakly — story-specific, hard to reduce to a fixed checklist item |
| 5 | `phoneNumber` has no format validation, only a length cap | Domain-Expert PO | Holistic today | Yes — a missing validation *category* (format/pattern), not just a missing instance |
| 6 | No `taxId`/`registrationNumber` field | Domain-Expert PO | Holistic today | Weakly — this is domain knowledge about manufacturers specifically, not a structural checklist item |
| 7 | No recorded reason for which fields are optional vs. mandatory | Domain-Expert PO | Holistic today | Yes — schema fields carry no `reason`, unlike ADRs |
| 8 | No role/permission model beyond a bare actor string; "authenticated" ≠ "authorized" | Governance-Oriented PO | No — see below, a fixed checklist item exists for exactly this | Yes — already enumerated |
| 9 | No stated access control on the event's PII-bearing payload | Governance-Oriented PO | Holistic today | Yes — no field anywhere in the domain-event ADR schema for payload/consumer/access control |
| 10 | No approval step between submission and effect | Governance-Oriented PO (also Customer-Outcome, #13) | Holistic today | Yes, adjacent to but distinct from the "idempotency" checklist item |
| 11 | No `security` scheme in generated OpenAPI | Governance-Oriented PO | N/A — downstream symptom | Not independently checkable; a consequence of #8, not a separate concern |
| 12 | Undefined relationship to a `Product` entity implied by the story's own `so_that` | Product-Portfolio PO | Holistic today, and holistic review wouldn't have caught it either — see below | Yes narrowly, but in tension with an existing principle — see below |
| 13 | Does `manufacturer-service` own only identity, or eventually the manufacturer-product relationship? | Product-Portfolio PO | Holistic | Cross-story; not this story's own exploration to catch |
| 14 | Manufacturer lifecycle beyond creation (deactivation, merging) never scoped | Product-Portfolio PO | Holistic | Cross-story; deliberately deferred by the intent-decomposition design (see below) |
| 15 | No confirmation of what happens after successful registration (visible immediately? pending?) | Customer-Outcome PO | Holistic today | Yes |
| 16 | Is the registering user actually the beneficiary of their own action? | Customer-Outcome PO | Holistic, and possibly not reducible to a checklist item at all | Unclear |
| 17 | Role-semantics ambiguity: does "manufacturer representative" mean an external or internal actor? | Terminology mapping / implicit across Governance and Customer-Outcome personas | **Yes — genuinely only discoverable holistically today** | Yes — a small, fixed, bounded question |

---

# What Exploration Explicitly Enumerates Today

Grounded directly in current prompt code, not in the documents that describe it — the documents
were re-checked against the code rather than assumed current.

**`identify_architectural_questions`** (`canopy-llm/src/prompts/spec.rs`) enumerates exactly four
categories per story: structural (service ownership; a domain-event ADR, MANDATORY whenever the
architecture is event-driven and the story's action creates/updates/deletes an aggregate), UI (if
a human actor performs the action), tech stack (for every new backend/frontend service), and
infrastructure (database, event broker). Nothing in this enumeration asks about the *actor's own
identity or role definition* — it treats `story.as_a` as a given, fixed fact.

**The business-policy checklist**, embedded in `entity_schema_prompt` (same file), enumerates
exactly six fixed areas, in a fixed order, for every entity-creating story: **uniqueness**
("must any field, or combination of fields, be unique across all existing records of this
entity?"), **defaults**, **retention**, **authorization** ("does creating or modifying this entity
require a specific role or permission *beyond the actor already being authenticated*?"),
**idempotency**, **consistency**. Each item must be classified `resolved` / `not_applicable` /
`unresolved`, and — confirmed directly in `bucket_policy_checklist` — a `resolved` or
`not_applicable` classification with no named `detail`/`evidence` is rejected as invalid output,
not accepted as a lower-confidence guess. `unresolved` items are routed into `IntentSpec.
open_questions`.

**Stage 0 (`identify_specification_gaps`, `canopy-llm/src/prompts/behaviors.rs`)** enumerates
three fixed checklists: constraint coverage (every field × validation-constraint pair, is it
covered by a scenario?), scenario outcome clarity (every scenario, is its `then` observable?), and
open-question resolution (every entry in `spec.open_questions`, is it resolved by an ADR or
scenario?). Confirmed directly in the prompt-building code: Checklist 3 is built from
`spec.open_questions` specifically — if that list is empty, Checklist 3 has zero items to check
and mechanically cannot produce an `unresolved_question` gap, regardless of whether any policy
question was actually resolved with evidence.

**Domain extraction** (`extract_domain_from_stories`, `canopy-llm/src/prompts/intent.rs`)
enumerates entities/events "directly created, read, updated, or deleted by these actions," and its
own prompt text explicitly instructs: "Do NOT extract actors, beneficiaries, or concepts only
implied by purpose or benefit." This is a deliberate exclusion, not an omission — confirmed by
reading the literal instruction, not inferred.

**Intent decomposition** (`generate_stories_from_intent`) treats update/deactivation/lifecycle
operations as belonging to a *separate* future story ("Split into an update story only when the
intent explicitly describes editing an existing record") — lifecycle-beyond-creation is by design
not this story's own concern to enumerate.

**What is enumerated nowhere, confirmed by grep against every prompt file that renders `so_that`**:
whether a story's own `want`/`so_that` text names an entity or relationship absent from
`domain_registry.yaml`. `so_that` is passed into every prompt purely as display/context text; no
prompt cross-checks it against the domain registry.

---

# Candidate Enumeration Gaps

Evaluated, not assumed, per the explicit instruction:

- **Role meaning** (#17): survives as a real candidate gap. Nothing enumerated above asks it; the
  authorization checklist item presupposes the actor's identity is already resolved and only asks
  about *additional* permission on top of it.
- **Ownership relationships** (#13): does not survive as a *within-story exploration* gap — it is
  a cross-story/portfolio question, structurally outside what any current per-story mechanism was
  built to check (Stage 2's Decision Points, e.g., derive only from a story's own already-extracted
  behaviors, never from another story's content).
- **Referenced-but-undefined entities** (#12): partially survives, with a real complication. A
  narrow version of this check ("does `so_that` reference something absent from domain vocabulary")
  is mechanically simple to state — but the domain-extraction step's deliberate exclusion of
  purpose/benefit-only concepts exists for a reason this project has independent, validated evidence
  for (`structure-emerges-from-behavior`'s anticipatory-over-generation finding: extracting
  structure not yet concretely described degraded output quality in an earlier, unrelated part of
  this same pipeline). This is not simply an uncovered checklist item; it is tension between two
  legitimate goals — flagging the discrepancy vs. not anticipating structure ahead of behavior.
- **Uniqueness criteria** (#1): does **not** survive as a current gap — already enumerated.
- **Authorization implications** (#8): does **not** survive as a current gap — already enumerated.
- **Lifecycle assumptions** (#14): does not survive as a within-story gap — deliberately deferred
  to future stories by the intent-decomposition design, the same "don't anticipate" logic as above.

---

# Counter-Evidence

Actively looking for evidence the current pipeline already handles these concerns, per the
explicit instruction not to merely support the hypothesis:

- **Uniqueness and authorization are the strongest counter-evidence in this whole investigation.**
  Both are explicitly named, both have a specific question worded almost identically to what the
  Domain-Expert and Governance-Oriented personas asked, and both are backed by a citation-
  enforcement mechanism (`bucket_policy_checklist`'s hard failure on an ungrounded `resolved`
  classification) that a separate, already-published investigation
  (`docs/blog-drafts/policy-discovery-vs-policy-invention.md`) measured directly: before the
  citation requirement, controlled runs resolved 5–6 of 6 policy questions with fabricated
  specifics; after it, 1–2 of 6, with the rest correctly routed to `open_questions`. This is real,
  measured evidence the *current* mechanism does meaningfully better than "not enumerated at all."
- **The real `manufacturer-001` artifact predates this mechanism, confirmed three independent
  ways**: (1) `IntentSpec.resolved_policies` has no `skip_serializing_if`, so a populated list would
  serialize as `resolved_policies: [...]` — the real `spec.yaml` has no such field at all, not even
  empty, meaning it was written by code that didn't have the field yet. (2) `llm-debug.log` records
  the real `generate_story_spec` call at `2026-07-13T18:39:11Z`; `git log -S` finds the checklist
  introduced by `cef2c96` at `2026-07-13 22:40:47+02:00` — roughly four hours *after* this story's
  spec was generated — and refined the next morning by `dc2f0c2`. (3) The file's own mtime
  (`2026-07-14 20:54:23`) has no corresponding `generate_story_spec`-shaped call anywhere in
  `llm-debug.log` at that time, and the surrounding retrospective record describes a project-wide
  artifact backup around that period (for the later Stage 6 ADR correction) — consistent with a
  file-copy operation bumping mtime without changing content, not a regeneration.
- **`out_of_scope` is a real, working mechanism, actively used in this exact file** — populated with
  two genuine entries ("Handling of invalid email addresses," "Integration with other services for
  data validation"). This is direct counter-evidence against "the pipeline has no way to mark a
  known exclusion" — it does, and uses it. The candidate gap for concern #12 is therefore not "no
  scope-declaration mechanism exists" but the narrower "nothing prompts a check of `so_that` against
  what got extracted, to decide whether a discrepancy belongs in that already-existing field."
- **One remaining unknown this counter-evidence pass could not resolve**: whether the *current*
  policy checklist, run live against `manufacturer-001`'s real story and domain data today, would
  actually classify `uniqueness` and `authorization` as `unresolved` (correct) or would still
  fabricate a resolution (the citation-check failure mode the blog draft's own "Remaining
  Questions" section flags as not fully closed — a citation naming a real source is checked for
  *presence*, not yet for whether it actually supports the specific claim). No live re-run was
  performed for this investigation, consistent with its own charter (evaluation, not verification).
  This is named as an open unknown, not resolved either way by assumption.

---

# Classification Table

| Concern | Classification | Grounding |
|---|---|---|
| #1 Uniqueness / duplicate-name | **Already handled** (in current code; artifact is stale) | `entity_schema_prompt`'s fixed "uniqueness" checklist item; timestamp comparison above |
| #2 Duplicate-name UX dead-end | **Not currently explored** | No prompt asks whether a failure scenario states a resolution path for the user |
| #3 Address structure | **Not currently explored** | No compound/structured-field rule in `entity_schema_prompt`'s validation rules |
| #4 Name legal/trading ambiguity | **Unclear** | Enumerable in principle, but story-specific; no evidence a fixed checklist item could generalize cleanly |
| #5 phoneNumber format validation | **Not currently explored** | Validation rules cover length/min-max only; no format/pattern category exists at all |
| #6 No taxId/registrationNumber | **Unclear**, leaning not-enumerable-as-checklist | Domain knowledge specific to manufacturers, not a structural gap the way uniqueness/authorization are |
| #7 No recorded reason for optional/mandatory choice | **Not currently explored** | Schema fields carry no `reason`/`evidence` field; ADRs do |
| #8 Authorization | **Already handled** (in current code; artifact is stale) | `entity_schema_prompt`'s fixed "authorization" checklist item; same timestamp evidence as #1 |
| #9 Event payload access control | **Not currently explored** | No field anywhere in the domain-event ADR proposal schema for payload contents or consumer access |
| #10 Approval-before-effect | **Not currently explored** | Adjacent to but distinct from "idempotency"; no dedicated check |
| #11 Missing OpenAPI security scheme | **Downstream consequence, not a separate concern** | Follows mechanically from #8 never having been resolved upstream for this story |
| #12 Undefined Product relationship | **Not currently explored, and in tension with an existing principle** | Domain extraction deliberately excludes purpose/benefit-only concepts; `structure-emerges-from-behavior` is the evidenced reason why |
| #13 manufacturer-product ownership | **Out of current per-story scope by design** | Cross-story; no per-story mechanism was ever built to check this |
| #14 Lifecycle beyond creation | **Out of current per-story scope by design** | Intent decomposition explicitly defers non-creation operations to a separate story |
| #15 Post-registration state visibility | **Not currently explored** | No prompt asks what the actor is told after a successful write |
| #16 Registering-user-as-beneficiary | **Unclear**, leaning not-cleanly-enumerable | Requires interpretive judgment about a story's own stated purpose, not obviously reducible to a yes/no item |
| #17 Role semantics | **Not currently explored — the clearest surviving gap** | No mechanism anywhere asks what an actor label denotes; the one adjacent check (authorization) presupposes it's already settled |

---

# Assessment

**Is "enumeration gap" a useful explanation?** Yes, but only for part of what the Product-Owner
Perspective Experiment found — and it is a *worse* explanation than the two clear alternatives
(staleness; deliberate design exclusion) for exactly the two most-corroborated findings in that
experiment.

**Which concerns are best explained by it?** Role semantics (#17) most cleanly of all — a small,
bounded, genuinely-never-asked question with no competing explanation. Behind it, a cluster of
concretely enumerable but currently-missing checklist items: event payload/access-control
specificity (#9), approval-timing (#10), post-registration visibility (#15), format validation as
a category (#5), and schema-decision reason-recording (#7). Each of these is the same shape as the
existing, working policy checklist — a fixed, small, nameable question — just not currently one of
its six items.

**Which concerns are not?** Uniqueness (#1) and authorization (#8) — both already enumerated;
their appearance in this experiment is much better explained by the reviewed artifact predating
the mechanism than by any current gap. The `Product`-relationship concern (#12) and the
manufacturer lifecycle/ownership questions (#13, #14) are not enumeration gaps in the same sense —
they are cross-story visibility problems, one of them in direct tension with an already-validated
principle against anticipatory extraction, not items missing from an otherwise-complete per-story
checklist. Name ambiguity (#4), the missing tax/registration identifier (#6), and the registering-
user-as-beneficiary question (#16) are borderline — plausible as concerns, but not clearly reducible
to the same fixed-checklist-item shape that makes uniqueness/authorization/role-semantics tractable
in the first place.

**Is there enough evidence to preserve this as an open question?** Yes, for a version scoped to
what actually survived this review — role semantics as the central, best-evidenced item, with the
smaller enumerable cluster (#5, #7, #9, #10, #15) as adjacent, lower-priority instances of the same
shape. Not for uniqueness/authorization, which have a different, better-fitting explanation and a
concrete next step of their own (a live check, not a new enumeration). Not for the `Product`-
relationship/lifecycle/ownership cluster, which already has a home
(`docs/open-questions/domain-boundary-explicitness.md`) and a real, unresolved tension with an
existing principle that a same-shaped "just add a checklist item" framing would obscure rather than
resolve.

---

# Recommendation On Whether This Should Remain An Open Question

**Yes, preserve it — narrowly.** The evidence supports treating "does exploration explicitly
enumerate role/actor semantics, and a small adjacent cluster of similarly-shaped, currently-missing
checklist items" as a genuine, real, currently-open question worth not losing. It should **not**
be filed as a restatement of the four-category PO-experiment framing, and it should **not** absorb
the uniqueness/authorization findings (better explained by staleness, with their own distinct next
step: verify live behavior against current code) or the `Product`/ownership/lifecycle findings
(already tracked elsewhere, and analytically in tension with an existing principle rather than a
clean gap). This document does not itself file that open-questions entry — per its own charter, it
evaluates and classifies; filing is a separate action left for an explicit next step.
