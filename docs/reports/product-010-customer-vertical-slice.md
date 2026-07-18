# Story Report — `product-010` (Customer browses the catalog)

Iteration 3 of the decided backlog execution plan (`docs/design/roadmap-reassessment.md`'s
2026-07-18 update): carry `Customer` — the first candidate the entity-with-no-story check found —
through the real pipeline as far as behaviors/contract generation, to test whether a Backlog
Evolution finding actually converts into a safely implementable story.

---

## Session 1 — Intent through Contract Generation (2026-07-18/19)

**Setup:** the same real dogfooding project the entity-with-no-story check was run against.
Statement given: "A customer can browse the published catalog to see available products" — the
exact capability the prior turn's dependency analysis identified as the cheapest, highest-unlock,
zero-unsatisfied-prerequisite first slice for `Customer`.

**Intent.** Derived as `product-010` (`as_a: customer`, `want: browse the published catalog`),
grouped under the existing `product-*` prefix rather than a new `customer-*` one — a defensible
model choice, not overridden. Accepted as-is; one real vocabulary-discrepancy prompt fired
(`available`, correctly dismissed as an adjective).

**Spec.** Six ADRs proposed and accepted without modification: a new `catalog-browsing-service`
(Spring Boot) owning catalog-browsing responsibility, a new `catalog-browsing-portal` (React +
Vitest/RTL) as the frontend, its own PostgreSQL database (a defensible CQRS-shaped read model,
populated via `ProductVariantPublished`), and reuse of the existing Redpanda event broker. Five BDD
scenarios generated, including an explicit "customer is not authenticated" failure case. Stage 0
completeness: `gaps: []` on the first pass.

**Stage 1 (behavior extraction) — real pipeline defect reproduced, then retried successfully.**
The first attempt returned `behaviors: []` for all 5 scenarios, and the harness's own audit
correctly flagged all 5 as "likely lost during generation or parsing, not a legitimate empty
outcome" rather than accepting it as a clean result. Checked the actual prompt in
`llm-debug.log` before doing anything else: the prompt was correct and complete (all 5 scenarios
properly listed, the per-scenario checklist properly enumerated) — the model simply returned a
12-token empty response. This is a live reproduction of the same defect class already documented
in the Human Insight Process Experiment (Finding #2: Stage 1 ignoring scenarios despite
verified-correct prompts) — not a new investigation, and not something this session tried to fix.
Declined to proceed past the broken result (`n` at the "looks correct — proceed?" gate, which
stops cleanly without corrupting anything) and re-ran `canopy behaviors product-010` directly. The
retry succeeded cleanly: 10 behaviors, 2 per scenario (an `http-request` and a matching
`http-response`), zero audit findings.

**Stage 2 (decisions):** none — `decisions.yaml` is empty, nothing blocked on an unresolved
business question.

**Stage 3 (clustering):** one integration cluster (`CatalogBrowsing`, all 10 behaviors), zero
review findings.

**Stage 4 (contracts):** one contract, `CatalogBrowsingWorkflow` (`product-010-contract-001`,
`scope: integration`, `derivation: mechanical`), owning all 10 behaviors as its `required_tests`.
Zero contract-audit findings, zero dependency-review findings, `dependencies: []`. Accepted.

**Completion criterion met**: the story is accepted, specified, and carried through contract
generation, exactly as Iteration 3's own stated bar required.

---

## Finding — a live, concrete instance of the Story-Readiness / Backlog-Evolution gap the roadmap named

This story passed every internal check the pipeline has: Stage 0 found zero gaps (twice — before
and after the Stage 1 retry), Stage 2 found zero unresolved decisions, Stage 3 and Stage 4 found
zero audit findings. By every existing Story Readiness signal, `product-010` is complete.

And yet: `spec.yaml`'s own `resolved_policies` entry for `authorization` reads *"The story does not
explicitly mention any authorization requirements for browsing a catalog"* — while, in the same
file, `out_of_scope` explicitly excludes *"Customer authentication and authorization"* — while the
accepted scenarios include `product-010-05` ("Customer receives an error message if they are not
authenticated"), and the accepted contract's own `required_tests` include a full
`GET /catalog without authorization` → `HTTP 401` behavior pair (`product-010-b009`/`b010`).

**No authentication capability exists anywhere in this project.** `Customer` had zero stories
before this session; there is no login, session, or identity story for any role. The pipeline
simultaneously (a) declared authorization explicitly out of scope, (b) generated and accepted a
401-response behavior that presupposes an authorization mechanism, and (c) reported zero gaps at
every stage designed to catch exactly this kind of thing.

This is not a new finding in the abstract — it's the same shape the Product-Owner Perspective
Experiment found for `manufacturer-001` ("authenticated as what, and authorized to do what? ...
Nothing decides this"), recurring here independently, on a different story, driven by the real
pipeline rather than a simulated persona. What makes it worth recording now, specifically: it's a
concrete, present-tense confirmation of the roadmap's own stated premise — *"a story can be
internally complete while the surrounding capability area remains incomplete"* — caught in the
first real vertical slice run under that premise, not asserted abstractly.

No mechanism proposed here, matching Iteration 3's explicit scope (execution, not new
investigation). Filed as evidence for whichever Story Readiness or Backlog Evolution work comes
next, not a new open thread.

---

## Summary

| Stage | Result |
|---|---|
| Intent | Accepted, `product-010`, one vocabulary-discrepancy prompt correctly dismissed |
| Spec | 6 ADRs accepted, 5 scenarios, Stage 0: 0 gaps |
| Behaviors | 1st attempt: 0/5 (known defect class, reproduced live); retry: 10/10 clean |
| Decisions | 0 |
| Clustering | 1 cluster, 0 findings |
| Contracts | 1 contract, 0 findings, accepted |

**Verdict**: yes — a Backlog Evolution finding (`Customer` has no story) converted into a real,
accepted, fully-specified capability that reached contract generation. Along the way, the session
also reproduced one known pipeline-reliability defect (resolved by retry, not investigated further)
and surfaced one genuine, disclosed Story-Readiness gap (an accepted 401 behavior with no
corresponding authentication capability anywhere in the project) — both recorded as evidence, per
this project's own reporting convention, not acted on beyond that.
