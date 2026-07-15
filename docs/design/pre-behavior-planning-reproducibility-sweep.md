# Pre-Behavior Planning Reproducibility Sweep — Design (scoped 2026-07-15, not yet run)

Status: design only. No sweep has been executed. This document defines what will be measured, how
variance will be classified, what counts as high/medium/low reproducibility, and what stop
conditions apply — all decided before any run, per the Roadmap Reassessment's own recommendation
(`docs/design/roadmap-reassessment.md`, §2.1/§5) and this project's standing "design → implement
small → verify → decide" methodology (Stages 1–6 of the contract-driven implementation
investigation, and Reproducibility Sweeps 1–4 for Stage 0 completeness/policy discovery,
`docs/reports/manufacturer-001.md`).

**Non-goals, stated up front, matching the explicit brief:** this is not a redesign exercise, not
a fix-the-pipeline exercise, and not a proposal exercise. It measures whether pre-behavior planning
is reproducible. It does not evaluate whether any given recommendation is *good*.

---

## 1. Experiment Design

### What gets called, and why this is the cleanest available experiment

The target is `identify_architectural_questions` (`canopy-llm/src/prompts/spec.rs:258`) — the one
LLM call that performs service discovery, service-ownership assignment, technology recommendation,
and infrastructure recommendation, all at once (established in
`docs/design/pre-behavior-planning-review.md`'s "Service Discovery"/"Technology Recommendation"
sections). Its signature:

```rust
pub fn identify_architectural_questions(
    client: &LlmClient,
    story: &UserStory,
    existing_adrs: &[Adr],
    services: &ServicesRegistry,
    domain: &DomainRegistry,
) -> Result<ProposedAdrs, LlmError>
```

**A naive "run `canopy spec` N times via the real CLI" does not work as a reproducibility
methodology**, and this is worth stating explicitly since it's the first design choice this
document has to reject: `canopy spec` mutates `services.yaml` and `decisions/adr-*.yaml` on every
run, and the prompt itself explicitly instructs the model to "skip if the specific service that
should own THIS story's domain is already in Known Services." A second real CLI run against the
same story would see the first run's own output as prior context and mostly skip re-proposing
anything — collapsing the experiment after run 1, not measuring reproducibility.

The correct pattern — the same one Stage 5/6 already established for exactly this reason — is to
call `identify_architectural_questions` directly, N times, with **frozen, never-mutated** input
loaded once from disk, discarding each run's output rather than persisting it. No modification to
`canopy-cli`'s `spec.rs`, `plan.rs`, or any production call site; a new standalone example, exactly
matching Stage 5's/Stage 6's own "reuses the actual mechanism, not a copy of it" discipline.

### Which frozen inputs to use — a real design decision, not a default

`manufacturer-001` is the only story with real data in the dogfooding project, so it's the natural
choice per "use a real story if possible." But its **current** `services.yaml` already has both
services fully decided (`manufacturer-service` = Spring Boot, `manufacturer-registration-portal`
= React) and its `decisions/` directory already has 8 ADRs — freezing *today's* state and calling
`identify_architectural_questions` against it would mostly reproduce the "already decided, skip"
branch, which is the least informative case for measuring discovery/recommendation variance.

The Contract Readiness Assessment already established (Q6 #2) that this dogfooding project's
`decisions/` directory contains **no** init-wizard ADRs at all — every one of the 8 existing ADRs
traces back to `manufacturer-001`'s own first `spec` run. That means the real state immediately
*before* that first run was: `services: ServicesRegistry { services: [] }`, `existing_adrs: []`.
This is not a fabricated or synthetic fixture — it is the project's own real starting condition,
recoverable exactly by constructing an empty `ServicesRegistry`/`Vec<Adr>` in the experiment
program rather than loading today's already-populated files for those two inputs specifically.

**Inputs, frozen, loaded/constructed once and reused for all N calls:**
- `story`: `manufacturer-001`'s real `UserStory` (`load_user_stories`, filtered by id) — unchanged
  from every prior stage in this investigation.
- `domain`: the real, current `domain_registry.yaml` (`load_domain_registry`) — this file is
  populated by `init`/`intent`, not by `spec`, so today's content correctly reflects what existed
  before `manufacturer-001`'s spec run, unlike `services`/`existing_adrs`.
- `existing_adrs`: an empty `Vec<Adr>` — reconstructing the real pre-spec state.
- `services`: an empty `ServicesRegistry { services: vec![] }` — same reasoning.

This maximizes how much of the interesting discovery/recommendation behavior actually fires,
while still using 100% real data for the two inputs (`story`, `domain`) that weren't themselves
produced by the step under test — reusing existing fixtures, per the brief, rather than inventing
a new one.

### Run count

**N = 5**, not this investigation's usual minimum of 3. Every prior stage used 3 runs to establish
*whether* variance exists at all (a pass/fail-shaped question). This sweep's question is different
— *how much* variance, at what rate — which needs enough data points that one outlier run doesn't
swing the classification. 5 is the smallest increase that meaningfully separates "1 of 5 diverged"
from "1 of 3 diverged" in the success criteria below (§4).

### What the (not-yet-written) example program would do

A new file, `canopy-llm/examples/pre_behavior_reproducibility_sweep.rs`, mirroring Stage 5/6's own
structure: load the frozen inputs once, call `identify_architectural_questions` 5 times in a loop
(discarding, never saving, each `ProposedAdrs`), print every run's full proposal list, and print a
final comparison table per the metrics below. No `mvn`/compile step is needed here (unlike Stages
3, 5, 6) — the artifact under test is prose/structured-YAML output, not code, so the sweep's own
comparison logic (§2) is what "verifies" this run, not an external compiler.

---

## 2. Metrics

For each of the 5 runs' `ProposedAdrs.proposals`, the following are extracted and compared. Each
metric is computed mechanically from the `ProposedAdr` struct's own fields (`canopy-core/src/
lib.rs:716-733`) — no LLM judgment is used to compute a metric, only to classify variance once
metrics are already extracted (§3).

| Output (per the brief's examples) | Field(s) read | How compared across the 5 runs |
|---|---|---|
| Number and shape of proposals raised | `proposals.len()`, and each proposal's `question`/`title` | Does the same *count* and same *set* of question topics appear in every run? |
| Discovered services | `proposal.service` (structural/UI proposals) | Is the same service *name* proposed in every run, or does the name, or the number of distinct services, vary? |
| Service ownership | `proposal.service` + `proposal.service_responsibilities` on the structural (service-ownership) proposal specifically | Does the same service own this story's domain in every run? Do the responsibilities listed differ? |
| Technology recommendation (backend) | `proposal.technology` where `component_type == Some("service")` | Same technology string (or a recognized equivalent, §3) in every run? |
| Frontend recommendation | `proposal.technology` where `component_type == Some("frontend")` | Same as above, for the UI proposal |
| Infrastructure recommendations (database, event broker) | `proposal.technology`/`decision` on proposals whose `title` matches a database/event-broker pattern | Same technology/decision across runs? |
| Generated ADR proposals overall | `title`, `decision`, `reason` for every proposal | Full-text comparison, feeding the wording/equivalent/material/divergence classification in §3 |

Category detection (which of "service ownership / UI / tech-stack-backend / tech-stack-frontend /
infrastructure-database / infrastructure-event-broker / domain-event" a given `ProposedAdr`
belongs to) is done mechanically by a fixed rule: `component_type` value first
(`"frontend"`/`"service"` distinguishes UI-tech from backend-tech), then a keyword match against
`title` for the rest (`"database"`, `"event broker"`, `"domain event"`, `"service ownership"`) —
the same "compute mechanically, don't ask the model" approach this project already applies
elsewhere (`compute-facts-mechanically`). This categorization is a fixed lookup, not a judgment
call, and is applied identically to every run's output before any comparison happens.

---

## 3. Classification Rules

Defined before running anything, per the explicit requirement. Applied per matched category
(comparing run *i*'s proposal in a given category against run *j*'s proposal in the same
category), in ascending order of severity:

1. **Wording only.** Every substantive field (`service`, `technology`, `component_type`,
   `service_responsibilities`) is byte-identical across the two runs; only `title`/`decision`/
   `reason` prose differs. Fully mechanical to detect — a direct field comparison, no judgment
   required.
2. **Equivalent recommendation.** A substantive field differs as a string but resolves to the same
   real-world choice (e.g. `"Postgres"` vs. `"PostgreSQL"`; `"manufacturer-service"` vs.
   `"manufacturer-svc"`). **This tier is not fully mechanical** — recognizing that two strings name
   the same real thing requires a bounded human (or a separately-justified canonicalization list)
   judgment call, stated honestly here rather than assumed away. Any classification that lands in
   this tier must be justified in the sweep's write-up with the specific reasoning for why the two
   strings were judged equivalent, the same way Stage 4's dependency review justified each
   addition/removal against a real behavior statement rather than asserting it.
3. **Materially different recommendation.** The same category of question receives a genuinely
   different real-world answer across runs — e.g. one run proposes Spring Boot for the backend,
   another proposes Node.js/Express, for the identically-worded service-ownership/tech-stack
   question. This is the specific shape of variance the original anecdotal observation
   (`docs/open-questions/pre-behavior-planning-review.md`) described.
4. **Architectural divergence.** The *set* or *structure* of what's being decided differs, not
   just an individual answer within a fixed structure — a different number of services discovered,
   a category of question raised in one run and never raised in another, or story ownership split
   differently across services. This is a strictly stronger form of disagreement than tier 3: tier
   3 assumes the same question was asked and answered differently; tier 4 means even *which
   questions get asked* differs.

---

## 4. Success Criteria

Explicit thresholds, computed from the classification counts across all `5 choose 2 = 10`
pairwise run comparisons per category (not a single hand-picked pair):

| Reproducibility | Criteria |
|---|---|
| **High** | Every pairwise comparison, in every category, classifies as tier 1 (wording only) or tier 2 (equivalent recommendation). The set of categories raised is identical across all 5 runs. Zero tier-3 or tier-4 classifications anywhere. |
| **Medium** | At least one tier-3 (materially different) classification exists, but confined to **at most 1 of the 5 runs** being the outlier in any given category (i.e. 4 of 5 runs agree, 1 disagrees) — and **zero** tier-4 (architectural divergence) classifications anywhere. |
| **Low** | Any tier-4 (architectural divergence) classification in any run, **or** tier-3 classifications appear across more than 1 of the 5 runs in any category (i.e. the disagreement isn't a single outlier). |

This mirrors the "X of Y runs" reporting convention every prior stage in this investigation has
used (e.g. Stage 5's "0/3 vs. 3/3," Stage 3's "2 of 3 runs") rather than a subjective read.

---

## 5. Stop Conditions

What the result would and would not justify concluding, decided before the result is known:

- **If all 5 runs classify as tier 1/2 only (High reproducibility):** justified — "for this one
  story, at this one frozen pre-spec snapshot, the architectural-questions mechanism is stable."
  **Not justified** — "pre-behavior planning is reproducible in general." Same single-story
  generalization caveat this investigation has applied to every contract-driven finding
  (`implementation-ownership-requires-full-file-scope-visibility`'s own confidence rating is
  exactly this shape of caveat, `medium`, for the identical reason).
- **If recommendations diverge substantially (Low, driven by tier-3 concentrated in tech-stack or
  service-ownership categories):** justified — "the anecdotal tech-stack-variance observation that
  opened this investigation is confirmed as real, not a one-off." **Not justified** — "the
  recommendation mechanism is broken" or "a specific fix is needed." This sweep measures agreement,
  not quality — a recommendation can be stable-but-wrong or variable-but-harmless, and nothing in
  this design tests correctness.
- **If architecture choices vary (Low, driven by tier-4 — e.g. a different number of services
  discovered across runs):** justified — "the discovery step itself, not just individual
  recommendation content, is unstable — a stronger finding than tech-stack wording variance
  alone." **Not justified** — any claim about *why* (model sampling vs. prompt-content ordering vs.
  something else). This design deliberately holds every input fixed and identical across all 5
  calls, which isolates model-sampling variance specifically; it does **not** test the
  order-dependent-prompt-content variability source `docs/design/pre-behavior-planning-review.md`
  separately identified (existing ADRs/services rendered in stored `Vec` order) — that would need
  a second, follow-up sweep that deliberately varies input *order* while holding content fixed, not
  a conclusion this design can draw.
- **If service ownership changes across runs:** a specific instance of tier-3 or tier-4 depending
  on severity (a different owning service = tier-4; the same service but different responsibility
  wording = tier-1/2). Justified — "service-ownership assignment for this one story is/isn't
  guaranteed stable." **Not justified** — any claim about multi-story or multi-entity projects,
  since only one story is exercised here, the same single-data-point caveat as everywhere else in
  this investigation.

---

## 6. Relationship to Future Work

Stated as effects on interpretation, not as fixes — per the explicit instruction not to propose
any yet.

- **Human-Insight Inventory** (`docs/design/roadmap-reassessment.md` §2.2): if this sweep finds
  Low reproducibility, the inventory's accept/modify/reject signal becomes harder to read cleanly
  — a human's "Modify" on a tech-stack proposal might reflect a genuinely bad recommendation, or
  might just reflect an unlucky roll a different run wouldn't have produced. High reproducibility
  makes the inventory's signal cleaner to interpret: a Modify would more confidently mean "the
  model's one stable answer didn't match domain knowledge," not "this particular sample was off."
- **Technology recommendation review**: `unresolved-decisions-become-explicit-decision-points`
  (an existing, `high`-confidence principle) argues that a genuine, unsupported judgment call
  should become an explicit, human-reviewable Decision Point rather than a silent interpretation.
  A Low result here would be direct evidence that technology recommendation is exactly this shape
  of judgment call, today handled as a Recommendation with only a per-proposal accept/modify/reject
  gate, not a Decision Point — worth knowing before any conversation about whether that principle
  should extend upstream. A High result would suggest today's shape may already be adequate for
  this specific decision.
- **Service discovery review**: architectural divergence specifically (a different *set* of
  services discovered) would be the single most concerning outcome this sweep could produce, more
  urgent to investigate further than tech-stack wording variance alone — it would mean the very
  shape of "what services exist" isn't determined by the input, not just how they're described.
- **Composition experiments**: Stage 6's real, non-synthetic dependency edge rests on
  `manufacturer-001`'s current `services.yaml`/ADRs as given facts. A Low result here — especially
  architectural divergence — would be worth naming as a scope caveat on Stage 6's own evidence
  base (the specific architecture Stage 6 tested against might not have been the only plausible
  outcome of this story's own spec run), without it invalidating anything Stage 6 actually found
  about contracts, which is a separate question from whether this was the "right" architecture.

---

## 7. Expected Learning Value

Regardless of outcome, this sweep answers a question with **zero** existing evidence today (the
entire evidence base is one anecdotal, unmeasured observation) using a design that's interpretable
under all three possible results (High, Medium, Low) — not one that only produces a usable
conclusion if the result comes out a particular way. That symmetry is deliberate: a design that
can only conclude something interesting when it finds variance would bias toward finding it. Cost
is low (one new standalone example file, no new fixture data, no modification to any production
code path) relative to the learning value (a previously entirely unmeasured, upstream-of-everything
part of the system), matching the Roadmap Reassessment's own stated reasoning for ranking this
above composition's remaining open questions.

No implementation has been performed. This document defines the sweep; running it is a separate,
later step.

---

## Results (2026-07-15): Low reproducibility, confirmed

Implemented exactly as designed: `canopy-llm/examples/pre_behavior_reproducibility_sweep.rs`
calls `identify_architectural_questions` 5 times against the frozen pre-spec state (empty
`ServicesRegistry`, empty `Vec<Adr>`, real `manufacturer-001` story and `domain_registry.yaml`).
Full raw output: `pre_behavior_sweep.log` (scratchpad). No modification to any production call
site; strictly read-only against the dogfooding project.

### Per-category classification

| Category | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 | Tier |
|---|---|---|---|---|---|---|
| Backend service name | `manufacturer-registration-service` | same | same | `manufacturer-service` | same | 2 (equivalent) |
| Backend technology | Spring Boot | Spring Boot | Spring Boot | Spring Boot | Spring Boot | 1 (identical) |
| Frontend component name | `manufacturer-registration-portal` | same | `manufacturer-portal` | `manufacturer-portal` | same | 2 (equivalent) |
| Frontend technology | React | React | **Angular** | React | React | **3 (materially different)** |
| Database | PostgreSQL | PostgreSQL | PostgreSQL | PostgreSQL | PostgreSQL | 1 (identical) |
| Event broker | **Kafka** | Redpanda | Redpanda | Redpanda | Redpanda | **3 (materially different)** |
| Domain-event proposal present? | **No** | Yes | Yes | Yes | **No** | **4 (architectural divergence)** |
| Domain-event follows "on topic" convention? | n/a | No | No | **Yes** | n/a | **4 (architectural divergence)** |
| Total proposal count | 4 | 5 | 5 | 5 | 6 | **4 (architectural divergence)** |

Backend/frontend naming (tier 2) split in a **correlated**, not independent, pattern: runs 3 and
4 both chose the shorter, entity-scoped names (`manufacturer-service`/`manufacturer-portal`) for
*both* the backend and frontend, while runs 1, 2, and 5 both chose the longer, process-scoped
names (`manufacturer-registration-service`/`-portal`) for both — a real naming-convention split
the model applies consistently within a run, not a coin-flip per proposal. Worth naming precisely
rather than only noting variance exists: for this one story, both namings equally identify the
correct real-world service, so this specific instance doesn't cross into tier 3 — but the two
conventions differ in scope (process-scoped vs. entity-scoped), which could matter for how a
second, related story's service ownership resolves later. Not judged further here, per this
sweep's own scope.

### Verdict: Low reproducibility

Per §4's own thresholds: any tier-4 classification in any run is sufficient for "Low," and three
independent tier-4 findings are present (domain-event presence, domain-event convention
compliance, and total proposal count all vary in structure, not just content) — on top of two
separate tier-3 findings in different runs (frontend tech in run 3, event broker in run 1). This
is not a marginal or borderline case.

### The most significant single finding

**The domain-event proposal's own presence, and its compliance with the "`<EventName>` on topic
`<topic>`" convention `parse_event_adr` requires, is the least stable output of the entire sweep.**
Present in 3 of 5 runs; of those 3, only 1 (run 4) actually included the topic clause the later
mechanical pipeline (`mechanical_event_behaviors`) needs to produce any `EventShape`/`Publication`
behavior at all. That's 1 of 5 runs (20%) producing a domain-event ADR the pipeline could consume
as designed, absent this sweep's own after-the-fact correction.

This directly reframes a conclusion from an earlier stage of this investigation. The Contract
Readiness Assessment (2026-07-14, Q6 #2) found the real dogfooding project's ADR-006 lacked the
topic clause and attributed this to the project's `decisions/` directory *predating* `cmd_init`'s
mandatory Topic Naming Convention ADR — "a stale-fixture artifact, not a reachable defect in
current code." Stage 6 (2026-07-15) then hand-corrected that ADR to add the missing clause, on
the same "stale fixture" reasoning, to exercise the mechanical dependency rule. **This sweep shows
the missing topic clause is not only a legacy/stale-fixture issue — a freshly-generated proposal,
today, against the same story, still fails to include it 4 times out of 5.** The earlier
diagnosis wasn't wrong (the specific historical ADR genuinely did predate the mandatory step), but
it understated the scope: the underlying instability is live, not just historical.

### What this does and doesn't justify concluding

**Justified**: for `manufacturer-001`, at this one frozen pre-spec snapshot, pre-behavior
planning's architectural-questions mechanism is Low reproducibility — confirming the anecdotal
tech-stack-variance observation that opened this investigation was real, not a one-off, and
identifying the domain-event proposal specifically as the least stable element.

**Not justified**: any claim about *why* (this design isolates model-sampling variance only, per
§5's own stated scope — it does not test the order-dependent-prompt-content variability source
separately); any claim generalizing beyond this one story; and no claim about whether any specific
recommendation is *good* — this sweep measured agreement, not quality, exactly as designed.

### Relationship to future work — outcomes now known, not hypothetical

- **Human-Insight Inventory**: a Low result means its accept/modify/reject signal will need
  reading carefully — a human's edit to a tech-stack or domain-event proposal may reflect a
  genuinely unstable recommendation, not necessarily a bad one considered on its own merits.
- **Technology recommendation review**: this result is direct, no-longer-anecdotal evidence for
  the shape `unresolved-decisions-become-explicit-decision-points` already describes — a
  genuinely unsupported judgment call, today handled as a Recommendation with only a per-proposal
  gate, not a Decision Point.
- **Service discovery review**: the domain-event proposal's structural instability (present/absent,
  convention-compliant/not) is a stronger, more specific version of "architectural divergence" than
  this design anticipated in the abstract — worth prioritizing over the naming-convention variance,
  which turned out to be tier 2, not tier 3/4.
- **Composition experiments**: Stage 6's real dependency edge existed only because of a manual
  correction this sweep now shows the model itself would supply unprompted only 1 time in 5. This
  doesn't invalidate Stage 6's findings about contracts, but it is a concrete, no-longer-speculative
  scope caveat on how that dependency edge came to exist in the first place.
