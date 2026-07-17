# Inventory of Recurring Concern Categories Introduced After the First Story Exists

Status: evidence inventory only. No exploration mechanism, workflow, or solution proposed. Answers
what categories of concern humans (or simulated personas standing in for them) repeatedly raised
across the Product-Owner Perspective Experiment, Human-Insight Inventory, Role Meaning experiment,
and Human Insight Process Experiment, that were absent from the original story/intent as generated.

Date: 2026-07-17

---

# A Methodological Caveat That Has to Govern How This Inventory Is Read

Before the categories themselves: the four investigations named do not use comparable methods, and
that difference directly limits what "recurrence" can honestly mean here. The Product-Owner
Perspective Experiment used **passive review** — five personas critiquing a finished artifact set
for `manufacturer-001`, explicitly looking for gaps. The Human Insight Process Experiment used
**active decision-making** — five personas making real choices through a live session for
`return-001`, never asked to critique output the way the first experiment's personas were. The
Human-Insight Inventory examined **real review history**, not simulated critique at all. The Role
Meaning experiment tested **one specific, pre-selected concern**, not an open search for gaps.

This means: a concern category that appears in the Product-Owner Perspective Experiment but not in
the Human Insight Process Experiment cannot be read as "it didn't recur on the second story" — it
may simply mean the second experiment's methodology was never structured to surface it, since it
never asked personas to review and critique the way the first one did. **Only one category in this
inventory was found by a method independent of review-style critique in both cases** — checking a
story's own text directly against the domain registry — and it is marked accordingly below. Every
other category's "systemic vs. incidental" judgment is qualified by this caveat, not asserted past
it.

---

# Category Inventory

## Authorization / Authorization-Beyond-Authentication

- **Appearances**: Product-Owner Perspective Experiment (Governance-Oriented persona: "authenticated
  as what, and authorized to do what? ... Nothing decides this," and the OpenAPI spec's confirmed
  missing `security` scheme). Echoed indirectly in the Human Insight Process Experiment through
  `risk_averse`'s and `compliance`'s verification/entitlement-shaped facts, and directly targeted
  by the entire Role Meaning investigation, which established authorization's *resolution*
  specifically depends on role identity being known.
- **Existing mechanism**: Yes — confirmed directly in code (Enumeration Gap Investigation):
  `authorization` is one of six fixed checklist areas the pipeline already enumerates and attempts
  to resolve.
- **Downstream effect**: Confirmed directly — the Role Meaning Value Experiment showed supplying
  role identity moved `authorization` from correctly unresolved to citation-backed resolved, with a
  corresponding new rejection scenario.
- **Systemic or incidental**: The *concept* recurs, but what's actually missing is narrower than
  "authorization as a category" — the mechanism exists; what's absent is one specific input
  (role identity) needed to resolve it meaningfully. This is the one category in this inventory
  already investigated to a firm, evidence-backed conclusion rather than left open.

## Downstream Consumers / Event Payload and Access Control

- **Appearances**: Product-Owner Perspective Experiment only (Governance-Oriented persona: "who
  consumes `ManufacturerRegistered`, and what does the payload contain?" — PII fields broadcast
  with no stated access control). Weakly echoed in the Human Insight Process Experiment
  (`growth_retention`'s fact implied a downstream consumer cares about captured return reasons, but
  no persona in that experiment ever posed the question explicitly the way the first experiment's
  Governance persona did).
- **Existing mechanism**: None found. No checklist area, ADR category, or Decision Point mechanism
  addresses event-consumer identity or payload access control.
- **Downstream effect**: Never measured — no experiment tested what happens if this is surfaced.
- **Systemic or incidental**: Cannot be determined from current evidence — appeared explicitly only
  once, and its absence from the second experiment is confounded by the methodology gap noted
  above, not evidence the concern is story-specific.

## Approval / Accountability

- **Appearances**: Product-Owner Perspective Experiment only (Governance-Oriented persona: "is
  there an approval step between submit and accept," "who signed off on this manufacturer being in
  our system"). No clear analog raised in the Human Insight Process Experiment.
- **Existing mechanism**: None found.
- **Downstream effect**: Never measured.
- **Systemic or incidental**: Weakest evidence base in this inventory for recurrence — a single
  appearance, in the one experiment structured to surface this kind of gap at all. Cannot be
  distinguished from a methodology artifact.

## Ownership

Two distinct senses appeared, worth separating rather than merging:

- **Architectural/service ownership** — "does `manufacturer-service` own only manufacturer
  identity, or eventually the manufacturer-product relationship too?" (Product-Portfolio persona,
  Product-Owner Perspective Experiment). Not raised again in the Human Insight Process Experiment.
- **Record/entitlement ownership** — "what proves this specific actor may act on this specific
  record?" This is the shape `risk_averse`'s fact actually took in the Human Insight Process
  Experiment (verified order/purchase confirmation), and it is the one instance in this inventory
  where a concern from this general family produced the cleanest measured downstream effect of any
  persona fact tested (cited verbatim in two resolved policies).
- **Existing mechanism**: The entitlement-ownership sense already maps onto existing checklist
  areas (`uniqueness`, `consistency`) — confirmed by `risk_averse`'s own successful citation. The
  architectural-ownership sense has no dedicated mechanism; it is closer to
  `domain-boundary-explicitness.md`'s own still-open question about service-boundary drawing.
- **Downstream effect**: Confirmed for entitlement-ownership (`risk_averse`'s result). Never
  measured for architectural ownership.
- **Systemic or incidental**: The entitlement-ownership sense is real but not a *new* category —
  it's already-working machinery, exercised successfully once. The architectural-ownership sense
  has the same single-appearance limitation as Approval/Accountability above.

## Lifecycle Concerns (State Beyond Creation)

- **Appearances**: Both stories, in different shapes. `manufacturer-001`: manufacturer
  deactivation, merging duplicate records, post-registration state visibility (Product-Portfolio
  and Customer-Outcome personas). `order-001`: return-window timing (`operational`, `compliance`
  personas), and post-decision flow — an exchange offer preceding a refund (`growth_retention`).
- **Existing mechanism**: Partial — `retention` is one of the six enumerated checklist areas
  (timing/expiry), but ownership-transition, merging, and multi-step post-decision flows have no
  dedicated mechanism.
- **Downstream effect**: `growth_retention`'s lifecycle-shaped fact produced the strongest content-
  level scenario match of any persona in the Human Insight Process Experiment, though it never
  resolved a citable policy the way `risk_averse`'s did.
- **Systemic or incidental**: The strongest candidate for genuine cross-story recurrence among the
  "new" categories, since it appeared — in different specific forms — in both stories, under both
  methodologies (review-critique and active decision-making alike). Worth flagging as the most
  evidence-supported systemic pattern in this inventory besides the domain-boundary gap below.

## Domain Boundary / Story-Vocabulary Discrepancy

- **Appearances**: Both stories, found by a method independent of persona review in either case —
  `manufacturer-001`'s `so_that` names `Product`, never extracted into domain vocabulary (found
  during the Product-Owner Perspective Experiment); `order-001`'s text names both `Product` and
  `Order`, neither extracted (found, unprompted, during the Human Insight Process Experiment's own
  setup work, not sought for this purpose).
- **Existing mechanism**: None. Confirmed by direct code reading (Enumeration Gap Investigation) —
  no prompt or checklist checks a story's own language against domain vocabulary at all.
- **Downstream effect**: Never measured.
- **Systemic or incidental**: **The single most robustly evidenced recurring category in this
  entire inventory.** Unlike every other category above, this was not found by asking a persona to
  look for it — it was confirmed directly against real artifacts in two independent, differently-
  motivated investigations. This is the one category the methodological caveat does not weaken.

## Role Semantics / Terminology Ambiguity

- **Appearances**: `manufacturer-001` only, and deliberately *not* retested on `order-001` — the
  Human Insight Process Experiment's own design explicitly chose a clean, unambiguous role
  (`customer`) specifically to avoid reproducing this already-known ambiguity, per its own stated
  method.
- **Existing mechanism**: Investigated exhaustively elsewhere in this chain — a real, unpopulated
  storage location exists (`Role::Described`), and the concern has already been reduced,
  through direct evidence, to a classification problem.
- **Downstream effect**: The most thoroughly measured of any category in this entire inventory —
  see the Role Meaning investigation in full.
- **Systemic or incidental**: Deliberately controlled out of the second experiment, not absent from
  it — its single-story appearance should not be read as narrow; it is simply the one category
  already carried to a separate, complete conclusion.

## Business-Rule Scoping (Uniqueness / Identity Criteria)

- **Appearances**: Both stories — `manufacturer-001`'s duplicate-name concern (Domain-Expert,
  Customer-Outcome personas); `order-001`'s uniqueness scoping across multiple personas
  (`customer_experience`'s customerId+orderId scope, `compliance`'s order+customer scope).
- **Existing mechanism**: Yes — confirmed directly in code as one of the six enumerated checklist
  areas.
- **Downstream effect**: Mixed — resolved reliably in some persona runs, fell through to
  unresolved in others, consistent with ordinary model variance already documented elsewhere in
  this chain, not with the category itself being unaddressed.
- **Systemic or incidental**: A clear negative case, useful for calibration: this concern recurs
  across both stories, but it is **already mechanized** — its recurrence is evidence the existing
  checklist gets exercised often, not evidence of a gap needing new capability.

## Rationale/Justification Tracking

- **Appearances**: `manufacturer-001` only ("no indication of *why* `phoneNumber`/`email`/
  `website` are optional rather than mandatory" — Domain-Expert persona). No analog raised in the
  Human Insight Process Experiment.
- **Existing mechanism**: Partial — ADRs record a `reason` field for architecture decisions;
  nothing analogous exists for individual entity-schema field choices.
- **Downstream effect**: Never measured.
- **Systemic or incidental**: Weakest evidence base of any category in this inventory — a single,
  narrow observation, never independently checked again.

---

# Summary

Ranked by strength of recurrence evidence, not by perceived importance:

1. **Domain Boundary / Story-Vocabulary Discrepancy** — the only category confirmed by a method
   independent of persona review, in both stories. The methodological caveat does not apply to it.
2. **Lifecycle Concerns** — appeared in different specific forms across both stories and both
   methodologies, the strongest genuinely cross-methodology signal after the category above.
3. **Business-Rule Scoping (Uniqueness)** — recurs reliably, but is a calibration case, not a gap:
   already mechanized, already exercised.
4. **Authorization** — recurs, but has already been reduced to a specific, narrower, already-
   investigated sub-question (role identity) rather than remaining an open category.
5. **Ownership (entitlement sense)** — recurs, and the one instance tested produced the cleanest
   downstream effect of any concern in this inventory outside Role Meaning itself — but, like
   uniqueness, it turned out to already have working machinery once supplied with concrete content.
6. **Downstream Consumers, Approval/Accountability, Ownership (architectural sense), Rationale
   Tracking** — each appeared once, exclusively within the one experiment structured to surface
   this kind of observation. Their absence elsewhere cannot be distinguished from the methodology
   never looking for them a second time. Genuinely open, not incidental — just insufficiently
   tested to call systemic.
