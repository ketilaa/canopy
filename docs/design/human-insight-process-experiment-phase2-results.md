# Human Insight Process Experiment — Phase 2 Results

Status: results of a real run. Executes Phase 2 of `docs/design/human-insight-process-experiment-
design.md` for real, against a real local LLM, using the standalone example `canopy-llm/examples/
return_request_persona_experiment.rs`. Strictly read-only; nothing was saved to the dogfooding
project.

Date: 2026-07-17

**Scope of this document, stated per explicit instruction**: this phase answers *what effect does
human meaning have* — persona-supplied fact → Canopy processing → artifact change. It does not
answer *how humans interact with real decision gates* — that is Phase 3's separate question, run
later against the two personas selected here. No gate/review-behavior language appears below.

---

# Shared Setup (Existing Canopy Inputs — Identical Across All Five)

**Story** (`order-001`, generated once via a real `intent` call and accepted as-is): "As a customer,
I want request a return for a previously purchased product, so that can exchange or refund the
product if it is no longer needed or defective."

**Domain registry** (populated by that same `intent` call): entity `ReturnRequest`, event
`ReturnRequestCreated`. Notably — and worth flagging immediately — **neither `Product` nor `Order`
was extracted**, despite both being named in the story's own text, for the same reason established
in `docs/design/role-semantics-investigation.md` and `domain-boundary-hypothesis-assessment.md`:
domain extraction deliberately excludes concepts named only in a purpose/benefit clause. This is the
same forward-reference gap `manufacturer-001` showed, recurring on an independently-chosen story —
direct evidence it is a pattern, not a one-off.

**Services**: empty, matching the real pre-spec state this project's other experiments have already
established as the correct frozen baseline.

**Mechanism, stated once rather than per persona**: each persona's fact was supplied as a single
pre-existing item in `existing_adrs`, present before `generate_story_spec` ran — the same channel
and technique the Role Meaning Value Experiment already validated, not a new one. No interactive
review step was involved in producing any of the five results below.

---

# Per-Persona Results

## Risk-Averse / Loss-Prevention

**Persona-supplied fact (verbatim)**:
> Decision: "A return request must include the original order/purchase confirmation number,
> verified against our records, before it can be accepted"
> Reason: "Unverified requests are rejected outright to prevent fraudulent return claims — loss
> prevention takes priority over convenience."

**Generated outputs**: `entity_schema: <none>` — this run took the fallback scenario path, not the
main schema-driven one. `resolved_policies` (2): `uniqueness` — "must be unique for a specific
combination of order ID and customer ID," citing the supplied fact by name as evidence;
`consistency` — "depends on verifying the original order/purchase confirmation number against our
records," also citing the supplied fact by name. Four scenarios, all built around a valid/invalid/
missing/foreign order-number narrative.

**Traceable downstream effects**: **directly traceable.** Both resolved policies cite the supplied
fact verbatim as their `evidence` field — the clearest citation-backed link in this entire run. All
four scenarios revolve around the order-confirmation-number verification concept the fact
introduced, including a scenario with no equivalent in any other persona's output ("a return for a
product not associated with their account").

**Not traceable / likely ordinary variance**: the absence of an `entity_schema` at all. Nothing in
the supplied fact discourages schema extraction — this looks like ordinary run-to-run variance in
whether `entity_schema_prompt` succeeds, the same kind of variance this project's reproducibility
work has already documented elsewhere, not an effect of this persona's content.

## Customer-Experience

**Persona-supplied fact (verbatim)**:
> Decision: "A return request is accepted based on the customer's own account order history, with
> no additional proof required from the customer"
> Reason: "Minimizing friction for a returning customer is more valuable than strict verification
> for a typical, low-value return."

**Generated outputs**: `entity_schema.entity: ReturnRequest` — mandatory `orderId`, `customerId`,
`returnReason`; optional `returnItems`. `resolved_policies` (3): `uniqueness` — unique per
`orderId`+`customerId`; `authorization` — "requires the customer to be authenticated"; `consistency`
— "depends on the state of the Order entity." All three cite **the user story**, not the supplied
fact, as evidence. 14 scenarios, including one for each resolved policy.

**Traceable downstream effects**: **plausible, not certain.** The schema's inclusion of
`customerId` as a named, mandatory field, and the uniqueness rule scoped to `orderId`+`customerId`,
align with the fact's own framing ("the customer's own account order history") — a real, if
circumstantial, connection. The `consistency` finding naming an "Order entity" that doesn't
structurally exist is the same forward-reference pattern noted above, surfacing here inside a
policy resolution's own text rather than as an open question.

**Not traceable / likely ordinary variance**: all three `resolved_policies` cite the user story
verbatim as evidence, never the supplied fact — meaning none of them can be honestly called a
direct, citation-backed effect of this persona's distinguishing content, unlike the risk-averse
result above. `authorization` resolving to a generic "requires the customer to be authenticated" is
exactly what a plain reading of the shared story alone would produce, fact or no fact.

## Operational / Efficiency

**Persona-supplied fact (verbatim)**:
> Decision: "Return eligibility is determined automatically by matching the order date against a
> fixed return window, with no manual verification step"
> Reason: "Verification should never require a human review step — the lowest-overhead, fully
> automatic resolution is preferred."

**Generated outputs**: **the call failed.** `generate_story_spec` returned a YAML parse error —
`scenarios[0].constraints[0]: invalid type: sequence, expected a string` — the model emitted
`constraints: [[]]` (a nested empty list) instead of a flat list. This is a real, live model output
defect, not a bug in this experiment's harness — the same class of live YAML-shape fragility this
project has documented before. Entity schema and policy resolution succeed internally before this
point (confirmed by where the error occurs, in the second, scenario-generation call) but are never
recoverable from outside `generate_story_spec`, since the function returns the whole operation as
one `Result` with no partial-success path.

**Traceable downstream effects**: none can be assessed — there is no valid `IntentSpec` to compare.
The raw (unparsed) scenario text is still worth reading, though not usable for comparison: it
describes a fixed "30-day" return window, an explicit used/not-used product distinction, and
automatic verification — content that clearly reflects the supplied fact's own "fixed window,
no manual step" framing, even though the run never completed successfully.

**Excluded from every comparison below** — a failed run cannot be carried into implementation or
compared to a resolved one without fabricating data that was never actually produced.

## Compliance / Finance-Minded

**Persona-supplied fact (verbatim)**:
> Decision: "Per consumer protection requirements, a return request must be honored within a
> legally mandated minimum window regardless of the item's condition, using the original payment
> method for any refund"
> Reason: "Verification exists only to confirm the purchase occurred, not to create a barrier to a
> legally guaranteed right — driven by external obligation, not internal preference."

**Generated outputs**: `entity_schema.entity: ReturnRequest` — mandatory `orderId`, `productId`,
`reason`; optional `quantity`, `comments`. `resolved_policies` (4): `uniqueness` — unique per
`orderId`+`productId`; `defaults` — quantity defaults to 1; `authorization` — "requires the customer
to be authenticated"; `idempotency` — duplicate submission rejected. All four cite `story:
order-001`, not the supplied fact, as evidence. 14 scenarios, one per resolved policy plus field
validation.

**Traceable downstream effects**: **weak.** The uniqueness rule is scoped to `orderId`+`productId`
rather than `orderId`+`customerId` (customer-experience's scoping) — a materially different, real
structural divergence — but nothing in the supplied fact (about legal windows and payment method)
obviously motivates a `productId`-centric scope over a `customerId`-centric one. This divergence is
real and worth reporting, but its *cause* cannot honestly be pinned on the supplied fact's specific
content.

**Not traceable / likely ordinary variance**: all four resolved policies cite the generic story, the
same pattern as customer-experience — none of them reference the persona's own legal/payment-method
framing at all. Notably, the persona's most distinctive stated concern (refund via original payment
method) appears in **no artifact at all** — not the schema, not any policy, not any scenario. This
persona's content had the least visible effect on its own output of any of the four usable runs.

## Growth/Retention-Minded

**Persona-supplied fact (verbatim)**:
> Decision: "A return request should trigger an offer to exchange for a replacement or store credit
> before a refund is processed, and the stated reason for the return must always be captured"
> Reason: "Verification should stay lightweight so it doesn't discourage a returning customer from
> remaining engaged — the relationship matters more than the single transaction."

**Generated outputs**: `entity_schema: <none>` — fallback path, same as risk-averse.
`resolved_policies` (0) — every one of the six checklist areas landed in `open_questions` instead,
including `authorization`, completely unresolved. Five scenarios: a return request that triggers an
exchange-or-store-credit offer, a scenario for selecting exchange, a scenario for selecting refund,
a scenario for providing the return reason, and a cancellation scenario.

**Traceable downstream effects**: **directly traceable, in scenarios specifically.** Every one of
the five scenarios reflects the supplied fact's own distinctive framing almost verbatim — the
exchange/store-credit/refund choice structure and the mandatory reason-capture step both appear
nowhere in any other persona's output. This is the cleanest content-level match between a supplied
fact and its resulting artifact in the whole run, even though — unlike risk-averse — it shows up in
*scenarios*, not in any cited policy evidence, since no policy was resolved at all this run.

**Not traceable / likely ordinary variance**: the complete absence of any resolved policy. Nothing
about a retention-minded framing obviously implies "every policy question stays open" — this reads
as the same kind of entity-schema/policy-resolution variance already seen in risk-averse's run, not
a consequence of this persona's specific content.

---

# Cross-Persona Synthesis

**What was directly, citation-backed traceable to a supplied fact**: risk-averse's `uniqueness` and
`consistency` resolutions, both citing the fact verbatim. **What was traceable through content
match, without a citation**: customer-experience's `customerId`-centric schema (plausible, not
certain); growth-retention's scenario content (strong, unambiguous match, but landing in scenarios
rather than policy). **What showed real, structural divergence with no traceable cause**:
compliance's `productId`-centric uniqueness scope, and the very fact that only two of four usable
runs produced an entity schema at all. **What showed almost no visible effect of its own distinct
content**: compliance's stated legal/payment-method concern, absent from every artifact it produced.

This is a more mixed, more honest result than the Role Meaning Value Experiment's cleaner finding.
That experiment used a narrowly-scoped, single-question fact (a role's identity, one thing, titled
accordingly) and got consistent, citable results in most conditions. This experiment used broader,
narrative policy statements covering several concerns at once under one shared ADR title, and got
inconsistent citation behavior — direct evidence that **how narrowly a supplied fact is scoped
appears to affect how reliably Canopy's own citation mechanism connects it to a resolved policy**,
a finding this specific comparison was not designed to test but surfaced anyway, worth carrying into
any future design of a real domain-exploration mechanism.

---

# The Shape of Supplied Meaning, Checked Against the Data

Restating the specificity finding above more precisely, since it may be the most consequential
single result of this phase — checked directly against all four usable runs, not asserted from
impression:

| Persona | Character of the supplied fact | Traceability result |
|---|---|---|
| risk-averse | **Operational** — names a specific artifact ("the original order/purchase confirmation number") and a specific check ("verified against our records") | Strong — both resolved policies cite the fact verbatim |
| growth-retention | **Operational** — names specific actions ("trigger an offer to exchange... or store credit," "the stated reason... must always be captured") | Strong — every scenario mirrors these specific actions directly |
| operational (raw, pre-failure content) | **Operational** — names a specific mechanism ("matching the order date against a fixed return window") | Strong in raw content, though the run itself failed for an unrelated formatting reason |
| customer-experience | **Mixed** — names one specific check ("the customer's own account order history") inside reasoning that is otherwise principle-level ("minimizing friction... more valuable than strict verification") | Weak/plausible only — schema hints align, but every resolved policy cites the generic story, not the fact |
| compliance | **Principle-level** — "legally mandated," "driven by external obligation, not internal preference," with no single concrete, quotable mechanism | Weakest of all four — no artifact reflects this persona's distinguishing content at all |

Three of four usable runs confirm the pattern cleanly at one pole or the other; the fourth
(customer-experience) sits in between rather than confirming either pole outright — worth stating
honestly rather than rounding it into either bucket.

**This is not an isolated new observation — it is a specific, testable instance of a mechanism this
project already holds at high confidence.** `unresolved-decisions-become-explicit-decision-points`
established that the citation-requirement fix only works because it forces a "resolved" answer to
point at something specific and checkable, rather than accepting a plausible-sounding claim on its
own. The same mechanism, read from the other direction, explains this phase's finding directly:
`generate_story_spec`'s own citation behavior can only point at a supplied fact when that fact
*contains* something specific enough to quote. "Verified against our records" gives the model a
concrete phrase to lift into an `evidence` field; "driven by external obligation, not internal
preference" gives it nothing equivalently specific to point at, so it falls back to citing the
generic story instead. The principle was validated for the model's *own* answers; this phase is the
first evidence it may extend to *human-supplied* facts as well — the same underlying requirement
(specificity, not just presence) on both sides of the exchange.

**Evidence-strength caveat, stated plainly given how much weight this finding could bear**: this is
one experiment, four usable data points, one run per persona — the same N=1 limitation every
experiment in this chain has carried. It is a strong, honest, well-supported *pattern* in this run,
not yet a validated principle by this project's own evidence bar (which has generally wanted
independent confirmation across more than one investigation before promoting a finding to
`docs/principles/`). Worth designing a dedicated follow-up to test directly — deliberately varying
only *how operationally specific* a supplied fact is, holding its underlying content constant — but
not undertaken here; this document reports what this run found, not a new experiment.

---

# Selecting the Pair for Phase 3

**Operational is excluded outright** — no valid spec exists to carry forward.

**Risk-averse and growth-retention, despite showing some of the clearest traceable content in this
whole run, are not viable candidates for implementation comparison**: both have `entity_schema:
<none>`. Stage 1's mechanical behaviors (validation, construction) are derived directly from
`entity_schema` — carrying a schema-less run into `behaviors`/`implement` would produce a
structurally degenerate comparison unrelated to persona content, undermining exactly the question
Phase 3 exists to answer.

**Customer-experience and compliance are selected** — the two personas whose runs actually produced
a complete, schema-driven specification, and which diverge on a genuine structural question neither
citation could explain away: whether a return is scoped by **customer identity**
(`orderId`+`customerId`, customer-experience) or by **product identity**
(`orderId`+`productId`, compliance) — plus a materially different field set (`returnItems` vs.
`quantity`+`comments`). This is not necessarily the theoretically largest divergence measured in
this run (growth-retention's scenario content arguably diverges more sharply in wording) — it is the
largest divergence **among the runs that can actually be carried through real implementation and
compared as running software**, which is what this selection is for.

---

# What This Phase Does and Doesn't Justify Concluding

**Justified**: persona-supplied facts can and do produce traceable differences in Canopy's real
output — confirmed again, independently of the Value Experiment, though less cleanly across the
board this time. A narrowly-scoped fact (risk-averse's) produced a directly citable effect; a
broadly-scoped one (compliance's) produced almost none. **Not justified**: that every persona's
distinct business instinct reliably shapes output — two of five runs show no clean causal link at
all, and one run failed outright. **Not justified**: any claim about reproducibility — N=1 per
persona, same caveat as every prior experiment in this chain.
