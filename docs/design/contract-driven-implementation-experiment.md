# Contract-Driven Implementation: Experiment Design

Status: Stage 1 executed 2026-07-14 (see "Stage 1 Results" below); Stage 2 scoped, not yet
implemented (see "Stage 2 Design" at the end). `canopy implement` remains unchanged throughout —
every stage so far has run as a standalone cargo example, never touching production code.

Date: 2026-07-14

Goal, stated precisely per the user's framing: not "replace `canopy implement`," but **"validate
contract-driven implementation with the smallest safe experiment."** The question being asked is
whether contracts can become the primary implementation boundary — not whether the whole pipeline
should be wired to them yet.

---

## 1. What the current implement pipeline actually consumes, by artifact

Grounded in `canopy-llm/src/prompts/plan.rs`, `step.rs`, `fix.rs`, and
`canopy-cli/src/commands/implement/{mod,execute,plan}.rs` — confirmed by reading each file, not
inferred from the design docs' own claims about them.

| Artifact | `plan.rs` (file discovery) | `step.rs` (test/impl generation) | `fix.rs` (repair loop) |
|---|---|---|---|
| **Story** (`as_a`/`want`/`so_that`) | Yes — every discovery/ordering prompt | Yes — every stub/test/impl prompt | No — not in `fix_prompt`'s signature at all |
| **Spec: `entity_schema`** | Yes — full YAML, always | Yes — full YAML, gated to the "model" layer only | No |
| **Spec: `scenarios`** | Yes — full YAML, always (drives file discovery) | Yes — full list, "one test method per scenario," filtered by four separate layer-conditional exception rules (`scenario_coverage_note`, `boundary_rule`, `missing_field_exception_rule`, `mock_dependencies_rule`) | No |
| **ADRs** | Yes — `adrs_summary` text + `skills_for_architecture` → `arch_skills`, plus `event_scope_rule` branches on broker-ADR presence | Yes — `arch_skills` again, plus `testing_skill_from_adrs` | Yes — `arch_skills` passed straight through |
| **Behaviors** (`behaviors.yaml`) | **No** | **No** | **No** |
| **Contracts** (`contracts.yaml`) | **No** | **No** | **No** |

Confirmed by grep across the whole `canopy-cli/src/commands/implement/` tree: zero references to
`Contract`/`Behavior`/`contracts.yaml`/`behaviors.yaml` anywhere. `canopy behaviors <story-id>`
and `canopy implement <story-id>` are today two fully disconnected pipelines that happen to read
the same `spec.yaml`/ADRs independently — not a partially-wired system, a completely unwired one.

**ADRs are the widest-reaching artifact today** — the only one of the three (story/spec/ADRs)
that reaches all the way into the fix loop. This matters for Q2 below: `fix.rs` is the *least*
coupled to story/spec (no `story`/`spec` parameters at all — only `arch_skills` as its one
upstream-derived input), which makes it the cheapest place to substitute a contract-derived input
for an ADR-derived one, but also the least informative, since the fix loop reacts to errors
rather than deciding what to build — not where the hypothesis actually gets tested.

## 2. Which part should become the first contract consumer?

Three candidates, ranked by how directly each maps onto what contracts already provide:

1. **`step.rs`'s test-generation prompt (`unit_test_stub_prompt`/`_ts`).** Today this iterates
   *every* scenario and applies four separate filters to decide which ones apply to this file's
   layer. A contract's `required_tests` is already exactly the pre-filtered list — no filtering
   logic needed, because Stage 3's clustering already partitioned behaviors by layer. This is the
   single most direct match between what a contract offers and what a current prompt has to work
   around today; it's also literally what `docs/design/behavior-first-planning.md`'s own "What
   this replaces" section names.
2. **`plan.rs`'s file-discovery step.** Contracts could replace this outright — one contract, one
   file, mechanically, via `resolve_implementation_target`. Structurally cleaner (no LLM guessing
   "what files are needed" at all), but the largest behavior change: it replaces the planner for
   an entire story, not one file.
3. **`fix.rs`.** Smallest code change (swap one `arch_skills` string for a contract-derived one),
   but the fix loop doesn't decide what to build — it only repairs what's already there. Least
   informative for testing the actual hypothesis.

**Recommendation: (1), `step.rs`'s test generation, for one already-existing file.** It's the
smallest surface area, the most direct substitution the design was built for, and it doesn't
require touching file discovery, `plan.yaml`'s shape, or the TDD/fix-loop machinery at all.

## 3. Smallest parallel implementation path to prove or disprove the hypothesis

**A new, separate, opt-in code path — not a modification to `canopy implement`.** Concretely:

- **Prerequisite, not yet satisfied:** a real, freshly-generated `contracts.yaml` for an actual
  story. Today none exists on disk — `product-001`'s data is gone (the project it lived in was
  deleted earlier this session) and `manufacturer-001` has never been run through Stages 1–4.
  Running `canopy behaviors manufacturer-001` for real, end-to-end, is step zero — everything
  below is desk design until that exists.
- **One hand-picked, low-risk unit contract** — a single-field validation contract is the safest
  starting point (smallest blast radius, matches the probe already run in Addendum 3).
- **Generate exactly that contract's file + test pair**, through a new isolated path (a CLI
  subcommand behind an explicit flag, or another `canopy-llm` example, in the same spirit as
  `contract_isolation_probe.rs` but now wired through the *real* production functions —
  `resolve_implementation_target`, `skill_for_technology`, the real `Contract` loaded from disk —
  instead of a hand-typed prompt).
- **Manually reviewed, not auto-fed into the existing TDD Red/Green/fix loop.** The first
  experiment stops at "generate and inspect" — it does not touch `execute.rs`, does not write
  into a real `plan.yaml`, and cannot regress anything the current pipeline already does, because
  it runs nowhere near it.

This is deliberately smaller than "prototype single contract implementation" as originally listed
in the readiness assessment's Q7 options — it's a probe of *one already-validated contract*
through the *real* skill/file-target machinery, not a new subsystem.

## 4. What a contract-driven implementation path should be allowed to consume

- The contract itself: `id`, `name`, `scope`, `kind`, `entity`, `member`, `mandatory`,
  `required_tests`, `dependencies`, `derivation`.
- For each id in `dependencies`: that dependency contract's own `entity`/`member`/`required_tests`
  — not its full file content. If the dependency's file already exists on disk, its Roots symbol
  surface (the same mechanism `sibling_section` already uses today) is the appropriate way to see
  its real exported shape — scoped strictly to contracts this one's `dependencies` actually names,
  not an open-ended Roots search.
- The tech-stack skill for the resolved layer (`skill_for_technology`) — the single existing,
  correct channel for anything tech-specific, unchanged by this experiment.
- The resolved implementation file path (`resolve_implementation_target`).
- Existing file content, only if the operation is `modify` — for the *first* experiment, restrict
  to a `create`-only contract to avoid this complexity entirely.

## 5. What it should explicitly NOT consume

- **The story's `as_a`/`want`/`so_that` narrative.** This is the core of the hypothesis: can a
  contract stand in for the story entirely? Feeding the narrative in alongside the contract would
  make a pass uninformative — it wouldn't be clear which one carried the signal.
- **The full BDD scenario list.** Only the contract's own `required_tests` — feeding the full
  list back in would just reintroduce the filtering problem contracts exist to remove.
- **The full `entity_schema`.** Only what's already on the contract (`entity`/`member`/
  `mandatory`). Falling back to `entity_schema` when the contract seems thin would mask exactly
  the kind of gap this experiment is designed to surface.
- **ADRs, directly.** Architecture-derived facts should already be behaviors (an event-shape or
  publication contract exists precisely so an ADR doesn't need to be re-read at generation time).
  Feeding ADR text in here would undermine the thing being tested.
- **The OpenAPI spec.** Irrelevant to a non-route contract; for a route contract specifically,
  this is an open question deliberately deferred to a later, route-focused experiment — not
  answered by this one.
- **Open-ended tool access** (`find_symbol`/`read_file` for exploration). Per
  `docs/design/behavior-first-planning.md`'s own stated philosophy — "use tools to provide facts,
  use contracts and behaviors to provide decisions" — the first experiment should give the model
  no way to go looking for more context to compensate for a contract gap. If the contract is
  insufficient, that needs to be visible as a failure, not quietly patched over by a tool call.

## 6. Migration stages, current → contract-driven

Each stage is independently stoppable — a failure at any stage means stopping there, not pushing
through to the next.

| Stage | What changes | What stays untouched |
|---|---|---|
| **0 — done** | Contracts validated as sufficient in isolation for a single field (this investigation, `docs/contract-readiness-assessment.md`). | Everything — no production code path reads a contract yet. |
| **1 — proposed next (§3 above)** | One contract → one file + test pair, generated via a new isolated path, manually reviewed. | `canopy implement`, `plan.yaml`, `execute.rs` — completely untouched. |
| **2** | Extend to a small *set* of contracts for one real story/service — tests whether multiple contracts compose correctly (dependency ordering; cases like Validation+Construction sharing one file, already observed in Addendum 2's Spring Boot trace). Still generated in isolation, still manually reviewed. | Still no wiring into the real TDD loop. |
| **3** | Wire that same small set into the *real* TDD Red/Green loop (reuse `execute.rs`'s test-run/fix-loop machinery) — first point contract-driven code is actually compiled and tested for real, not just eyeballed. | `plan.rs`'s file discovery still runs as it does today for everything else. |
| **4** | Replace `plan.rs`'s LLM-driven file discovery with contract-driven enumeration (one contract = one file, mechanically) for one entire story. | Story/spec/ADR-driven generation remains the path for every other story. |
| **5** | Full cutover: `canopy implement` requires `canopy behaviors` to have run first; `plan.yaml`'s shape becomes contract-referencing; the current story/spec/ADR-driven prompts become a fallback, not the default. | — |

Stage 1 is the only one being proposed for actual work right now.

## 7. Concrete success criteria for the first experiment

**Experiment name:** Single-Contract Parallel Implementation Trial.

**Inputs:** one real, freshly-generated unit contract (single-field validation, `create`
operation) from a real `contracts.yaml` — requires running `canopy behaviors manufacturer-001`
first. Everything else per §4/§5 above.

**Success criteria** (reproducibility-tested the same way the mandatory/optional probe was — at
least 3 runs, not one):

1. The generated test file compiles and its assertions map 1:1 onto the contract's
   `required_tests` — no fewer, no more, no drift into unrelated behavior.
2. The generated implementation file passes that test, reproducibly across the runs — or, where
   it doesn't, the failure is traceable to a **specific, nameable** missing fact on the contract
   (not "the model got confused," which would carry no actionable signal).
3. No hallucinated constraints — every validation rule in the generated code corresponds to an
   entry in `required_tests`; nothing invented.
4. The generated file's path matches exactly what `resolve_implementation_target` predicts —
   a trivial check given the function is doing the placing, but worth stating as an explicit
   acceptance criterion since it's the concrete claim Addendum 2 was building toward.

**Stop condition:** if 2 or more of 3 runs produce functionally wrong code for a reason that
*cannot* be pinned to a specific missing contract fact, that's a signal the gap is bigger than
another incremental field — worth stopping and having the redesign conversation, not iterating
another probe.

**What would make this a clear win:** all four criteria hold reproducibly, *and* the failure
modes (if any) each point at one nameable, addressable gap — mirroring exactly how the
mandatory/optional gap was found and closed in this investigation's Addendum 3.

---

## Stage 1 Results (2026-07-14)

### 1. Experiment setup

A real `contracts.yaml` did not exist for any story before this run — `product-001`'s data is
gone and `manufacturer-001` had never been run through Stages 1–4. Running
`canopy behaviors manufacturer-001` for real (via a pty-driven session, per this project's
established interactive-dogfooding practice) hit Stage 0's completeness gate: `manufacturer-001`'s
`spec.yaml` had no scenario testing any field's boundary constraint (every `max_length`/
`min_length` was a blocking gap). Fixed the input artifact directly, per this project's own
documented "human-in-the-loop corrections belong in the saved YAML" practice: added 7 boundary
scenarios (one per missing constraint) to `spec.yaml`, and cleared `open_questions` (all three
were orthogonal business-policy questions — phone format, concurrency policy — unrelated to the
validation contract this experiment targets, and leaving them open would otherwise route through
Stage 2's interactive Decision-resolution gate for no reason relevant to this trial). This is an
input-artifact fix, not a synthetic contract — Stages 1–4 then ran unmodified and produced a real
`contracts.yaml` mechanically and via their own existing LLM calls, exactly as `canopy behaviors`
does for any story.

One pre-existing, orthogonal issue surfaced along the way, not fixed here: Stage 1's LLM-driven
scenario→behavior extraction call produced zero surviving behaviors for all 12 scenarios (flagged
correctly by `audit_behavior_coverage`) — separately, the ADR-event-coverage audit added earlier
in this investigation correctly flagged that `manufacturer-001`'s "Domain Event for Manufacturer
Registration" ADR produces no `EventShape` behavior (its decision text predates the mandatory
Topic Naming Convention ADR, as already diagnosed in Addendum 3's discussion). Neither affects
the contract selected below, which is entirely mechanically derived and untouched by either gap.

New code, all additive: `canopy-llm/examples/contract_driven_stage1_experiment.rs` (the
experimental path itself) and a `[dev-dependencies]` entry for `canopy-storage` in
`canopy-llm/Cargo.toml` (examples-only — the shipped library still doesn't depend on storage).
Nothing in `canopy-cli`, `plan.rs`, `execute.rs`, or `step.rs` was touched.

### 2. Chosen contract

`ManufacturerNameValidation` (`manufacturer-001-contract-001`): `kind: Validation`,
`entity: Manufacturer`, `member: name`, `mandatory: true`, `dependencies: []`,
`required_tests: ["Name longer than 200 characters is rejected.", "Name shorter than 1
characters is rejected."]`. Chosen because it's the same contract this whole investigation has
used since Q3's original hand-trace — a single field, two behaviors, zero dependencies, zero
contract-audit findings — the smallest, least ambiguous real case available, matching the
complexity level of the mandatory/optional probe exactly (same field, in fact).

### 3. Inputs actually used

Allowed and used, nothing else: the contract's own `kind`/`entity`/`member`/`mandatory`/
`required_tests` (loaded via `canopy_storage::load_contracts` — the same function `canopy
behaviors` itself calls); `resolve_implementation_target` (the mechanical resolver from Addendum
2) → `services/manufacturer-service/src/main/java/manufacturer_service/domain/Manufacturer.java`;
`skill_for_technology("Spring Boot", ..., "model")` — the real, unmodified skill text. This
contract's `dependencies` is empty, so no dependency-contract content was involved.

Confirmed absent by construction, not just by claim — grepped the experiment file itself for
`load_story_spec`, `load_all_adrs`, `load_story_openapi`, `load_user_stories`, and `ToolSpec`:
none appear anywhere except in the file's own doc comment describing what to check for. The
story narrative, the full scenario list, `entity_schema`, ADRs, OpenAPI, and exploratory tool
access were never constructed, let alone passed to the model.

### 4. Three-run results

| Run | Path | Test | Implementation | Ownership | Constraint fidelity |
|---|---|---|---|---|---|
| 1 | Correct | Calls an invented `validateName()` method directly (non-idiomatic — real Bean Validation is framework-triggered, not manually invoked) | Provides that same `validateName()` alongside a `@Size` annotation; self-consistent with its own test | **Correct** — only `name` + getter/setter, nothing invented | Both behaviors correctly enforced (test and impl agree with each other) |
| 2 | Correct | **Wrong**: imports `javax.validation.ConstraintViolation` — violates the skill's explicit, present "NEVER javax" rule | Adds an unauthorized `@Entity`/`@Id`/`@GeneratedValue` (fields belonging to a *different*, not-given contract); one `@Size` annotation carries only one message, assigned to the min-length case | **Violated** — invented `id`/`@Entity` beyond this contract's scope | **Broken** — the max-length test's exact-message assertion doesn't match the implementation's one shared message string |
| 3 | Correct | Correct `jakarta.validation.*` imports, real `Validator`, reasonable unit test | Adds the same unauthorized `@Entity`/`@Id`/`@GeneratedValue`; combines `@NotNull` + `@Size(message=...)` to attempt two distinct messages | **Violated** — same over-reach as run 2 | **Broken** — `@NotNull` doesn't reject an empty string (only `null`); the min-length test's assertion doesn't match what actually fires for `""` |

Every run produced at least one concrete, verifiable defect. No run passed clean.

### 5. Failure analysis

Three distinct failure classes, each traced to a specific cause — not lumped together as
"the model got confused":

**`javax` import (run 2).** Traces to **model variance on an instruction that was present**, not
a missing contract fact. `skill_for_technology`'s Spring Boot skill states "jakarta.* everywhere —
NEVER import javax.*" explicitly, and this text was confirmed present in every run's prompt (it's
part of the skill's `namespace_rules`, which — unlike the layer-partitioned skills — renders in
full regardless of layer). One of three runs violated a rule that was there. Not a contract gap.

**Wrong/missing constraint message (runs 2, 3).** Traces to a **tech-stack skill gap**, not a
contract gap. Jakarta Bean Validation's `@Size(min, max, message)` carries exactly one message for
both bounds — expressing two distinct messages needs `@NotBlank` (rejects `null` *and* empty/
blank, unlike `@NotNull`, which only rejects `null`) paired with `@Size(max=...)`. Spring Boot's
skill (`spring_boot_skill` in `tech_stack.rs`) doesn't currently document this `@NotBlank`-vs-
`@NotNull`-vs-`@Size` distinction anywhere. The contract itself is not at fault here — it correctly
states two distinct, separately-worded behaviors; the skill doesn't yet teach the one annotation
combination that expresses both faithfully. Addressable as a skill-prose fix, not a schema change.

**Unauthorized `@Entity`/`@Id`/`@GeneratedValue` (runs 2, 3 — the "ownership correctness" failure
this experiment specifically targeted).** The prompt explicitly instructed: "This file may
eventually need to satisfy OTHER fields/contracts not shown to you here — implement ONLY what
THIS contract requires... do NOT invent unrelated fields, methods, or class structure beyond the
minimum." Violated in 2 of 3 runs regardless. This is **not a missing contract fact** — nothing
about adding a new field to `Contract` would have told the model "don't add `id`" any more
directly than the instruction already did. It's also not simply "the prompt needs different
wording" in isolation — the more precise diagnosis, matching this project's own established
distinction between a missing instruction and a strong training prior overriding a present one:
asking a model to write "the complete file content" for something that *looks* like a JPA entity
class, while authorizing only one of that entity's several eventual fields, sets up a direct
conflict between the explicit scope instruction and a strong training-driven default ("a JPA
entity has an `@Entity`/`@Id`"). Run 1 (which asked for a much smaller, less entity-shaped
snippet, and got a correctly-scoped result) is suggestive evidence for this reading, though not
conclusive on its own. This points to a **process/staging question for Stage 2** of the migration
plan (§6 above) — whether multi-contract composition needs to show a model *every* contract that
targets a shared file at once, rather than asking it to write a "complete file" from a single
partial contract — not a contract-schema gap to patch with a new field.

### 6. Contract sufficiency assessment

**The contract itself held up under this trial — every failure traces outside the contract
schema.** `entity`/`member`/`kind`/`mandatory`/`required_tests` were loaded correctly, mapped
correctly to a resolved file target, and correctly constrained what the model was told to test
and implement. Nothing in any of the three failures would be fixed by adding a field to
`Contract` — they trace to (a) an existing skill instruction not always followed (model
variance), (b) a real, nameable tech-stack-skill documentation gap (`@NotBlank` vs `@NotNull` vs
`@Size`), and (c) a legitimate open question about how multi-contract file composition should be
staged, already anticipated in this document's own migration plan.

This is a materially different outcome than the mandatory/optional probe, and worth being
precise about the difference: that probe's failures traced to something *actually missing on the
contract* (no mandatory/optional signal existed anywhere), closed by adding one field. This
trial's failures trace to *skill incompleteness and a staging question*, not to anything missing
on the contract. Per the instruction to prove the diagnosis before proposing a fix: this is why
no new field is proposed here.

### 7. Recommendation

**Neither stop-condition applies as stated.** This is not "contracts are missing fact X" (no new
field is warranted) and it is not "consistently wrong with no identifiable cause" (all three
failure modes are concretely traced, per §5). The honest middle finding: **the contract boundary
itself is sound; two of the three defects are ordinary tech-stack-skill gaps, and the third is a
legitimate open process question this document already flagged as Stage 2's job, not Stage 1's.**

Concretely:
- **Do not add a new `Contract` field.** Nothing here demands one.
- **Worth a small, separate, tightly-scoped skill fix** (out of scope for this report to
  implement without confirmation): document the `@NotBlank`/`@NotNull`/`@Size` message
  distinction in `spring_boot_skill`. This is a real, nameable gap, independent of the
  contract-driven hypothesis — it would improve any Spring Boot generation today, contract-driven
  or not.
- **Proceed to Stage 2 (§6) to test the ownership question directly**, rather than trying to
  patch it with more Stage-1 prompt tuning: give a model *every* contract that targets one shared
  file at once (e.g. `ManufacturerNameValidation` + `ManufacturerConstruction`, both resolving to
  `Manufacturer.java`) and check whether ownership correctness improves when the model can see the
  file's full intended scope, rather than one partial slice of it. That is the next experiment
  this result actually points to.

---

## Stage 2 Design (scoped 2026-07-14, not yet implemented)

Of Stage 1's three failure classes (skill guidance, model variance, ownership visibility), this
scopes only the third — the one judged most architecturally significant and least understood.
The other two (the `javax` slip and the `@NotBlank`/`@NotNull`/`@Size` skill gap) are independent
of this question and are deliberately not addressed here.

**Hypothesis being tested:** the unauthorized `@Entity`/`@Id`/`@GeneratedValue` invention seen in
2 of 3 Stage 1 runs was caused by showing the model only *one* of several contracts that share a
target file — not by a general unwillingness to respect scope. If true, showing the model every
contract that targets the same file should eliminate the invention, because everything the model
added without authorization in Stage 1 (an `id` field) is actually authorized by a *different*
contract the model simply never saw.

### 1. Contracts that target a shared file

Using the real `manufacturer-001` `contracts.yaml` already generated for Stage 1 — no synthetic
data needed, this case already exists. `resolve_implementation_target` places **six** contracts
at the exact same file, `services/manufacturer-service/src/main/java/manufacturer_service/domain/
Manufacturer.java`, because every `Validation` and `Construction` contract resolves to the
`"model"` abstract layer:

| Contract | `kind` | `member` | `mandatory` | `required_tests` |
|---|---|---|---|---|
| `ManufacturerNameValidation` | Validation | `name` | `true` | 2 (max/min length) |
| `ManufacturerAddressValidation` | Validation | `address` | `true` | 2 (max/min length) |
| `ManufacturerPhoneNumberValidation` | Validation | `phoneNumber` | `false` | 1 (max length) |
| `ManufacturerEmailValidation` | Validation | `email` | `false` | 1 (max length) |
| `ManufacturerWebsiteValidation` | Validation | `website` | `false` | 1 (max length) |
| `ManufacturerConstruction` | Construction | *(none — whole entity)* | *(n/a)* | 3 (`id`, `createdAt`, `modifiedAt` assignment) |

Ten `required_tests` in total across six contracts, all resolving to one file. This is precisely
the shape Stage 1 exposed: Stage 1 showed the model only the first row and it invented pieces of
the last row (`id`) unprompted.

### 2. What context the model would receive

Everything Stage 1 allowed, unchanged, plus one addition — the full set of contracts sharing this
resolved target, not one:

- All six contracts' `kind`/`entity`/`member`/`mandatory`/`required_tests` (the complete table
  above), so the model can see the *combined* authorized scope of the file it's asked to write.
- The single resolved target path (unchanged from Stage 1 — all six agree on it, which is itself
  part of what's being shown: "these six, together, are this file").
- The tech-stack skill for the `"model"` layer (unchanged, same `skill_for_technology` call).
- Any contract *dependencies* among the six, if present (checked: all six currently have
  `dependencies: []`, so this doesn't arise in this specific run, but the harness should still
  read and render them, exactly as Stage 1 did, in case a future re-generation changes that).

Still explicitly excluded, unchanged from Stage 1 §5: story, full scenario list, `entity_schema`,
ADRs, OpenAPI, exploratory tool access. Only one variable changes between Stage 1 and Stage 2 —
one contract's view vs. all six contracts sharing this file's view — so that a result can be
attributed to that one change, not confounded with a second change at the same time.

The generation shape also changes from Stage 1: instead of one contract → one file, this is
six contracts → one file, in a single call (mirroring how `canopy implement` would eventually
need to compose multiple contracts into one generation step for a shared target, not how Stage 1
tested one contract at a time). A combined test file (covering all ten `required_tests`) is
generated first, then the implementation against it — same Red/Green shape Stage 1 used, just
scaled to the full file's authorized scope instead of one field's.

### 3. Success criteria

Reproducibility-tested the same way as every prior probe in this investigation — at least 3 runs:

1. The generated file contains exactly what the six contracts, combined, authorize: five
   validation-annotated fields (matching each contract's own message-bearing `required_tests`)
   and the three system-generated fields `ManufacturerConstruction` authorizes (`id`, `createdAt`,
   `modifiedAt`, with whatever construction-appropriate annotations/initialization that implies) —
   **and nothing else.** `@Id`/`@GeneratedValue`-style annotations are *correct* here, since a
   contract now explicitly authorizes them — the bar is not "no persistence annotations at all,"
   it's "nothing beyond what these six contracts, together, license."
2. The combined test file's assertions map 1:1 onto the union of all ten `required_tests` — no
   fewer, no more.
3. No field, method, or annotation appears in the implementation with no corresponding line in
   any of the six contracts' `required_tests` (e.g. a `version` field for optimistic locking, a
   `deletedAt` field, a `@Column` naming override never asked for) — checked explicitly, not
   assumed clean by absence of an obvious violation.
4. Reproducible across the 3 runs — not just true once.

### 4. Failure criteria

- **The model still invents a field/annotation with no corresponding contract among the six
  shown.** This would directly falsify the hypothesis: if full visibility of everything
  authorized for this file still isn't enough to stop invention, the cause isn't *incomplete*
  visibility — it's a general tendency to "complete" what looks like an entity class regardless
  of the scope it's given, which is a prompt-strength problem, not a visibility problem.
- **Message-per-field fidelity is still wrong even with all six contracts visible together.**
  Expected to persist regardless of this experiment's outcome — Stage 1 already traced this to a
  Spring Boot skill gap, independent of ownership visibility. If it disappears here too, that's a
  bonus data point, not something to read the hypothesis's validity into.
- **The combined generation introduces cross-field errors that didn't exist per-field** (e.g. a
  validation message from one field's contract leaking onto another field) — would indicate the
  larger, combined prompt itself introduces new confusion, a cost worth weighing against whatever
  ownership-correctness gain it produces.

### 5. What conclusion justifies which fix

| Outcome | Conclusion | Right response |
|---|---|---|
| Invention stops (3/3 clean runs) | Ownership violations were caused by incomplete visibility, not general unruliness. | **Process fix, not a schema change.** `Contract` already carries everything needed — formalize "assemble every contract sharing a resolved target before generating" as a required step in whatever eventually drives Stage 3+ generation (grouping contracts by `resolve_implementation_target` output before calling the model), not a new field. |
| Invention persists despite full visibility | The cause is a strong training prior overriding an explicit scope instruction, not missing information. | **Prompt-strength escalation, still tier 2 (fix the prompt), not tier 3.** Per this project's own escalation order, try a stronger instruction shape next — e.g. an explicit WRONG/CORRECT worked example naming this exact failure — before concluding it's an unfixable compliance limitation. |
| Invention persists even after a stronger prompt is tried and still fails | A real, structural compliance limitation — prompting alone can't hold the boundary. | **Only now does a redesign conversation become warranted** — not a `Contract` schema redesign, but a new *verification* mechanism: a deterministic, post-generation audit that checks a generated file's actual fields/annotations against the union of its owning contracts' `required_tests` and flags anything unauthorized. This matches `docs/design/behavior-first-planning.md`'s own already-anticipated future capability ("contract-to-test/OpenAPI verification") and this project's audit-not-compensation house rule — it would reject or flag, never silently rewrite, the model's output. |

No implementation performed in this scoping pass, per the instruction to design before building.

---

## Stage 2 Implementation and Results (2026-07-14)

**Implementation.** `canopy-llm/examples/contract_driven_stage2_experiment.rs` — same non-
negotiables as Stage 1 (standalone cargo example, no import from or call into `canopy-cli`'s
`implement` command, `plan.rs`, `execute.rs`, or `step.rs`, no change to any existing behavior).
Reused, unmodified: `canopy_storage::load_contracts`, `resolve_implementation_target`,
`abstract_layer_for_kind`, `skill_for_technology` — identical calls to Stage 1's. New and
experimental: `target_for`, which resolves each contract's target and groups every contract
landing on the same path — the one variable this stage changes relative to Stage 1. Confirmed
absent by grep, same as Stage 1: `load_story_spec`, `load_all_adrs`, `load_story_openapi`,
`load_user_stories`, `ToolSpec` — none appear. Build and full workspace test suite confirmed
green and unchanged both before and after adding this file.

Grouping used the same real `manufacturer-001` `contracts.yaml` Stage 1 already generated — the
six contracts identified in §1 above, all resolving to `Manufacturer.java`, no synthetic data.

**Three runs, same reproducibility standard as every prior probe in this investigation:**

| Run | Ownership (8 authorized fields, nothing extra) | Validation-triggering mechanism | `id` assigned at construction? | Boundary tests would actually pass? |
|---|---|---|---|---|
| 1 | **Clean** — exactly `id`, `name`, `address`, `phoneNumber`, `email`, `website`, `createdAt`, `modifiedAt`, nothing else | Manual imperative checks in the constructor (functions correctly) | **No** — `@GeneratedValue` only fires at JPA persist time, not on a plain `new Manufacturer(...)` call; the constructor never assigns `id` itself | 7/7 boundary + 2/3 construction tests pass; the `id` test fails |
| 2 | **Clean** — same 8 fields, nothing else | Declarative annotations only (`@Size`/`@NotBlank`), no manual check, no validator ever invoked — nothing triggers on plain construction | **No** — same `@GeneratedValue`-only gap | 0/7 boundary tests pass (nothing ever throws); 2/3 construction tests pass; `id` test fails |
| 3 | **Clean** — same 8 fields, nothing else | Same as run 2 — declarative-only, nothing fires | **No** — same gap | Same as run 2 |

**Ownership correctness: 3/3 clean — the hypothesis from Stage 1 is confirmed, not just
plausible.** Every run produced exactly the union of what the six contracts authorize and
nothing beyond it. `@Entity`/`@Id`/`@GeneratedValue` appear in all three runs, and correctly so
this time — `ManufacturerConstruction` now explicitly authorizes them, and the model never
reached for anything with no corresponding contract line (no `version` field, no unrelated
methods, no repository-layer content). This directly falsifies the alternative Stage 1 reading
("the model just tends to complete entity-shaped classes regardless of scope") in favor of the
original hypothesis: Stage 1's invention was caused by incomplete visibility, not general
unruliness.

**A second, more severe failure mode emerged, exactly where Stage 1's own analysis predicted one
would persist regardless of ownership visibility.** 2 of 3 runs produced a `Manufacturer` class
whose *only* enforcement mechanism is declarative Bean Validation annotations, with nothing that
ever triggers them on plain construction (`new Manufacturer(...)`) — no manual check, no
`Validator` invocation, no `@Valid` boundary. Every one of that run's 7 boundary tests would
therefore fail: the test expects an exception; the implementation never throws one. This is a
broader version of Stage 1's `@NotBlank`/`@NotNull` message finding, not a new category — both
trace to the same root cause, now visible more clearly: **the Spring Boot skill never explains
how — or whether — Bean Validation actually fires outside a full persistence/`@Valid` context**,
so the model guesses inconsistently: sometimes it adds a working manual check (run 1), twice it
trusted the annotations alone to "just work" (runs 2, 3) and they don't.

**A third, fully reproducible (3/3) defect: `id` is never actually assigned at construction
time.** Every run relied solely on `@GeneratedValue`, which only fires when JPA persists the
entity through a real `EntityManager` — never on a plain `new Manufacturer(...)` call used in a
unit test. `testManufacturerConstructionAssignsId` would fail in all three runs. This is the
JVM-side analogue of a convention the Node/Express skill already states explicitly ("factory
assigns id via `randomUUID()`" — eager, at construction) that Spring Boot's skill has no
equivalent for.

### Contract sufficiency assessment

Per §5's decision table: **outcome matches the first row — invention stopped (3/3 clean runs).**
Conclusion: ownership violations were caused by incomplete visibility, not general unruliness.
**Right response: a process fix, not a schema change** — `Contract` already carries everything
needed (`kind`/`entity` are exactly what grouping-by-target required); what's missing is an
assembly step in whatever eventually drives contract-driven generation, grouping contracts by
`resolve_implementation_target` output before calling the model, not a new field.

The other two findings (validation-triggering mechanism, `id`-assignment timing) are, exactly as
predicted in Stage 2's own design ("expected to persist regardless of this experiment's
outcome"), unaffected by ownership visibility — both are Spring Boot skill completeness gaps,
independent of the contract-driven hypothesis, and would improve generation today whether or not
contracts are involved at all.

### Recommendation

1. **The ownership-visibility hypothesis is confirmed. Formalize it as a process requirement**
   for any future contract-driven generation step: before generating a file, assemble every
   contract whose `resolve_implementation_target` output matches, not one contract at a time.
   No `Contract` schema change follows from this — the mechanism (`resolve_implementation_target`
   grouped by output) already exists; only the *calling convention* around it needs to change,
   and only once wiring into `canopy implement` is actually undertaken (still not this stage's
   job).
2. **Two concrete, separately-scoped Spring Boot skill fixes are now justified by reproducible
   evidence**, independent of contracts: (a) document how Bean Validation actually gets triggered
   in a plain unit test — either teach a manual-check convention (mirroring what Node/Express's
   skill already does) or teach the correct `Validator`/`@Valid` invocation pattern, but pick one
   and state it explicitly rather than leaving the model to guess; (b) document that `id`
   assignment for a not-yet-persisted aggregate needs an eager, construction-time value (e.g. a
   manually generated `UUID`), the same convention the Node/Express skill already states for its
   own stack.

   **Done (2026-07-14).** `spring_boot_skill` (`canopy-llm/src/skills/tech_stack.rs`) migrated to
   the layer-partitioned shape and gained a `"domain"`-layer rule for both: imperative
   constructor validation is now required alongside (not instead of) Bean Validation annotations,
   and `id` must be assigned eagerly via a manually-generated value, mirroring Node/Express's own
   `randomUUID()` convention. Landed a real, independent bug along the way: `detect_layer()`
   (`canopy-llm/src/skills/mod.rs`) had no recognition of any JVM singular package directory
   (`/domain/`, `/repository/`, `/dto/`, `/service/`, `/controller/`) at all — every real
   Green-phase generation call for a Spring Boot file fell through to the generic `"module"`
   fallback, silently different from what Red-phase's own separate Java layer closure computed
   for the same file. Two earlier keying attempts for this fix were caught and corrected across
   two prompt-review rounds before landing (see the commit history) — first inert (keyed under a
   string neither call site produces), then over-broad (a `"module"` key would have leaked into
   every JVM layer, not just domain files, given `detect_layer()`'s blind spot). Fixing
   `detect_layer()` itself, not working around it, is what closed this correctly. 5 new unit
   tests (2 in `tech_stack.rs`, plus `detect_layer_recognizes_jvm_singular_directories`).
3. **No redesign conversation is warranted.** Both remaining failure modes are nameable and
   traced to a specific, addressable skill gap — the stop condition for escalating to a
   verification-mechanism/redesign discussion (row 3 of §5's table) was not reached.

---

## Design Decision: Group Contracts by Resolved Target Before Generating

Status: **Decided** (2026-07-14), formalizing Stage 2's outcome. Not yet implemented in any
production code path — `canopy implement` still doesn't consume contracts at all (Stage 4 of the
migration plan, §6). This decision governs how a future contract-driven generation step must
behave once that wiring is undertaken; it does not change anything today.

**Decision:** whenever a future implementation step generates a file from `contracts.yaml`, it
must first assemble *every* contract whose `resolve_implementation_target` output resolves to
that same file, and generate from the complete set — never from one contract in isolation once
more than one targets the same place.

**Rationale.** Stage 1 showed that generating from a single contract, even with an explicit
scope-limiting instruction, produces unauthorized invention 2 of 3 runs (`@Entity`/`@Id`/
`@GeneratedValue` with no supporting contract). Stage 2 changed exactly one variable — full
visibility of every contract sharing the target — and eliminated it, 3 of 3 runs, with no other
input changed. See [[implementation-ownership-requires-full-file-scope-visibility]] for the
generalized principle this decision instantiates.

**What this does and doesn't require.**
- Does **not** require a `Contract` schema change. `kind` and `entity` (Option 2, already landed)
  are exactly what `resolve_implementation_target` needs to group contracts by output — the
  grouping is a query over data `Contract` already carries, not a new field.
- Does **not** require deciding *how* to compose the model call yet (one combined prompt, as
  Stage 2's example did; or an incremental accretion where later contracts see the file already
  written by earlier ones — both are open implementation questions for whoever builds Stage 3/4,
  not resolved by this decision).
- **Does** require that whatever eventually drives Stage 3/4 generation group contracts by
  resolved target as a precondition — a contract must never be handed to a generation call alone
  when others share its file. This is a real constraint on the *calling convention*, not on the
  contract data model.

**Scope boundary, honestly stated:** this decision rests on one entity, one file, six contracts,
one story (per the principle's own Confidence Assessment, rated `medium`, not `high`). It should
be treated as the working assumption for Stage 3/4's design, not as closed the way Option 2's
schema change is — a second confirming case (a different entity, a different-sized contract
group) would move it from a working decision to a validated one.
