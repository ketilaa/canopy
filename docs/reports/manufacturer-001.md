# Story Report — `manufacturer-001`

A running record of every dogfooding session and reproducibility sweep run against one story
(registering a manufacturer) across the behavior-first planning pipeline's first two days of live
use. Consolidated from session logs and sweep reports written 2026-07-13 through 2026-07-14.
Kept in date order so the arc — what broke, what got fixed, what a repeat measurement showed — stays
visible rather than being flattened into a single "current state" summary.

---

## Session 1 — Initial dogfooding run (2026-07-13)

**Setup:** a dogfooding e-commerce project, reset to a clean slate before the session. Model:
Qwen2.5-Coder-14B-Instruct-GGUF, served locally. Driver: a scripted pseudo-terminal session acting
as the human at every interactive gate. Scope: `canopy intent` → `canopy spec` → `canopy behaviors`
for one new story — registering a manufacturer.

### Intent

Given the plain statement "Manufacturers must be registered in the system before products can
reference them," the model derived:

```yaml
stories:
  - id: account-001
    as_a: Manufacturer
    want: register in the system
    so_that: products can reference them
```

Rejected on review for two reasons: the actor was wrong (manufacturers don't self-register; a
product manager enters the data), and `want` never named the entity — which had a direct
downstream consequence, since domain-vocabulary extraction reads only the `want` field and, with
no mention of "manufacturer" in it, extracted the entity as `User` and the event as
`UserRegistered`. Corrected by hand to:

```yaml
stories:
- id: manufacturer-001
  as_a: product manager
  want: register a new manufacturer
  so_that: products can reference a valid, known manufacturer
  depends_on: []
  status: accepted
```

**Finding:** domain-vocabulary extraction has no fallback when `want` doesn't name the entity — it
invents something plausible-sounding but wrong rather than failing.

### Spec

Seven architectural proposals generated in one call, all accepted. **Finding:** no domain-event ADR
was proposed for this story, unlike an equivalent earlier session for a different story (same
event-driven architecture, same shape of story) — a materially different ADR set from an
apparently-identical setup, with a visible downstream effect (see Stage 1 below).

Generated schema: `Manufacturer` with `name`/`country` (mandatory), `website` (optional). First pass
produced 3 scenarios; an open question about name-uniqueness was resolved by hand (unique globally,
not per-country) and a fourth scenario added.

### Stage 0 — Specification Completeness

First check against the 4-scenario spec: 6 real gaps, all missing constraint-violation scenarios.
Six more scenarios written, one per flagged constraint, each with an explicit `constraints` field
naming the exact rule.

**Re-check against the now-10-scenario spec found the identical 6 gaps again, verbatim** — despite
six scenarios now existing that directly, explicitly addressed every one of them, each with a
`constraints` field stating the exact rule. In the same response, the model's own second checklist
correctly echoed all ten scenarios back, proving it had "read" them — yet still answered the
coverage question as if none of them existed.

After a prompt structure change: 6 false gaps → 1 on the immediate re-run (defensible, an edge
case). A second, otherwise-identical re-run of the *same unchanged spec* produced 3 gaps — a
different set than the previous run's 1, despite dedicated scenarios existing for all three.

**Finding:** Stage 0's constraint-coverage judgment was not deterministic between otherwise-identical
calls against an unchanged specification.

The pipeline was allowed to proceed past Stage 0 with an internal test-only override, given no
single run could be treated as authoritative.

### Stages 1–4

- **Stage 1:** 37 behaviors extracted, 0 coverage findings. **Finding**, following from the missing
  domain-event ADR above: zero mechanical event-shape/publication behaviors were produced — a direct,
  visible consequence of the missing ADR, versus an equivalent earlier session that had dedicated
  event-shape behaviors.
- **Stage 2:** no decision points (the uniqueness question was already resolved during spec review) —
  a clean, gate-free pass.
- **Stage 3:** 4 unit clusters + 1 integration grouping, 0 clustering findings. LLM review of the
  mechanical baseline found 4 issues: one genuine (a vacuous `min_length: 0` constraint had produced
  a nonsensical "shorter than 0 characters is rejected" behavior — a real, self-caught defect), three
  likely false positives (construction behaviors flagged as requiring persistence-layer access, when
  assigning fields in isolation is exactly what a factory should do).
- **Stage 4:** 5 contracts, 0 contract-audit findings. Dependency review added 3 missing dependencies
  the mechanical substring-matching baseline missed — all 3 validation contracts, because the
  workflow's own error-translation text says "Name," never the compound "ManufacturerName" the
  cluster is keyed on. **Finding:** this is the second live confirmation (after an equivalent gap in
  an earlier session) that the mechanical dependency baseline is a deliberately crude first pass, not
  a substitute for review.

### Summary of Session 1 findings

1. Domain-vocabulary extraction has no fallback for an under-specified `want`.
2. The same architecture/story shape produced a different ADR set between two sessions, with a
   visible downstream effect.
3. Stage 0 reported 6 gaps against scenarios that directly addressed every one of them.
4. Stage 0's gap-finding was non-deterministic run-to-run against an unchanged spec.
5. A vacuous constraint produced a nonsensical behavior statement, caught by Stage 3's own review.
6. The mechanical dependency-inference baseline reliably missed validation contracts specifically.

---

## Reproducibility Sweep 1 (2026-07-13) — same intent, 3 runs

**Method:** reset to a clean slate, run `intent → spec → behaviors` three times with identical
(default) gate answers throughout, so every difference between runs is attributable to model
sampling, not human curation.

**Story generation:** story-ID prefix was "manufacturer" in 2 of 3 runs, "account" in 1. **All
three runs** produced the literal `want` text "register an account" — the exact CORRECT-column
example from that session's own naming rule, copied near-verbatim regardless of the story's actual
subject. One run also generated an unplanned second story beyond what was asked.

**Domain vocabulary:** "Manufacturer" survived as an entity in all 3 runs even against the polluted
`want` field. Event naming varied legitimately (`ManufacturerCreated` vs. `ManufacturerRegistered`
— both defensible under the extraction rule's own ambiguity). Two of three runs introduced a second,
incorrect entity alongside Manufacturer.

**Entity schema — the most severe finding of the sweep.** One run's generated schema was `entity:
Account` with `username`/`password`/`email` fields — a generic authentication schema, not a
manufacturer, despite that same run's own prompt directly stating `as_a: manufacturer` and `Domain
Entities: Manufacturer`. Nothing about the schema described a manufacturer at all. This is a live,
concrete example of exactly the cross-stage divergence a mechanical Entity Continuity check would
catch: story and domain registry both said "Manufacturer," entity schema said "Account," and nothing
existed yet to fail loudly on that mismatch.

**Stage 0:** both completed runs correctly found real, unaided scenario-coverage gaps (constraint
violations, unwritten). **Stage 2:** both runs correctly created one Decision Point per open
question and correctly blocked further stages when left unresolved.

**Conclusion:** the entity-schema divergence was the single most convincing piece of evidence for
building an Entity Continuity audit next — cheap (a string comparison, no model call), and this run
showed concretely what it would have caught.

---

## Reproducibility Sweep 2 (2026-07-13) — confirmatory, post-fix, 2 runs

Run after the want-naming rule was rewritten and Entity Continuity was added.

- **Entity derailment: gone, 2/2.** Both runs produced `entity_schema.entity: Manufacturer`; the
  continuity check never fired (nothing to catch). Combined with a direct verification run just
  before this sweep, 3 consecutive clean runs since the fix.
- **Domain vocabulary: stable, 2/2** on Manufacturer as the core entity.
- **Domain-event ADR: appeared in every completed run since the fix, 2/2.**
- **Scenario generation confirmed as the primary remaining source of variance.** Neither run came
  close to covering its own schema's constraints unaided (5–6 missing-scenario gaps each).
- **Business Policy Discovery still drifted run to run** — both *which* policy surfaced and *how
  many* varied between the two runs.
- Story-ID prefix instability and the scope-creep extra-story generation both recurred (now 2 of 5
  runs across both sweeps for each) — noted, deferred, not yet at the severity the entity divergence
  had shown.

**Conclusion:** the pipeline had shifted from "can the model become a different system" (now
well-guarded) to "did we discover every relevant behavior" (still open) — a healthier class of
problem. Recommended proceeding to Event Continuity next, then treating scenario-coverage
enumeration as the next major piece of work.

---

## Reproducibility Sweep 3 (2026-07-14) — after Scenario Coverage Enumeration, 3 runs

Run after Scenario Coverage Enumeration and Policy Discovery Enumeration both landed, from an
identical fixed baseline (entities/ADRs/services held constant across all 3 runs, isolating spec-
generation variance specifically).

- **Zero crashes; the coverage-matrix mechanism worked exactly as designed.** Field-count and
  policy-count varied slightly (schema field naming still isn't fully stable run to run), and
  scenario count matched the computed coverage matrix size exactly every time — 12, 15, 16
  scenarios across the 3 runs, each number fully explained by that run's own matrix size, not by
  independent scenario-writing drift.
- **New finding: the policy checklist's own resolved/not-applicable/unresolved classification was
  not reliably honored, even though it was already fully enumerated.** One run put 3 of 6 policy
  items into an unsanctioned fourth bucket the prompt never offered, rather than one of the three it
  did.
- **Separately, live inspection of a generated spec found the model confidently answering business
  questions it had no basis to answer** — a specific authorization role, a specific default value,
  a specific retention statement, none of them present anywhere in the story, ADRs, or domain
  vocabulary shown to it.
- **Confirmed, quantified: the domain-event-ADR duplicate bug reproduced in 2 of 3 runs** — a third
  independent occurrence of a bug that had been noted but not yet fixed.

**Conclusion:** this sweep directly motivated two fixes — forcing the policy checklist into exactly
six named, evidence-required entries, and computing domain-event-ADR existence as a mechanical fact
instead of asking the model to check.

---

## Reproducibility Sweep 4 (2026-07-14) — confirmatory, after both fixes, 3 runs

Same fixed-baseline methodology, run after the evidence-grounding fix and the mechanical
domain-event-ADR fix both landed.

- **Domain-event-ADR duplication: fully fixed.** 6 ADR files in all 3 runs — zero duplicates, down
  from the ~2/3 incidence measured in Sweep 3.
- **Policy fabrication: sharply reduced.** Before the fix, most runs had confidently "resolved" 5
  of 6 policy questions with fabricated specifics. After: only 1–2 of 6 resolved per run, with the
  rest correctly landing as open questions or a silently-accepted "not applicable" — and the
  stricter evidence-presence check never once fired an error across these runs, meaning the model
  was genuinely satisfying the requirement, not just failing it repeatedly.
- **Genuine residual finding, milder than before:** the evidence citations for the remaining
  "resolved" items were sometimes generic rather than specifically substantiating — e.g. citing the
  full story text as the source for a uniqueness rule the story doesn't literally state. The model
  now must cite a source; nothing yet checks that the cited source actually supports the specific
  claim.
- Entity identity and ADR count remained rock-solid across all 3 runs. Schema field-naming variance
  (e.g. `contactPhone` vs. `phoneNumber`, presence/absence of a `website` field) persisted unchanged
  — a known, separate, not-yet-addressed variance source.

**Conclusion:** all three steps of the stated remediation plan (fix the ADR bug → sweep → measure
policy stability) are confirmed done. The remaining problems at this point are localized,
measurable, and auditable — a materially different class of problem than the severe entity
derailment Sweep 1 found two days earlier.

---

## Contract-Driven Implementation, Stage 1 (2026-07-14) — Single-Contract Parallel Implementation Trial

First evidence in this report generated *after* the behavior-first planning pipeline's Stages 0–4
were confirmed stable (Sweep 4, above) — the question shifted from "is planning reliable" to
"can the resulting contracts actually drive implementation." Full design in
`docs/design/contract-driven-implementation-experiment.md`; full assessment of what a contract
carries in `docs/contract-readiness-assessment.md`. Neither `canopy implement` nor any production
code path was touched by this or the Stage 2 session below — both ran as standalone cargo
examples.

No real `contracts.yaml` existed for this story before this session. Running
`canopy behaviors manufacturer-001` for real hit Stage 0's completeness gate — the story's own
`spec.yaml` had no scenario testing any field's boundary constraint. Fixed directly in the YAML
(7 added boundary scenarios, 3 orthogonal open questions cleared), then Stages 0–4 ran clean and
produced six real contracts: five single-field validation contracts (`name`, `address`,
`phoneNumber`, `email`, `website`) and one construction contract (`id`/`createdAt`/`modifiedAt`).

Selected `ManufacturerNameValidation` — one field, two behaviors, zero dependencies — and gave a
model *only* that one contract plus the resolved file target and the Spring Boot skill. Three
runs, same reproducibility standard as the earlier sweeps:

- **Every run produced a real, distinct defect — none passed clean.** Run 1: correctly scoped but
  used a non-idiomatic manual validation method. Run 2: imported `javax.validation` despite the
  skill's explicit "never javax" rule, and invented an unauthorized `@Entity`/`@Id`/
  `@GeneratedValue` the single given contract never licensed. Run 3: correct imports, same
  unauthorized entity invention, a different message-fidelity break.
- **The unauthorized-field invention (2 of 3 runs) was the headline finding** — traced not to a
  missing contract fact but to a live hypothesis: the model was shown only one of several
  contracts that would eventually share this file, and defaulted to "completing" what looked like
  a whole JPA entity regardless of the explicit scope instruction.
- The other two defects (a `javax` import despite an explicit rule, and a `@Size` annotation that
  can only carry one message for two distinct required behaviors) were diagnosed as a model-
  variance slip and a genuine Spring Boot skill documentation gap, respectively — neither traced
  to the contract schema.

**Conclusion:** the contract boundary itself held up under this trial; every failure traced
outside it. No new `Contract` field was proposed. Directly motivated Stage 2 below — testing
whether showing a model every contract sharing a file, not one, stops the unauthorized invention.

## Contract-Driven Implementation, Stage 2 (2026-07-14) — Full-File Contract Visibility Trial

Used the same real `contracts.yaml` from Stage 1 — no synthetic data needed. `resolve_
implementation_target` places all six of the story's unit contracts (five validation, one
construction) at the same file, `Manufacturer.java` — exactly the multi-contract-per-file case
Stage 1's failure pointed at. Gave a model all six contracts' combined scope at once (still
withholding story, scenarios, entity_schema, ADRs, OpenAPI, and any exploratory tool), asked for
one combined test file and one combined implementation. Three runs:

- **Ownership correctness: 3/3 clean — the Stage 1 hypothesis confirmed, not just plausible.**
  Every run produced exactly the eight authorized fields (five validated, three constructed) and
  nothing beyond them. `@Entity`/`@Id`/`@GeneratedValue` appeared in all three runs and correctly
  so this time, since `ManufacturerConstruction` now explicitly licensed them — zero unauthorized
  fields, zero unrelated methods, across all three runs.
- **A second, more severe failure mode surfaced, exactly where Stage 1 predicted one would
  persist regardless of ownership visibility.** 2 of 3 runs produced a `Manufacturer` class whose
  only enforcement was declarative Bean Validation annotations with no triggering mechanism at
  all — a plain `new Manufacturer(...)` call never throws, so every one of that run's 7 boundary
  tests would fail. A broader version of Stage 1's `@NotBlank`/`@NotNull` message finding: the
  Spring Boot skill never explains whether or how Bean Validation fires outside a full
  persistence/`@Valid` context, so the model guesses inconsistently.
- **A third, fully reproducible (3/3) defect: `id` is never actually assigned at construction
  time.** Every run relied solely on `@GeneratedValue`, which only fires at JPA persist time, not
  on a plain constructor call used in a unit test — the JVM-side analogue of a convention the
  Node/Express skill already states explicitly (eager id assignment via `randomUUID()`) that
  Spring Boot's skill has no equivalent for.

**Conclusion:** ownership visibility was confirmed as the cause of Stage 1's invention — a
process fix (group contracts by resolved target before generating), not a schema change, per the
decision table scoped before this ran. The other two findings are real, reproducible, separately-
scoped Spring Boot skill gaps, independent of the contract-driven hypothesis and unaffected by
ownership visibility, exactly as predicted going in. No redesign conversation was warranted —
every defect in both sessions traced to a specific, nameable, addressable cause.

## Current open items for this story

- The domain-event-ADR fix's operation-classification logic (whole-word verb matching, exact
  canonical-suffix matching) has not yet been tested against a story requesting an *update* or
  *deletion* operation for an entity that already has a *creation* event ADR — only the creation
  case has been exercised live so far.
- Policy-evidence citations are checked for presence, not for whether they actually support the
  specific claim made — flagged as the next candidate refinement if it recurs as a problem.
- Entity-schema field-naming variance (which optional fields appear, what they're called) remains
  unaddressed and untouched by either enumeration fix, since it's produced by an earlier stage
  (schema generation) than either fix targeted.
- **New from Stage 2:** `spring_boot_skill` doesn't document how Bean Validation actually gets
  triggered outside a full persistence/`@Valid` context — 2 of 3 Stage 2 runs produced a class
  whose annotations never fire on plain construction, so every boundary test would fail. Not yet
  fixed; a specific, reproducible skill gap, independent of contracts.
- **New from Stage 2:** `spring_boot_skill` has no equivalent to Node/Express's "factory assigns
  id eagerly at construction" convention — every run relied solely on `@GeneratedValue`, which
  never fires on a plain constructor call. 3/3 reproducible. Not yet fixed.
- **New from Stage 2:** the ownership-visibility process fix (group contracts by
  `resolve_implementation_target` output before generating, rather than one contract at a time)
  is confirmed necessary but not yet formalized anywhere — still only exercised inside a
  standalone example, not any production code path.
