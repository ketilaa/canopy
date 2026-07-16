# Product-Owner Perspective Experiment

Status: observation only. No implementation, no redesign, no fix proposed. Answers one question —
where does human insight actually need to enter this pipeline's output, and does Canopy's current
behavior look like "generate the boilerplate, leave humans the insight," or something else?

Date: 2026-07-16

Method note: five simulated Product Owner personas, distinct in background and priority, review
the same real, already-generated artifact set. This is explicitly not a software-generation,
composition, or reproducibility experiment — no new LLM calls were made, nothing was regenerated.
Personas react to what Canopy already produced for one real story, as a Product Owner actually
would: at the level of business meaning, not architecture or implementation. Two prior
investigations already examined this same story from adjacent but different angles — the
Human-Insight Inventory (`docs/design/human-insight-inventory.md`) measured *how a proposal was
actually reviewed* (Accept/Modify/Reject, historically); this experiment asks *what a reviewer with
real domain judgment would notice*, independent of what was historically clicked. The two are
complementary, not duplicates.

---

# Experiment Design

**Case selection**: `manufacturer-001`, the real dogfooding project's only story with a complete,
real artifact set through the specification stage — not a constructed example. Reusing it here
keeps this experiment grounded in genuine model output rather than a hand-built fixture, and lets
findings be checked directly against the artifacts cited below.

**What personas review**: everything a Product Owner would plausibly be shown or asked to approve
between `intent` and the end of `spec` — the user story, the domain vocabulary, the *business-
meaningful* architecture decisions (service ownership, UI, the domain event), the entity schema,
and the BDD scenarios. Personas explicitly do **not** weigh in on the technology-stack ADRs
(Spring Boot, PostgreSQL, Vitest, Redpanda) — a real Product Owner has no basis to accept or reject
a database choice, and asking one to would misattribute an architecture decision to the wrong
reviewer. This exclusion is itself a finding, addressed in Cross-Persona Analysis.

**What personas do not do**: propose fixes, redesign the pipeline, or say what Canopy "should" do
differently. Each persona produces a reaction, not a recommendation for Canopy's engineering team.
Synthesis (where insight enters, what looks like boilerplate) happens only after all five reviews
are recorded independently.

---

# Selected Case

**Story** (`stories.yaml`):
```
id: manufacturer-001
as_a: manufacturer representative
want: register a manufacturer
so_that: products can reference them in the system
status: accepted
```

**Original intent** (`idea.yaml` context + the behavioral statement given at `intent` time):
"An e-commerce platform to demonstrate modern architecture and tech stacks." /
"Manufacturers must be registered in the system before products can reference them."

**Domain vocabulary** (`domain_registry.yaml`): entity `Manufacturer`, event
`ManufacturerRegistered`. Nothing else has ever been added to this project's domain registry —
`Manufacturer` is the only entity that has ever existed here.

**Business-meaningful architecture decisions** (`decisions/adr-00{1,2,6}`):
- ADR-001: service ownership → `manufacturer-service`, "the primary data owner," alternatives
  considered were `admin-service` and `product-service`.
- ADR-002: UI → `manufacturer-registration-portal`, alternatives `admin-portal`,
  `manufacturer-portal`.
- ADR-006: domain event → `ManufacturerRegistered on topic manufacturer.registered`.

**Entity schema** (`stories/manufacturer-001/spec.yaml`): `Manufacturer` — system-generated `id`,
`createdAt`, `modifiedAt`; mandatory `name` (1–200 chars), `address` (1–2000 chars); optional
`phoneNumber` (≤20 chars), `email` (≤200 chars), `website` (≤200 chars).

**Scenarios** (12 total): one happy path with all fields, one happy path with mandatory fields
only, one missing-name failure, one missing-address failure, one duplicate-name failure, and seven
field-length boundary failures. Every scenario's `given` includes "The manufacturer representative
is authenticated." `out_of_scope`: "Handling of invalid email addresses," "Integration with other
services for data validation." `open_questions`: empty. Stage 0's own completeness check
(`completeness.yaml`) found zero gaps. The generated OpenAPI spec has no `security` scheme defined
anywhere on the `POST /manufacturers` operation.

---

# Personas

The five personas requested, held fixed across the review — no persona is an architect, developer,
or operations engineer; all five are doing Product Owner work, differentiated only by background,
priority, and what kind of thing they instinctively notice first.

1. **Delivery-Focused PO** — wants to ship, defaults to trusting a sensible-looking proposal,
   reads for "does this get us to a demo," not for edge cases.
2. **Domain-Expert PO** — has run supplier/vendor operations before, reads for whether the
   vocabulary and relationships actually match how the business works.
3. **Governance-Oriented PO** — thinks first about who is allowed to do what, what's auditable,
   and who's accountable if something goes wrong.
4. **Product-Portfolio PO** — thinks about this capability's place in a bigger system, what
   adjacent capabilities it implies, what it will need to connect to later.
5. **Customer-Outcome PO** — cares whether the actual business problem gets solved and whether the
   experience makes sense for whoever is on the other end of this workflow, not the internal shape
   of the solution.

---

# Per-Persona Review

## 1. Delivery-Focused Product Owner

### Accept
- The entity schema as a whole — mandatory `name`/`address`, optional `phoneNumber`/`email`/
  `website` — reads like a sensible, unremarkable "register a supplier" form. Nothing here looks
  wrong or worth debating.
- The 200/2000/20/200/200-character length limits — arbitrary-looking but harmless defaults; not
  worth a conversation.
- The happy-path and missing-mandatory-field scenarios (01–04) — obviously correct, matches what
  "register a manufacturer" means at face value.
- Service/UI naming (`manufacturer-service`, `manufacturer-registration-portal`) — fine, ships
  today, can rename later if it ever matters.

### Modify
- Nothing. This persona's whole disposition is to not modify a proposal that looks reasonable —
  that's the point of being delivery-focused.

### Questions Raised
- "Can we ship this now?" — essentially the only question. Reads `completeness.yaml`'s "gaps: []"
  as confirmation there's nothing left to check, not as a claim that needs independent verification.

### Missing Information
- None *noticed* — this persona's blind spot is structural, not a specific gap. The absence of an
  authorization scheme on the API, the unexplained duplicate-name rule, and the undefined
  relationship to `Product` (see other personas below) all pass by unremarked, not because they were
  weighed and dismissed, but because nothing on the page prompts this persona to look for them.

### Domain Insights Added
None. This is the expected, honest result for this persona — its value is velocity, not insight,
and it contributes none here because nothing in the artifact set actively surfaces a decision that
needs business judgment. Everything reads as already-decided.

---

## 2. Domain-Expert Product Owner

### Accept
- The core entity shape (`name`, `address` mandatory; contact fields optional) — matches how a
  manufacturer/supplier record is actually structured in practice.
- `ManufacturerRegistered` as the domain event name and its lifecycle framing — correctly
  identifies this as a creation event, not a generic CRUD notification.

### Modify
- **The duplicate-name rejection rule (scenario 05) needs a scope qualifier, not a blanket
  rejection.** "No manufacturer with the same name exists" as a global uniqueness constraint is
  the kind of rule that looks fine in a demo and breaks in the field: two genuinely different
  manufacturers can share a name (a regional subsidiary, a common surname-based company name, a
  rebrand that collides with an unrelated existing entry). This persona would push back: uniqueness
  should be scoped to something more specific — a registration number, a tax ID, or at minimum
  name *plus* address — not name alone.
- **`address` as a single 2000-character string is under-modeled for a field this persona would
  expect to be structured** (street/city/region/postal code/country) — not because 2000 characters
  is wrong, but because an unstructured address blocks any future capability (shipping cost
  estimation, region-based reporting, duplicate detection by normalized address) that assumes
  structure exists.

### Questions Raised
- Is `name` the manufacturer's registered legal name, a trading/brand name, or whatever the
  representative typed? The schema doesn't distinguish, and the duplicate-name rule's correctness
  depends entirely on which one it is.
- Why is `phoneNumber` optional but not validated for format at all (just a length cap)? A
  domain-expert PO managing supplier data expects at least a shape check, even if full validation
  is out of scope.

### Missing Information
- No `taxId`/`registrationNumber`/similar business identifier anywhere in the schema — the single
  field most domain-expert reviewers of a real manufacturer-onboarding flow would expect to see
  first, and its absence is exactly what makes the duplicate-name rule fragile.
- No indication of *why* `phoneNumber`, `email`, and `website` are optional rather than mandatory —
  is that a business decision (some manufacturers really can't be reached by phone) or just "the
  model made every non-obviously-mandatory field optional"? Nothing in `spec.yaml` records a
  reason for this choice the way ADRs record a reason for architecture choices.

### Domain Insights Added
The uniqueness-scope concern and the address-structuring concern are both genuine domain insight —
neither is derivable from the artifacts themselves; both come from this persona's own experience of
what goes wrong when a manufacturer registry is used for real. This is the clearest single instance
across all five personas of insight that could not plausibly have come from the generated content.

---

## 3. Governance-Oriented Product Owner

### Accept
- The event name and topic (`ManufacturerRegistered on topic manufacturer.registered`) as a
  concept — an audit trail of registration events is exactly the kind of thing this persona wants
  to exist.
- The existence of field-level validation (length limits, mandatory/optional split) as a baseline —
  better than no validation at all.

### Modify
Nothing in the schema itself needs rewording — this persona's concerns are almost entirely about
what's *absent*, not what's *wrong* in what's present.

### Questions Raised
- **Every single scenario's `given` states "The manufacturer representative is authenticated" —
  authenticated as what, and authorized to do what?** There is no role/permission concept anywhere
  in this project beyond the single string `manufacturer representative` in `roles.yaml`. Is
  registering a manufacturer open to anyone with an account, or does it require a specific
  privilege? Nothing decides this.
- **Who consumes `ManufacturerRegistered`, and what does the event payload contain?** The ADR names
  the event and its topic but says nothing about payload contents. `name`, `address`, `phoneNumber`,
  and `email` are all real-world PII/business-sensitive fields — broadcasting them onto an event
  bus with no stated access control on either the publishing or the consuming side is a real,
  specific compliance question this persona would refuse to let pass silently.
- **Is there an approval step between "representative submits" and "manufacturer is registered,"**
  or does submission == acceptance? Every scenario treats registration as an immediate, unmediated
  write. For a persona focused on accountability, "who signed off on this manufacturer being in our
  system" is a natural question with no visible answer.
- The generated OpenAPI spec confirms this isn't just an omission in prose: `POST /manufacturers`
  has no `security` scheme defined at all. This is directly checkable, not inferred.

### Missing Information
- No authorization model, no approval/review workflow, no data-classification note on the PII
  fields being collected and published, no retention statement. All four are the kind of gap this
  persona would flag as blocking, not merely worth discussing.

### Domain Insights Added
None of this is "domain insight" in the vocabulary sense the Domain-Expert PO adds — it's a
different category entirely: structural/governance gaps that exist regardless of what a
manufacturer actually is. This persona's contribution is best read as *identifying an entire
category of decision the artifacts never attempt to make*, not correcting a specific one that was
made wrong.

---

## 4. Product-Portfolio Product Owner

### Accept
- `manufacturer-service` as a standalone service, separate from a hypothetical `product-service` —
  the right instinct for a capability that will likely be reused by more than one downstream
  consumer.

### Modify
Nothing to reword — this persona's contribution is almost entirely about what isn't there yet, at
the level of *entities and relationships*, not fields.

### Questions Raised
- **The story's own stated purpose — "so that products can reference them in the system" —
  describes a relationship to `Product` that doesn't exist anywhere.** `domain_registry.yaml` has
  never contained a `Product` entity. The entire justification for this story presupposes a
  relationship this pipeline has never modeled, and nothing flags that as a gap.
- Is `manufacturer-service` intended to own only manufacturer identity, or will it eventually own
  the manufacturer-product relationship too? If a future `Product` entity references a
  `Manufacturer`, does that reference live in `manufacturer-service`, a future `product-service`, or
  a separate association? This is exactly the kind of question `docs/open-questions/
  domain-boundary-explicitness.md` was written to preserve — this persona reproduces that same
  concern independently, from a portfolio-thinking angle rather than an architecture-analysis
  angle, which is itself worth noting as convergent evidence.
- Does `manufacturer-service` also need to support manufacturer *deactivation* or *merging* (the
  duplicate-manufacturer problem the Domain-Expert PO raised, from a different angle — if two
  manufacturer records for the same real company get created, is there ever a path to merge them)?
  Nothing in the current scope mentions a manufacturer's lifecycle beyond creation.

### Missing Information
- No `Product` entity, no stated relationship shape, no signal anywhere about whether future
  capabilities (catalog browsing, product-manufacturer search, manufacturer deactivation) were ever
  considered — not because they need to be built now, but because none of this artifact set even
  names them as deliberately out of scope. `out_of_scope` lists two narrow items (invalid email
  handling, external data-validation integration) and neither is this.

### Domain Insights Added
The `Product` relationship gap is the single most consequential finding of this persona's review,
and arguably of the whole experiment — it is a real domain-boundary question, evidenced directly by
the story's own `so_that` field, not speculative. This persona reaches the same underlying concern
`domain-boundary-hypothesis-assessment.md` reasoned about abstractly (whether service-ownership
proposals silently encode unresolved boundary decisions), but reaches it concretely, from one real
story's actual content, rather than from cross-run naming variance. That convergence — two very
different methods landing on the same specific gap — is stronger evidence than either alone.

---

## 5. Customer-Outcome Product Owner

### Accept
- The two happy-path scenarios (all fields; mandatory-only) — these actually describe an outcome a
  real manufacturer representative would recognize: "I filled in what I have, and it worked."
- Clear, field-specific error messages in every failure scenario ("indicating that the name is
  required," "indicating that name must be at least 1 character") — good for an actual user
  encountering a rejected submission.

### Modify
- **The duplicate-name failure scenario's outcome is a dead end for the actual user.** Scenario 05
  just rejects with "name must be unique" — but if two legitimately different companies share a
  name (the Domain-Expert PO's exact same underlying observation, from a usability angle instead of
  a data-modeling angle), what does the representative do next? Nothing in the scenario describes
  a resolution path (disambiguate by adding a distinguishing detail, contact support, request
  merge) — just a rejection with nowhere to go.

### Questions Raised
- Does successful registration give the representative any confirmation of *what happens next*
  (is the manufacturer immediately visible to product teams, pending some review, live
  immediately)? The scenarios describe the write succeeding; none describe what the representative
  is told about downstream state.
- Who actually benefits from this capability being fast versus being verified? The story's `so_that`
  is about products being able to reference manufacturers — a downstream, internal benefit — not
  about anything the manufacturer representative themselves gets out of registering. Is the
  registering user even the beneficiary of their own action, or purely a means to an internal end?
  That's worth naming, since it changes what "good UX" even means here.

### Missing Information
- No scenario describes the representative's experience of *not knowing whether they're allowed to
  do this in the first place* — connects to the Governance-Oriented PO's authorization question,
  but from the angle of "what does a rejected/blocked user actually see," which is a UX gap
  regardless of how the authorization question itself gets resolved.
- No outcome-level success criterion beyond "the write succeeded" — nothing about how quickly
  products can actually reference a newly-registered manufacturer, which is the entire stated
  reason this story exists.

### Domain Insights Added
The "registering user isn't the beneficiary" observation is a genuine outcome-level insight, not
derivable from the schema — it comes from reading the story's own `so_that` field critically rather
than taking it at face value. The duplicate-name dead-end is the same underlying gap the
Domain-Expert PO found, independently reached from a different angle (usability vs. data
correctness) — worth noting as the second instance of two personas converging on one gap from
different reasoning paths (the first being the `Product`-relationship gap above).

---

# Cross-Persona Analysis

**Areas everyone accepts, unremarked:** the basic entity shape (mandatory `name`/`address`,
optional contact fields), the happy-path and missing-mandatory-field scenarios, and the domain
event's existence as a concept. Every persona treats these as settled without needing to weigh in —
this is the clearest boilerplate-shaped territory in the whole artifact set.

**Areas everyone — or nearly everyone — questions, independently:** two gaps were reached by
*multiple* personas via entirely different reasoning paths, which is stronger signal than any one
persona's individual concern:
- The **duplicate-name uniqueness rule** was flagged by the Domain-Expert PO (data-modeling
  fragility — two real companies can share a name) and the Customer-Outcome PO (usability
  dead-end — no resolution path for the rejected user) independently.
- The **undefined relationship to `Product`** was flagged by the Product-Portfolio PO (portfolio/
  boundary reasoning, from the story's own `so_that` field) and echoes, from a completely different
  method, the domain-boundary hypothesis already raised abstractly in
  `docs/design/domain-boundary-hypothesis-assessment.md`.

**Areas only certain personas challenge:** the authorization/PII/audit gap is raised only by the
Governance-Oriented PO — none of the other four personas notice it, even though "the manufacturer
representative is authenticated" appears in literally every scenario's `given` clause. This is a
sharp illustration of how persona-dependent this category of gap is: it's not that the information
is hidden, it's stated in every scenario, but only one reviewing lens treats it as a question rather
than a fact already settled.

**A structural observation, not attributable to any one persona:** three of the six generated
ADRs for this story (tech stack, database, testing strategy) were never reviewed by any persona at
all, by this experiment's own design — no Product Owner has the standing to approve a database
choice. But the *review mechanism itself*, per the Human-Insight Inventory's prior finding, treats
all six ADRs identically: same Accept/Modify/Reject gate, same lack of differentiated scrutiny.
This experiment's persona split reveals a category distinction (which proposals are even *in scope*
for a PO to judge) that the existing review gate doesn't encode anywhere.

---

# Human Insight Inventory Findings

Mapping every insight/question surfaced above into the requested categories. An item appears once,
under the category that best explains *why* a human noticed it (several items are cross-listed
where two categories both apply, noted explicitly):

**Business rules**
- Duplicate-name uniqueness scoped globally, with no distinguishing identifier (Domain-Expert PO)
  and no user-facing resolution path (Customer-Outcome PO) — the single most-corroborated finding
  in this experiment.
- Whether registration is immediate-effect or requires an approval/review step before a
  manufacturer is "live" (Governance-Oriented PO; Customer-Outcome PO's "what happens next"
  question is the same gap from the user's side).
- Why contact fields are optional — a business choice with no recorded reason (Domain-Expert PO).

**Authorization**
- No role/permission model beyond a single actor string; "authenticated" stated everywhere,
  "authorized to do what" answered nowhere (Governance-Oriented PO).
- No stated access control on the `ManufacturerRegistered` event's PII-bearing payload
  (Governance-Oriented PO).
- No security scheme on the generated OpenAPI operation — directly checkable, not inferred
  (Governance-Oriented PO).

**Ownership**
- Whether `manufacturer-service` owns only manufacturer identity, or eventually the
  manufacturer-product relationship too (Product-Portfolio PO) — overlaps directly with Domain
  Boundaries below; listed here for the ownership-specific framing (which service is accountable
  for which fact).

**Domain boundaries**
- The undefined relationship to a `Product` entity that has never existed in this project's domain
  vocabulary, despite being the story's own stated reason to exist (Product-Portfolio PO) — the
  most consequential single finding, and independently convergent with prior architectural analysis
  (`domain-boundary-hypothesis-assessment.md`).
- Manufacturer lifecycle beyond creation (deactivation, merging two duplicate records) never
  considered in scope or out of scope (Product-Portfolio PO).

**Terminology**
- Whether `name` means legal/registered name, trading name, or free text — directly determines
  whether the duplicate-name rule is even meaningful (Domain-Expert PO).
- Role-semantics ambiguity in `manufacturer representative` itself: does this denote an external
  actor (an employee of the manufacturer, self-registering their own company — implying a trust/
  verification boundary) or an internal actor (a catalog/admin team member registering manufacturers
  on the business's behalf — a routine internal action)? Nothing in `roles.yaml` or the story
  disambiguates this, and the two readings have entirely different authorization implications —
  connecting directly to the Governance-Oriented PO's concern, but the terminology question is
  logically prior to it (you cannot design the authorization model until you know which actor this
  is).

**Architecture**
- None directly raised by any PO persona, by design — this category was deliberately routed away
  from these reviewers. The three tech-stack/database/testing ADRs exist but were correctly outside
  every persona's scope.

**Integration**
- What (if anything) consumes `ManufacturerRegistered`, and what contract exists between publisher
  and consumer beyond a topic name (Governance-Oriented PO, from an access-control angle; also
  implicit in the Product-Portfolio PO's future-capabilities question).

**Other**
- The address field's lack of internal structure — not wrong, but blocks future capabilities that
  would assume structure (Domain-Expert PO).
- Whether "fast" or "verified" registration serves the actual beneficiary of this story, given the
  registering user isn't obviously who benefits (Customer-Outcome PO).

---

# Canopy Effectiveness Assessment

**Boilerplate Canopy handles well, per this experiment:** the mechanical shape of a registration
capability — field-level validation with sensible types and length constraints, a clean happy path,
missing-mandatory-field and boundary-condition failure scenarios, correctly-generated OpenAPI
request/response shapes, a plausibly-named service and event. Every persona converged on accepting
this layer without debate. This matches the original vision's own "boilerplate generation" framing
directly, and the personas found nothing to disagree with here.

**Areas that still require substantial human insight, per this experiment:** four distinct
categories, each identified independently by at least one persona and two of them independently
corroborated by a second persona reasoning a completely different way:
1. **Business-rule scoping** (the duplicate-name rule) — a rule Canopy generated confidently and
   completely, with no signal that it's under-specified, until challenged by domain and usability
   reasoning simultaneously.
2. **Authorization and data governance** — not generated at all, in any form, despite every
   scenario textually depending on "authenticated" as a precondition. This is the sharpest
   boilerplate/insight boundary found: the *word* "authenticated" is boilerplate; the *meaning* of
   who's authorized to do what was never asked.
3. **Domain-boundary/ownership** — the undefined `Product` relationship, which the story's own
   justification depends on but which the domain vocabulary has never captured. This is the
   clearest evidence in this whole experiment that a domain-boundary question can hide inside an
   otherwise fully-resolved-looking artifact set, corroborating the more abstract reasoning in
   `domain-boundary-hypothesis-assessment.md` with a concrete instance.
4. **Role semantics** — whether the sole named actor is an external, self-registering party or an
   internal operator. This sits underneath the authorization question and the business-rule
   question both, and nothing in the pipeline's current artifacts forces it to be answered before
   everything downstream proceeds as if it doesn't matter.

**Missing questions that appear repeatedly, across personas and categories:** every one of the four
areas above shares one shape — **Canopy produced a complete-looking, internally consistent answer
with no visible seam indicating a decision was made**, rather than surfacing the question and
leaving it open. `open_questions: []` and `gaps: []` (Stage 0's own completeness check) are both
technically accurate against this pipeline's own narrower definition of "gap" — and both are
misleading against what five independent Product Owner readings actually needed clarified. This is
the same shape of finding the Human-Insight Inventory already reported from the review-log
angle (a low-reproducibility recommendation received no differentiated scrutiny) — this experiment
reaches a compatible conclusion from an entirely different method: not by asking how something was
reviewed, but by asking what a genuinely engaged reviewer would need to ask.

**Whether Canopy is succeeding at "generate the boilerplate, leave humans to contribute insight,"
as stated**: partially, and unevenly. The mechanical layer (fields, validation, happy/failure
scenarios) is generated well enough that no persona found anything to correct — real boilerplate,
handled well. But the four gaps above did not present themselves as things *for* a human to weigh
in on — they presented as already-resolved, requiring a persona to actively doubt a confident answer
rather than simply notice an open question. The original framing implies Canopy should leave a
visible seam where insight belongs; this experiment finds the opposite failure mode more often —
insight is *needed* but the artifact set gives no visible signal of *where*, which is a harder
problem than an explicitly flagged gap would be.
