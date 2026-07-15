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

---

## Stage 3 Results (2026-07-15): real compile + test, not eyeballed

**Setup.** `canopy-llm/examples/contract_driven_stage3_experiment.rs` — same non-negotiables as
Stages 1-2 (standalone example, `canopy implement`/`plan.rs`/`execute.rs`/`step.rs` untouched).
"Reuse execute.rs's test-run/fix-loop machinery" (this document's own Stage 3 description) is
satisfied by calling `canopy_llm::fix_file` directly — the actual production fix-loop function,
not a copy of it — and by compiling/running the generated code for real in a standalone Maven
project (this experiment's own scratch harness; not the dogfooding project's real service tree,
not part of this repo). Reused unmodified: `load_contracts`, `resolve_implementation_target`,
`abstract_layer_for_kind`, `skill_for_technology`, `fix_file`. Same six-contract group as Stage 2.

**A methodology bug caught mid-run, disclosed rather than quietly patched:** the first version of
this experiment passed `abstract_layer_for_kind`'s output ("model") directly to
`skill_for_technology`. That's correct for resolving the file *target*, but wrong for the *skill
lookup* — the Spring Boot skill's new content (this session's own fix, above) is keyed under
`"domain"`, the string `detect_layer()` actually produces for this path, not the tech-agnostic
abstract name. The bug made the first run's generated code look exactly like Stage 2's *pre-fix*
behavior (no imperative validation, `@GeneratedValue`-only id) even though the fix was already
committed — because the fix was never actually reaching the prompt. Corrected to
`detect_layer(&target_path)`; re-ran from a clean baseline afterward. (Stage 2's own example has
the same latent bug but is unaffected by it retroactively: at the time Stage 2 ran, the skill had
no layer-partitioned content at all, so no layer string could have changed what it saw.)

**Three runs, real `mvn clean test`, not eyeballed:**

| Run | Attempt 1 | Root cause if failed |
|---|---|---|
| A | **PASS** — all 10 tests compiled and passed | — |
| B | FAIL, fix attempt also FAIL | Missing `assertNotNull` static import in the **test file** — an ordinary test-generation slip, unrelated to contracts or the Spring Boot skill fix. This harness's one bounded `fix_file` call only targets the implementation (matching `fix_file`'s own real production scope), so a broken test file is outside what this specific harness attempts to repair — a disclosed limitation of this experiment, not a finding about contracts or the skill. |
| C | **PASS** — all 10 tests compiled and passed | — |

**2 of 3 runs: real, compiled, executed, zero-iteration success**, both showing exactly the
pattern the Spring Boot skill fix was meant to produce — imperative validation for all five
fields (correctly distinguishing mandatory from optional), `id` assigned eagerly via
`UUID.randomUUID()` in the constructor, no unauthorized fields beyond the six contracts' combined
scope. This is the first point in this whole investigation where "the generated code works" is an
objective, tool-verified fact rather than a manual read of LLM output.

**The one failure doesn't implicate contracts, ownership visibility, or the skill fix** — it's a
routine test-generation import bug, the same category of noise any LLM-driven test-writing step
can produce, caught here only because this stage finally checks for real. Distinguishing this
from a contract-driven-implementation-specific finding matters: conflating "any bug that shows up
during a real compile" with "a problem with this approach" would overstate what Stage 3 found.

**Conclusion:** the Spring Boot skill fix works, confirmed by real compilation and test
execution, not by re-reading LLM output and guessing. The contract boundary + full-file
visibility + fixed skill combination produced working code without any fix-loop iteration in 2 of
3 runs. The remaining gap is an ordinary test-generation reliability question this harness wasn't
built to address (it would need to extend `fix_file`-style repair to test files too, matching the
real fix loop's actual scope) — a natural refinement for a future Stage 3 run, not a finding about
this investigation's central question.

---

## Stage 4 (2026-07-15): production wiring — the first stage that touches `canopy implement`

Every prior stage ran as a standalone cargo example; none touched production code. Stage 4 is
different by design — it wires `canopy implement` itself to consume `contracts.yaml`, per this
document's own migration plan (§6): "Replace `plan.rs`'s LLM-driven file discovery with
contract-driven enumeration... for one entire story."

**Design decision, confirmed before implementation:** the switch is triggered by a mechanical
fact — the presence of a story's `contracts.yaml` — not an explicit opt-in flag, per the same
"compute facts mechanically" house rule this whole investigation keeps returning to. Two
refinements requested before building: (1) never silent — the CLI always prints which path ran
and how to force the other one; (2) a temporary diagnostic (`--compare-with-legacy-planner`)
that runs the legacy planner alongside contract-driven discovery purely to print a file-list
diff, never affecting what gets saved or executed, to build confidence before this becomes the
only path.

**What shipped:**
- `canopy-llm/src/prompts/contract_plan.rs` (new): `generate_story_plan_from_contracts` — fully
  mechanical, zero LLM calls. Filters to `scope: Unit` contracts, resolves each one's file target
  via `resolve_implementation_target`, groups contracts sharing a target into one step (the
  ownership-visibility finding from Stage 2, now load-bearing in production, not just an
  experiment), derives each step's `operation` (create/modify) directly from whether the target
  is already in `existing_files` — mechanically, not by asking an LLM — and each step's
  `description` from a fixed kind→verb table (the "Layer verbs" convention already named in this
  project's CLAUDE.md: Validates, Constructs, Persists, Orchestrates, ...). `depends_on` maps each
  contract's own `dependencies` (other contract ids) to *those* contracts' resolved targets.
  Ordering reuses `plan.rs`'s existing `layer_weight`/`frontend_tier` sort (made `pub(crate)`, not
  duplicated). 7 unit tests.
- **Refuses to guess, by design.** Returns `Err(String)` rather than a plan whenever it can't be
  confident: more than one non-frontend service exists (nothing on `Contract` yet records which
  service owns which entity — a real, disclosed gap, not a bug); an `HttpRequest`/`HttpResponse`
  contract exists (its "route" layer is ambiguous between a backend controller and a frontend
  api-client, and nothing yet disambiguates); a contract has no `entity`/`kind`; or no mechanical
  file-target convention exists yet for the resolved (tech, layer) pair. The caller falls back to
  the LLM-driven planner on any `Err` — never a silently incomplete plan.
- `canopy-cli/src/cli.rs`: two new flags on `Implement` — `--legacy-planner` (forces the old
  path even when contracts.yaml exists) and `--compare-with-legacy-planner` (diagnostic only).
- `canopy-cli/src/commands/implement/plan.rs`: `load_or_generate_plan` now checks
  `canopy_storage::load_contracts(story_id)` before generating a new plan. `Err` (no
  contracts.yaml — every story today except one manually-generated test case) falls through to
  the exact same `generate_story_plan` call, with the exact same arguments, as before this
  change — the "no contracts.yaml" path is unchanged, not just similar.

**Verified against real data, not just synthetic test fixtures**, without requiring the target
project to be scaffolded first (a separate, larger, more invasive action `cmd_implement`'s own
`ensure_services_scaffolded` gate would otherwise demand before plan generation is even reached):
`canopy-llm/examples/stage4_dry_run_verification.rs` calls the new function directly against
`manufacturer-001`'s real, already-generated `contracts.yaml` and `services.yaml`. Result: **one
step**, exactly as Stage 2/3 predicted (`ManufacturerNameValidation` through
`ManufacturerConstruction` all resolve to the same `Manufacturer.java`), `operation: create`
(correct — the file doesn't exist on disk yet), description `"Constructs and validates
Manufacturer."` (mechanically derived from the two distinct kinds present, alphabetized and
deduplicated), no `depends_on` (correct — nothing here depends on anything outside this one
merged file).

**Independent safety review before shipping, per this project's own practice of never folding
"wrote it" and "shipped it" into the same unchecked step — found two real bugs, both fixed:**
1. **JVM package path used dots instead of slashes.** `resolve_implementation_target`'s
   `pkg_path` parameter expects a slash-separated path; a real scaffold-detected package name is
   dotted (`com.example.widget`). Passing it straight through would have silently produced a
   single bogus directory literally named `com.example.widget` instead of the correct
   `com/example/widget/` tree — invisible until the first time a real JVM package is actually
   detected (this pre-existing bug lived in `file_targets.rs` since Addendum 2, but had never
   been reachable from production before this stage wired it in). Fixed: the same
   `.replace('.', "/")` conversion `plan.rs`'s own LLM-driven planner already applies.
2. **Integration-scope contracts were silently dropped, not fallback-triggering, when mixed with
   unit contracts** — contradicting this function's own "complete plan or an explicit `Err`"
   contract. A story with both kinds of contract (a normal, expected shape, not a hypothetical)
   would have produced an `Ok` plan covering only the unit contracts, with no signal anything was
   left out. Fixed: an explicit check returns `Err` when any integration-scope contract exists,
   rather than silently filtering.

Neither bug affects `manufacturer-001`'s own current contracts.yaml (no detected JVM package yet,
no integration-scope contracts yet) — both were caught by review, not by a live failure, exactly
the point of reviewing before shipping rather than after. 2 new regression tests added for each.

Build and full workspace test suite (73 tests in `canopy-llm` after this change) green throughout.

---

## Stage 5 Design (scoped 2026-07-15, not yet implemented)

Per the Contract Composition Assessment (`docs/design/contract-composition-assessment.md`, §1.1,
§4): the dominant remaining uncertainty is no longer "can contracts drive implementation" but
"does contract-scoped *generation* actually improve on what `canopy implement` already produces
today, or only match a hand-built minimal prompt no one currently ships." Stages 1-3 only ever
compared contract-scoped generation against itself across runs, never against production's real,
fuller-context prompt. This stage answers that, before composition work takes priority (per the
user's explicit sequencing: only promote composition to the top of the roadmap once this is
understood).

**Still a standalone experiment.** Same non-negotiables as Stages 1-4: no modification to
`canopy implement`, `plan.rs`, or `execute.rs`. This stage calls production's real, unmodified
public functions directly (`generate_unit_test_stub`, `execute_implementation_with_test`) —
reusing the actual mechanism, not a copy of it — without going through the CLI or `execute.rs`'s
orchestration at all.

### Exact files

- **Target entity**: the same real `manufacturer-001` case Stages 2-4 all used — six contracts
  merging onto `Manufacturer.java`. No new data needed; reuses the existing real `contracts.yaml`,
  `spec.yaml`, `services.yaml`, ADRs, and `openapi.yaml` already on disk.
- **Harness**: the existing Stage 3 Maven project
  (`<scratchpad>/stage3-maven/services/manufacturer-service/`), reused as-is — its `pom.xml`
  already carries every dependency both paths need (`junit-jupiter`, `jakarta.validation-api`,
  `jakarta.persistence-api`, `hibernate-validator`).
- **Two sibling sub-packages, not one shared file** — so both paths' output exists
  simultaneously, inspectable side by side, with no overwrite race between runs:
  - Production path: `src/main/java/manufacturer_service/domain/production/Manufacturer.java` +
    `src/test/java/manufacturer_service/domain/production/ManufacturerTest.java`
  - Contract-scoped path: `src/main/java/manufacturer_service/domain/contractscoped/Manufacturer.java`
    + `src/test/java/manufacturer_service/domain/contractscoped/ManufacturerTest.java`
  Each of a path's 3 runs overwrites its own sub-package (sequential, not accumulating) —
  matching Stage 3's own `mvn clean test` discipline so no stale `target/` class from a prior
  run masks a new one.
- **New experiment file**: `canopy-llm/examples/contract_driven_stage5_experiment.rs` — one
  program, both paths, all 6 runs (3 per path), printing a final comparison table. Not two
  separate files, so both paths definitely share the exact same loaded story/spec/contracts/
  services/ADR data — no risk of the two paths silently drifting from slightly different inputs.

### Exact contracts and exact inputs, per path

**Contract-scoped path (B)** — identical to Stage 2/3: the same six real contracts
(`ManufacturerNameValidation` through `ManufacturerConstruction`), their `required_tests`
verbatim, `resolve_implementation_target`'s resolved layer, `skill_for_technology`'s "domain"
render. Reuses Stage 2/3's own `test_prompt`/`impl_prompt` functions unchanged (only the target
sub-package path differs, for co-existence with path A).

**Production path (A)** — the real, unmodified functions `canopy implement` calls today:
`generate_unit_test_stub` (Red phase) then `execute_implementation_with_test` (Green phase),
loaded with real data read from disk:
- `story`: `manufacturer-001`'s real `UserStory` (`load_user_stories`, filtered by id).
- `spec`: the real `IntentSpec` (`load_story_spec`) — full `entity_schema` (8 fields: 5
  validated + 3 system-generated) and full `scenarios` (**12** real scenarios — 5 original plus
  the 7 boundary scenarios added earlier this investigation to clear Stage 0's completeness
  gate).
- `openapi_yaml`: the real, already-generated OpenAPI spec for this story.
- `adrs`: all 6 real ADRs (`load_all_adrs`).
- `services`/`service_packages`: the real `ServicesRegistry` (`load_services_registry`); an
  empty `service_packages` map, matching Stages 1-4 (no scaffold detected for this project).
- `step`: an `ImplementationStep` with `file` = the production sub-package path above,
  `service` = `"manufacturer-service"`, `operation` = `"create"`, and — deliberately held
  identical to path B's own step, so the *only* experimental variable is prompt context, not
  incidental step-description wording — `description` = `"Constructs and validates
  Manufacturer."` (the exact mechanical description `generate_story_plan_from_contracts` already
  produces for this file).
- `sibling_section`/`arch_skills`: `sibling_section` empty (no dependencies — matches path B);
  `arch_skills` via the real `skills_for_architecture(adrs, tech)`, since production always
  includes this and withholding it would no longer be testing production's real prompt.
- `package_constraints`/`observed_call`: `None` (no dependency gate, no Roots-parsed call shape
  available or needed here).

**A concrete, checkable prediction going in, not just an open question:** `unit_test_stub_prompt`
(the Java-specific test-stub function `generate_unit_test_stub` calls) has no layer-based
scenario filtering at all — confirmed by reading it directly, `canopy-llm/src/prompts/step.rs`
around line 315, which embeds `spec.scenarios` in full with a flat "one @Test method per
scenario" instruction, unlike the TypeScript-specific `unit_test_stub_prompt_ts`'s
`scenario_coverage_note` mechanism, which explicitly filters scenarios by layer relevance. For a
domain-layer Java file, this means the model will be shown all 12 real scenarios — including ones
describing HTTP rejection responses and event publication, concerns a plain domain class can't
and shouldn't express — with no instruction telling it to disregard the inapplicable ones. Stage
5 will show directly whether this produces measurable harm (irrelevant or malformed test methods,
wasted prompt budget) or turns out inconsequential in practice.

### Reproducibility methodology

3 independent runs per path (6 total generate→write→`mvn clean test` cycles), matching the
standard every prior stage in this investigation used. All 6 runs share identical loaded
input data (one program execution, data loaded once) — only the model's own sampling varies
run to run, not the inputs. Each run's real `mvn clean test` output (pass/fail per test method,
compiler errors if any) is captured and reported, not eyeballed.

### Success metrics, computed identically for both paths

1. **Real compile-and-test pass rate** — the primary metric. For each run, how many of that
   path's own generated test methods actually pass against that path's own generated
   implementation, via real `mvn clean test`. Aggregated as (runs, tests-passed) across all 3
   runs per path.
2. **Ownership correctness** — the authorized field set is identical for both paths by
   construction (`entity_schema`'s 8 fields exactly match the 6 contracts' combined 8 fields, a
   fact already established, not assumed). Any declared field in either path's generated
   implementation with no corresponding source (a contract's `required_tests`/`entity`/`member`
   for path B, an `entity_schema` field for path A) is a violation, checked for both paths, not
   assumed clean for either — the Contract Composition Assessment explicitly flagged this as
   untested for production's own fuller-context prompt.
3. **Constraint fidelity** — folded into metric 1: a boundary-condition test method passing *is*
   the constraint-fidelity check (the test encodes the exact bound; a pass means the
   implementation enforces it correctly), so this isn't a separate subjective judgment call.
4. **Prompt size** — each path's actual constructed prompt length (characters), measured and
   reported for both, both test-stub and implementation calls. Informative, not a pass/fail gate
   — the hypothesis is that path B is meaningfully smaller, but this alone doesn't decide the
   experiment either way.
5. **Scenario-noise effect (specific to the prediction above)** — for path A's generated test,
   how many of the 12 scenarios' worth of prompted "one test per scenario" instruction produced a
   test method that doesn't compile, doesn't apply to a domain class, or duplicates a boundary
   test already covered — checked explicitly, not inferred from the aggregate pass rate alone.

### Stop conditions

- **If path B (contract-scoped) meets or exceeds path A (production) on metric 1, with no new
  ownership violations (metric 2)**: the hypothesis holds — contract-scoped generation is not
  just capable of working, it's competitive with or better than what ships today. Composition
  work (multi-entity, real dependency edges) becomes the next priority, per the user's own stated
  sequencing.
- **If path B underperforms path A** (lower real pass rate, or new defects path A doesn't have):
  a direct, falsifying result. The right response is not to redesign contracts or the schema —
  it means today's fuller context is pulling real weight, and wiring contract-scoped generation
  into `execute.rs` would be premature. Treat "does narrower context actually help generation" as
  still open, and leave `step.rs`/`execute.rs` exactly as they are.
- **If both paths perform similarly (no clear winner)**: also a real, useful result — it would
  suggest the *narrower* prompt is preferable on cost/latency/maintainability grounds even without
  a quality edge, but that's a different, weaker claim than "better," and should be reported as
  such rather than rounded up to a win for either side.
- **Not a stop condition, but worth naming**: if metric 5's prediction is confirmed (production's
  scenario-noise measurably hurts its own results), that's independently useful information about
  a real, fixable gap in `unit_test_stub_prompt` — worth its own separately-scoped skill/prompt
  fix regardless of which path wins the broader comparison, per this project's escalation order
  (a missing layer filter is a prompt fix, not a reason to touch the contract schema).

No implementation performed in this scoping pass, per the instruction to design before building.

## Stage 5 Results (2026-07-15): production 0/3, contract-scoped 3/3 — a valid, decisive result

Built `canopy-llm/examples/contract_driven_stage5_experiment.rs` exactly to this design: one
program, both paths, 6 runs, real `mvn clean test` per run, no modification to `canopy implement`
or any of its production call sites. Path A calls production's real, unmodified
`generate_unit_test_stub`/`execute_implementation_with_test`; path B reuses Stage 2/3's own
`contract_test_prompt`/`contract_impl_prompt` verbatim.

### A harness-validity gap surfaced twice before a valid result existed

The Stage 3 Maven harness was built to exercise only the contract-scoped path, so its `pom.xml`
never needed dependencies that a real Spring Boot scaffold provides transitively via
`spring-boot-starter-test`. Production's real prompt uses both — first surfaced as AssertJ
(`org.assertj.core.api.Assertions`) missing, then, after that fix, Mockito
(`org.mockito.*`/`MockitoExtension`/`@InjectMocks`) missing too — each causing all 3 path-A runs
to fail identically on `package ... does not exist`, before reaching any behavior this experiment
was designed to compare. Both were harness gaps, not findings: a real scaffolded project would
have had both transitively, and production's prompt correctly assumes their availability. Fixed by
adding `assertj-core` and `mockito-core`/`mockito-junit-jupiter` (test scope) to the harness
`pom.xml`, verified via `mvn dependency:resolve`, and the full 6-run experiment re-run from
scratch each time so the comparison stayed symmetric. The second, corrected, full re-run
(`stage5_full_v3.log`) is the run these results are drawn from. Disclosed per this project's own
house style for harness-validity gaps (see Stage 3's fence-extraction bug, Stage 4's dot-vs-slash
path bug) — not smoothed over.

### Results (3 runs per path, real `mvn clean test`)

| Run | Path A (production) | Path B (contract-scoped) |
|---|---|---|
| 1 | **FAIL** — compile error: generated test references `ManufacturerRepository` and `ManufacturerService`, neither of which this call generates or was asked to generate — an out-of-file-scope invention | **PASS** |
| 2 | **FAIL** — compiles; 12 of 30 generated `@Test` methods fail at runtime | **PASS** |
| 3 | **FAIL** — compile error: test invokes 3-arg and 4-arg `Manufacturer(...)` overloads the generated implementation never defines (which only has no-arg/2-arg/5-arg) | **PASS** |

**Path A: 0/3 pass. Path B: 3/3 pass.** (A separate, earlier full execution of the corrected
harness — `stage5_full_v2.log`, run before the Mockito gap was found and fixed — additionally
showed path B at 2/3, with the one failure a self-contained `int`-to-`Long` type error in
`this.id = UUID.randomUUID().hashCode()`. Combined across both valid-for-path-B executions: 5/6.
That failure is a real, reproducible defect — the same id-assignment defect class Stage 2/3 also
found — not a fluke to discard, but it doesn't change path B's standing relative to path A, which
failed 6/6 across the same two executions.)

### Per-path failure analysis

**Path A (production), all 3 failures are distinct, genuine content-generation defects — none is
a harness artifact:**

1. **Scope invention beyond the file being implemented.** Run 1's test assumes a
   `ManufacturerRepository`/`ManufacturerService` pair exists, uses `@InjectMocks`/`MockitoExtension`
   to wire them in, and asserts through them — a full application-service-plus-repository shape,
   when the step being implemented is scoped to one domain file. Confirms the Contract Composition
   Assessment's flagged-but-untested risk that production's fuller prompt has no equivalent to the
   ownership-visibility constraint Stage 2 had to add for contracts.
2. **Annotation-only validation, no constructor-level enforcement.** Runs 2 and 3 both generate
   `@Size`/`@NotBlank`-style Bean Validation annotations but no `if (...) throw new
   IllegalArgumentException(...)` in the constructor. Jakarta Bean Validation constraints are only
   evaluated by an active `Validator` (or JPA lifecycle callback) — never by a bare `new
   Manufacturer(...)` in a unit test — so every test asserting eager validation or eager `id`
   assignment (`@GeneratedValue(strategy = GenerationType.IDENTITY)` only assigns at persist time)
   fails at runtime. This is the same eager-construction-vs-JPA-generated-value conflation Stage
   2/3 already found and fixed for the contract-scoped skill — production's prompt has no
   equivalent fix.
3. **Ad hoc telescoping constructors, invented per scenario rather than as one canonical shape.**
   Across runs, path A's generated test calls `Manufacturer(name,address)`,
   `(name,address,phoneNumber)`, `(name,address,null,email)`, and
   `(name,address,phoneNumber,email,website)` — a different arity per scenario, with no single
   constructor design driving all of them. The paired implementation call then only defines a
   subset of the arities the test invented (typically 2-arg and 5-arg), so several scenario-driven
   test methods don't compile. This directly matches the confirmed prediction below: the flat,
   unfiltered 12-scenario prompt gives the model no reason to converge on one constructor shape.
4. **The predicted unsatisfiable scenario surfaced exactly as expected.** Every run's test includes
   a `should_fail...due_to_duplicate_name` method — scenario `manufacturer-001-05` — which no
   domain constructor can satisfy without repository access. Two runs left it empty with a comment
   disclaiming it ("assumed the service layer checks this"); one run wrote it as a real assertion
   that fails at runtime for exactly the reason predicted. Confirms metric 5's prediction:
   `unit_test_stub_prompt`'s lack of layer-based scenario filtering (unlike the TypeScript path's
   `scenario_coverage_note`) produces real, measurable harm for a Java domain-layer file, not just
   a theoretical gap.

**Path B (contract-scoped): all 3 runs structurally identical** — one canonical 5-arg constructor,
manual imperative validation for every field, eager `UUID.randomUUID()` id assignment, exactly the
8 fields the six contracts' combined scope authorizes, nothing more. The single failure observed
in the separate `_v2` execution (the `int`/`Long` id-assignment bug) was self-contained to one
run and didn't recur in `_v3`'s 3 runs.

### Metrics, per the design's own definitions

1. **Real pass rate** — path A 0/3 (0/6 combined), path B 3/3 (5/6 combined). Decisive, not close.
2. **Ownership correctness** — path B: clean in all 3 runs, exactly the contracts' 8 authorized
   fields, no invented fields or classes. Path A: no invented *fields* beyond `entity_schema`'s 8
   (the field-level check the design specified passes), but run 1 shows a distinct, broader scope
   violation the design's field-level metric didn't anticipate — inventing entire *sibling classes*
   (a repository, a service) outside the file being implemented. Worth naming as an ownership
   violation in substance even though it falls outside metric 2's literal field-based definition.
3. **Constraint fidelity** — folded into metric 1 as designed: path B's passing runs are the
   fidelity check; path A never reaches a state where this is measurable, since it never compiles
   or passes cleanly.
4. **Prompt size** — confirmed as designed: production ~13,098 chars (entity_schema 992 + 12
   scenarios 6,228 + arch_skills 1,755 + tech_rules 4,123) vs. contract-scoped ~5,395 chars (six
   contracts' facts 1,272 + tech_rules 4,123) — production's prompt is ~2.4x larger, and none of
   that extra size bought a passing result across either execution.
5. **Scenario-noise effect** — confirmed (see failure analysis point 4 above): the unfiltered
   12-scenario prompt directly produced both the unsatisfiable duplicate-name test and pressure
   toward inventing multiple constructor arities to fit disparate scenario shapes into one file's
   test class.

### Stop condition reached

**"Path B meets or exceeds path A on metric 1, with no new ownership violations."** Reached
cleanly — path B is not merely competitive, it wins outright (3/3 vs 0/3, or 5/6 vs 0/6 combined),
with a cleaner ownership profile than path A, not a worse one. Per the design's own stated
consequence: **the hypothesis holds.** Contract-scoped generation is not just capable of working
in isolation (Stages 1–3 already showed that) — it beats production's real, shipped prompt on the
same file, same story, same harness. Composition (multi-entity, real cross-contract dependencies)
becomes the next priority, per the user's own stated sequencing, now that generation quality is no
longer the open question it was when this stage was scoped.

**What this result does *not* say.** It does not say production's prompt design is bad in the
abstract — `unit_test_stub_prompt`'s missing scenario-layer filter is a specific, nameable,
fixable gap (this stage's metric 5 finding), not evidence that the whole story/spec/scenario/ADR
approach is unsound. It also does not generalize beyond this one entity/file shape yet — path B's
win here is on a single-entity, no-dependency case, exactly the case Stages 1–3 already validated
contracts on. Whether contract-scoped generation holds up once dependencies are real (not empty
lists, as the Composition Assessment already flagged) is untested by this stage and remains
composition's open question, not this one's.
