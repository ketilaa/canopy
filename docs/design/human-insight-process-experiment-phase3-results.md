# Human Insight Process Experiment — Phase 3 Results

Status: results of a real, live-driven run. Stops at Stage 4 (contract generation) — per explicit
decision, this phase does not proceed to `scaffold`/`implement`. Answers Phase 3's question
directly: does the persona-driven spec divergence Phase 2 measured survive into contracts, once the
comparison is carried through a real, interactive `canopy behaviors` session for each of the two
selected personas?

Date: 2026-07-17

**Primary finding, stated up front**: **the original persona-driven divergence did not survive
regeneration.** Neither persona's distinguishing supplied fact left a traceable mark in the
regenerated specification. The structural difference that originally motivated selecting this pair
(customer-identity-scoped vs. product-identity-scoped uniqueness) did not persist. What divergence
does remain at the contract level is better explained by pipeline behavior (Finding #2, below) than
by either persona's supplied meaning. This is a materially different, weaker result than Phase 2's
own value-comparison finding, and the report is structured to make that contrast explicit rather
than to force a positive result.

---

# 1. Phase 3 Objective

Per `docs/design/human-insight-process-experiment-design.md`: extend one branch of the five-persona
comparison to real implementation, selecting the most divergent pair *after* the spec stage, to test
whether spec-level differences survive into runnable software — tests, API behavior, validation
behavior, authorization behavior. The two selected personas, per Phase 2's own analysis
(`docs/design/human-insight-process-experiment-phase2-results.md`), were **customer_experience**
and **compliance** — the only two of five persona runs that produced a complete, schema-driven
specification, diverging on customer-identity-scoped vs. product-identity-scoped uniqueness.

---

# 2. Experimental Corrections and Controls

Kept explicitly separate from both the Findings (§3) and the persona comparison itself — these are
setup/harness decisions, not experimental signal.

- **Shared architecture.** A single real `canopy spec order-001` run established one shared
  architectural foundation (service ownership, UI, tech stack, database, event broker) via the
  actual `spec` CLI command, copied identically into both persona branches before any
  persona-specific content was introduced — isolating the comparison to spec-level content, not
  architecture-recommendation noise (already documented elsewhere as an independent variability
  source).
- **ADR-persistence correction.** Both personas' facts were originally only supplied in-memory to
  the standalone value-comparison example (Phase 2's own method) — never persisted as real ADRs.
  Discovered before branch comparison began, for **both** branches, and corrected identically: each
  persona's fact was written as a real `adr-008-return-eligibility-and-verification-policy.yaml`,
  and `canopy spec order-001` was re-run for real (via the actual CLI, not the standalone example)
  against each branch, regenerating `spec.yaml`/`openapi.yaml` from a corrected, equivalent setup.
  This correction is not persona-driven and is not evidence for or against Domain Exploration — it
  fixes a setup gap that applied identically to both branches.
- **Generic open-question harness resolution.** Three business-policy checklist areas
  (uniqueness/retention/idempotency, or whichever subset the LLM itself didn't resolve) reliably
  fell through to `open_questions` in a way neither persona's fact ever addressed. Resolved with
  identical, clearly-labeled neutral text ("HARNESS ASSUMPTION — not persona-attributable, applied
  identically to both Phase 3 branches") in both branches, to let both reach implementation-eligible
  state on equal footing.
- **Stage 0 bypass** (`SpecificationCompleteness::has_blocking_gaps`, `canopy-core/src/lib.rs`) —
  temporary, disclosed, `&& false`. Introduced only after Finding #1 (below) was confirmed
  reproducible across 4 independent runs against real, verified-correct spec content. Applied
  identically to both branches. Reverted before any commit (see §9).
- **Stage 2 bypass** (`DecisionLog::has_pending_decisions`, same file) — same pattern, introduced
  after Finding #3 (below) was confirmed as a downstream consequence of Finding #2, not a real
  business ambiguity. Applied identically to both branches. Reverted before any commit.

---

# 3. Findings #1–#3 (Pipeline-Reliability Bucket — Not Persona-Attributable)

Tracked entirely separately from the persona comparison throughout, per explicit instruction.

### Finding #1 — Stage 0 False-Positive Scenario-Gap Detection

**Status: reproducible pipeline finding**, confirmed across 4 independent runs against
`customer_experience`'s regenerated spec (identical input each time) and recurring again on
`compliance`'s branch.

- **Gaps reported**: a rotating-but-overlapping subset of "missing_scenario" flags — most
  frequently `customerId`/`orderId`/`productId` `min_length=1` and `reason`/`status` `max_length`
  constraints — plus, in two runs, an "ambiguous_outcome" flag against every single scenario
  including ones with unambiguous, clearly-observable `then` clauses.
- **Scenarios actually present**: verified directly against real `spec.yaml` content each time —
  every flagged constraint had a real, correctly-worded scenario testing it (e.g. `order-001-04`
  genuinely tests `customerId` `min_length=1`).
- **Why this is a false positive, not a real gap**: the checklist's own per-item prompt already
  names the correct candidate scenario ("candidate: scenario order-001-04 mentions 'customerId' and
  '1'"). The mechanical candidate-identification logic is working correctly in most instances
  (though in one run, candidate identification itself pointed at the wrong scenario — `order-001-03`
  instead of `order-001-04` — a distinct, secondary mechanical inconsistency). What fails is the
  model's own per-item yes/no judgment on a correctly-identified candidate.
- **Does the exact same set of scenario IDs get repeatedly flagged?** Largely yes — a 4/5-item
  overlap between the first two runs, with drift in later runs (one item dropping, new
  "ambiguous_outcome" flags appearing) — a rotating-but-substantially-overlapping pattern, not
  either a fixed identical set or a fully random one.
- **Root cause classification**: not "the scenario truly isn't recognized" (the candidate is
  correctly named in the prompt in most instances) and not "the checklist generation is
  structurally broken" (it correctly identifies candidates most of the time). Closest fit:
  **the model's own per-item verification judgment is unreliable even when given an unambiguous,
  correctly-identified candidate to check against** — a compliance gap on a well-posed question,
  not a missing-fact or missing-instruction problem.
- **Why treated as pipeline-reliability, not experimental signal**: reproducible across multiple
  runs, on both branches, independent of which persona's fact was supplied — nothing about it
  varies with persona content.

### Finding #2 — Scenario-Derived Behavior Extraction Ignores Available Scenarios

**Status: reproducible pipeline finding on `customer_experience` (2 of 2 independent runs);
did NOT recur on `compliance`'s single Stage 1 run** — stated precisely rather than implying
uniform recurrence across both branches.

- On both `customer_experience` runs, Stage 1's scenario-derived behavior-extraction call returned
  `blocked: [...]` for all 14 scenarios, each citing a variant of "No scenario listing provided" /
  "No scenario provided for X" as the reason — despite the actual LLM prompt, verified directly,
  genuinely containing the full scenario listing both times.
- Result: only the 12 mechanical (validation/construction) behaviors survived each time; zero
  inferred integration behaviors (persistence, orchestration, http-response) were ever produced for
  `customer_experience`.
- On `compliance`'s single real Stage 1 run, this did **not** recur — the call produced 54
  behaviors total, including 42 genuinely inferred integration behaviors. A **separate, distinct**
  quality issue appeared instead: every inferred behavior was uniformly SUCCESS-shaped ("persisted,"
  "event published," "HTTP 201") regardless of whether the source scenario was actually a success or
  a failure case — scenarios `order-001-03` through `order-001-12` are validation-rejection/
  duplicate/unauthenticated failure scenarios, yet each produced identical persistence/event/201
  behaviors, which should have been *prevented*, not asserted, for a failure case. This is reported
  as its own distinct observation, not folded into Finding #2, since the failure mode is different
  in kind (wrong content vs. no content).
- **Causal chain confirmed directly**: on `customer_experience`, Stage 2 subsequently reported one
  fabricated Decision Point per run, both times derived from the "no scenario" blocked reason. On
  `compliance`, where Stage 1 succeeded (0 blocked), Stage 2 correctly reported "No decision points
  — nothing blocked on an unresolved business question." This is direct, mechanical confirmation
  that Finding #3 is downstream of Finding #2, not an independent defect.

### Finding #3 — Fabricated Decision Point Generation (Downstream Consequence of Finding #2)

**Status: reproducible downstream consequence, confirmed twice on `customer_experience`; did not
occur on `compliance` because its precondition (Finding #2) did not occur.**

- Instance 1: `order-001-dec-001`, "What scenario listing should be provided for order processing?"
  — options about order-listing detail levels, none related to return requests at all.
- Instance 2 (after a fresh session restart): `order-001-dec-001`, "What is the scenario for orders
  from order-001-01 to order-001-14?" — options about single-product vs. multi-product vs. service
  orders, again unrelated to the actual story.
- Both instances: off-topic relative to the story's actual domain (return requests, not order
  composition/listing), generated from the corrupted "no scenario" reason text, and classified
  `business` by Stage 2's own categorization call despite having no real content.
- Both deferred, never resolved with fabricated content — the only honest response available.

**Causal chain, confirmed mechanically, not just inferred:**

```
Scenario listing present in the real prompt (verified directly)
        ↓
Stage 1 behavior extraction claims no listing was provided
        ↓
Zero inferred integration behaviors generated
        ↓
Stage 2 receives 14 "blocked" items with no real content
        ↓
Stage 2 fabricates an off-topic Decision Point from the corrupted reason text
        ↓
Pipeline blocks on a non-existent business question
```

**Pipeline-state summary**, using the canonical Stage numbering from
`docs/design/behavior-first-planning.md`:

| Stage | customer_experience | compliance |
|---|---|---|
| Stage 0 (completeness) | Finding #1 (bypassed) | Finding #1 (bypassed) |
| Stage 1 (behavior extraction) | Finding #2 (both runs) | Healthy on content presence; separate SUCCESS-uniformity defect observed |
| Stage 2 (Decision Points) | Finding #3 (both runs, bypassed) | Healthy — correctly reported zero decision points |
| Stage 3 (clustering) | Healthy — 6 clusters, 0 findings | Healthy — 7 clusters + 1 integration group, 1 real (non-fabricated) finding |
| Stage 4 (contracts) | 6 contracts, all mechanical, 0 integration | 8 contracts, including 1 real integration contract |

---

# 4. Regeneration Reassessment

Direct comparison of the two branches' regenerated `spec.yaml`, once both had gone through the
identical ADR-persistence correction:

| | customer_experience | compliance |
|---|---|---|
| Mandatory fields | `customerId`, `orderId`, `productId` | `orderId`, `customerId`, `returnReason` |
| Optional fields | `reason`, `status` | `returnNotes` |
| `productId` present? | Yes | No |
| `status` field present? | Yes (LLM-resolved default: `pending`) | No |
| `uniqueness` resolved by the LLM itself? | No (fell through to harness-neutral) | Yes — "per order and customer" |
| `consistency` references | order **and** product entities | order entity only |
| Either persona's own fact cited as `evidence` anywhere? | **No** | **No** |

The schema that emerged from `compliance`'s regeneration (`orderId`/`customerId`/`returnReason`,
no `productId`) is materially closer to `customer_experience`'s **original** Phase 2 materialization
shape than to `compliance`'s own original materialization (which had been `productId`-centric with
a `quantity` field). The axis that motivated selecting this pair — customer-identity vs.
product-identity scoping — **inverted** under regeneration rather than persisting: this
regeneration, `customer_experience` (not `compliance`) is the one with `productId` present.

---

# 5. Why the Original Divergence Collapsed

Two explanations were named as live possibilities before this reassessment; the evidence bears on
both without cleanly deciding between them, and both are reported rather than only the more
convenient one:

1. **The original Phase 2 divergence was itself substantially model-sampling variance**, of the
   same kind the Pre-Behavior Planning Reproducibility Sweep already documented for architecture
   recommendations — not a stable, reproducible consequence of either persona's fact. Regenerating
   under nominally identical conditions (same fact, same architecture, same story) produced a
   different schema shape each time, which is exactly what pure sampling variance predicts.
2. **The supplied facts were never strongly connected to the specific field-level outcome in the
   first place.** Neither fact mentions products, quantities, or status fields at all —
   `customer_experience`'s fact is about verification friction ("account history, no additional
   proof"); `compliance`'s is about refund timing and payment method. Whatever produced the
   `productId`/`quantity`/`status` differences in either run, it wasn't traceable engagement with
   the fact's own content, in the original materialization or in this regeneration.

Both explanations point the same direction: **the field-level structural divergence Phase 2 found
was not a reliable signal of persona-supplied meaning**, whether because it was noise all along or
because the facts never reached that level of specificity to begin with.

---

# 6. What Survived

- **A real structural difference between the two branches still exists** at every stage through
  contract generation — different mandatory/optional field sets, different resolved-policy content,
  different contract counts (6 vs. 8) and shapes (0 vs. 1 integration contracts).
- **The general finding that persona-supplied facts *can* leave a citation-traceable mark** —
  established independently by the Role Meaning Value Experiment (`docs/design/role-meaning-value-
  experiment-results.md`), which found a narrowly-scoped, single-question role fact reliably moved
  `authorization` from unresolved to a cited resolution. That result is unaffected by this phase's
  findings — it used a different, narrower fact shape and a controlled four-condition comparison,
  not the broader multi-concern facts used here.
- **The Phase 2 specificity pattern gains a second, independent data point.** `compliance`'s fact —
  principle-level, no single quotable mechanism — again failed to appear in any resolved policy's
  evidence field, in a completely separate regeneration run, on top of its original Phase 2
  non-result. Recorded as supporting evidence for the pattern, not proof of it — still one story,
  still a small sample, and not elevated to a principle here.

# 7. What Did Not Survive

- **The specific customer-identity-vs-product-identity divergence** that justified selecting this
  pair — inverted, not merely weakened, under regeneration.
- **Any citation-level connection between either persona's own distinguishing fact and its
  branch's resulting schema, policy resolution, or scenario content.** Every resolved policy in
  both regenerated specs cites either the generic user story or (in `compliance`'s case, incorrectly)
  the domain-event ADR — never the actual supplied policy fact.
- **The premise that this pair's original divergence was itself evidence of a persona effect.**
  It is now better explained as spec-generation variance the reproducibility sweep already
  characterized, occurring on top of two facts that left little independent trace either time.

---

# 8. Implications for Domain Exploration

This phase's result is a **necessary corrective, not a refutation of the Domain Exploration
thread as a whole.** Two results from this same investigation chain now need to be held together,
not collapsed into one verdict:

- The **Role Meaning Value Experiment** found real, controlled, four-condition evidence that a
  narrowly-scoped, single-question fact can move a specific policy resolution and its downstream
  scenario content, traceably and repeatedly.
- **This phase** found that a broader, multi-concern, less operationally specific fact, exercised
  through a full real session rather than a single controlled call, did not leave a traceable mark
  at all — and that the structural divergence it was originally credited with producing does not
  survive regeneration.

Read together, the evidence now points at **fact specificity and scope, not persona identity
itself**, as the variable that determines whether supplied meaning survives into the pipeline. This
sharpens rather than weakens the case for a real Domain Exploration capability: it suggests such a
capability's value depends heavily on eliciting narrow, specific facts (the shape the Role Meaning
Value Experiment tested) rather than broad instinct-level guidance (the shape this phase tested) —
a concrete, evidence-backed design constraint for any future mechanism, not a reason to abandon the
thread. No mechanism is proposed here, consistent with this investigation's own charter throughout.

---

# 9. Reversal of Temporary Bypasses

Both temporary, disclosed bypasses introduced in `canopy-core/src/lib.rs`
(`SpecificationCompleteness::has_blocking_gaps`, `DecisionLog::has_pending_decisions`) are reverted
immediately following this report, restoring both functions to their original, unmodified behavior.
Neither was ever committed.
