# Contract Readiness Assessment

Status: validation exercise — no implementation performed. Answers the question "are contracts
actually sufficient implementation inputs?" before any work goes into wiring `canopy implement`
to consume `contracts.yaml`.

Date: 2026-07-14

Reviewed: `docs/design/behavior-first-planning.md`, `docs/narratives/the-road-to-contracts.md`,
`canopy-core/src/lib.rs` (`Contract`/`Behavior`/`ClusteringResult` and related types),
`canopy-llm/src/prompts/behaviors.rs` (Stages 0-1), `canopy-llm/src/prompts/clustering.rs`
(Stage 3), `canopy-llm/src/prompts/contracts.rs` (Stage 4), `canopy-llm/src/prompts/plan.rs` and
`step.rs` (today's live `canopy implement` prompts), `canopy-llm/src/skills/tech_stack.rs`
(skill architecture), and the dogfooding project's real `manufacturer-001` `spec.yaml` and
`adr-006`.

**Evidence-availability caveat:** the design doc's own live-verification notes describe a
`product-001` contract-generation run (9 contracts, zero audit findings). That project's
`.canopy/` state is no longer present on disk — the only story with a real `spec.yaml` in the
current dogfooding project is `manufacturer-001`, which has never been run through Stages 1-4
(no `behaviors.yaml`/`clusters.yaml`/`contracts.yaml` exist for it). Rather than re-run the
pipeline live (a multi-LLM-call, interactively-gated operation, arguably closer to "building"
than "validating"), the worked example below hand-traces the *mechanical* portions of Stages 1,
3, and 4 against `manufacturer-001`'s real schema — those functions are deterministic Rust, not
LLM output, so the trace is exact, not a guess. This is disclosed rather than glossed over: it
means Q3's example is real data run through real code by hand, not a live pipeline execution.

---

## Q1 — What does a generated contract currently contain?

From `canopy-core/src/lib.rs:457-487`, a `Contract` is exactly:

| Field | Content |
|---|---|
| `id` | mechanical, e.g. `manufacturer-001-contract-001` |
| `name` | mechanical: `{subject}{PascalCase(kind)}` (unit) or `{subject}Workflow` (integration) |
| `scope` | `Unit` or `Integration` |
| `source_cluster` | the one cluster/grouping id it was generated from |
| `owned_behaviors` | behavior **ids** — no inline content |
| `required_tests` | the owned behaviors' own `statement` strings, verbatim (a redundant *view* of `owned_behaviors`, not new data) |
| `dependencies` | other contract **ids** |
| `derivation` | `Mechanical` or `Reviewed` |

That's the entire artifact. There is **no** `inputs`, `outputs`, `constraints`, or
`responsibilities` field, and no `forbidden_imports` or `implementation_target` — and this isn't
an oversight to discover: the design doc says so itself, in the Stage 4 section:

> "Two deliberate simplifications versus the sketch above: no `forbidden_imports`/
> `implementation_target` fields yet — those are file-generation specifics that likely belong to
> a later scaffolding step once `canopy implement` is wired to consume contracts, not to this
> abstract artifact." (`docs/design/behavior-first-planning.md`)

So the *content* a contract carries is: an id/name/scope tuple, a list of one-line prose
behavior statements, and a list of other contract ids it depends on. Everything else a
consumer might want — target file path, layer, field types, exact code-identifier names — has
to come from somewhere else, today, if at all.

One nuance worth being precise about: the prose statements are **not** vague. Because
`mechanical_validation_behaviors` (`behaviors.rs:280-375`) interpolates the real threshold value
into the statement text (`"{} longer than {n} characters is rejected."`), a required_test like
`"Name longer than 200 characters is rejected."` does carry the exact numeric constraint — just
as English, not as a typed field. What's *not* carried: the field's code identifier in its
original case (`name` vs. the capitalized `Name` used in prose), its declared type (`string`),
or which entity owns it beyond what can be parsed back out of the PascalCase contract `name`.

## Q2 — Gap analysis: what does `canopy implement` need that contracts don't provide?

Reading `canopy-llm/src/prompts/plan.rs` and `step.rs` (today's live prompts — confirmed these,
not `contracts.rs`, are what `canopy implement` actually calls) shows the *current* pipeline
uses, per file:

- The full story narrative (`as_a`/`want`/`so_that`)
- `spec.entity_schema` (full YAML, gated to the "model" layer)
- `spec.scenarios` (full BDD scenario YAML — one `@Test`/`it()` per scenario, not per behavior)
- The OpenAPI spec (gated to "route"/"api-client" layers)
- ADR summaries → an "architecture skill" rendered as prose
- A tech-stack skill scoped to the file's detected layer (`detect_layer(&step.file)`)
- Roots-derived sibling symbol surfaces, and current file content for `modify` ops
- A directory-prefix rule, an existing-files list, an installed-packages list
- For test generation specifically: a large set of layer-conditional prose rules
  (`scenario_coverage_note`, `boundary_rule`, `missing_field_exception_rule`,
  `mock_dependencies_rule` in `step.rs`) that exist entirely to tell the model *which slice of
  the shared scenario list applies to this one file*.

None of this is Contract- or Behavior-shaped. `canopy implement` today has no code path that
reads `ContractSet`, `Contract`, or `Behavior` at all — grepping `canopy-llm/src/prompts/plan.rs`
and `step.rs` for any of those types returns nothing. This matches the narrative's own framing
(`docs/narratives/the-road-to-contracts.md`): the wiring is described as unbuilt, and this
confirms it by absence, not just by the narrative's own claim.

Concretely, what a contract is missing to replace the current inputs above:

1. **A file path / implementation target.** Nothing on `Contract` says which file(s) realize
   it. Today file discovery happens in `plan.rs`'s LLM-driven "discover every file" step, working
   from `entity_schema`+`scenarios`, not from contracts.
2. **A layer classification.** `detect_layer()` works off the *file path string* (`/models/`,
   `/routes/`, etc.), which doesn't exist yet without #1. `BehaviorKind` (Validation,
   Construction, Persistence, ...) is a plausible proxy for layer but isn't currently used as one
   anywhere in `plan.rs`/`step.rs`.
2. **The field's own code-identifier form and type.** `required_tests` carries prose
   ("Name longer than 200 characters..."), not `{field: "name", type: "string"}`. A model can
   *usually* recover this from the PascalCase contract `name` for a single-word field, but that
   parsing is exactly the kind of inference the whole pipeline exists to eliminate elsewhere
   (see "the recurring principle" in the design doc) — un-parseable for any compound field/entity
   name.
3. **Sibling/dependency shape.** `dependencies` is a list of other contract *ids*. Concretely
   generating code against a dependency (e.g. calling a factory) needs its actual exported
   signature — today that comes from Roots symbol surfaces computed at generation time from
   already-written files, not from anything on the `Contract` itself.
4. **The layer-conditional exception rules currently baked into `step.rs`.** The design doc's
   own "What this replaces" section names these as things a contract-driven pipeline should make
   unnecessary ("a file's contract only ever contains the behaviors already assigned to it; there
   is nothing left to filter") — but today they still exist and are still load-bearing, because
   nothing consumes contracts yet.
5. **Decision Point resolution flow-through.** The design doc flags this itself as an open gap:
   "currently nothing re-derives a blocked behavior once its decision resolves" — so even the
   planning side doesn't yet fully close the loop Stage 2 opens.

## Q3 — Can a single small contract be implemented in isolation today?

**Chosen representative contract:** the unit-scope validation contract for `Manufacturer.name`,
hand-traced from `manufacturer-001`'s real `spec.yaml` through the actual mechanical functions
(no LLM step involved for this particular contract — construction and validation contracts are
100% mechanical):

- `mechanical_validation_behaviors` (`behaviors.rs`) on the `name` field
  (`mandatory`, `max_length: 200`, `min_length: 1`) produces two behaviors:
  - `manufacturer-001-b001`: `subject=ManufacturerName, kind=Validation,
    statement="Name longer than 200 characters is rejected.", source_ref=Manufacturer.name.max_length`
  - `manufacturer-001-b002`: `subject=ManufacturerName, kind=Validation,
    statement="Name shorter than 1 characters is rejected.", source_ref=Manufacturer.name.min_length`
- `mechanical_cluster` (`clustering.rs`) groups both by `(subject=ManufacturerName, kind=Validation)`
  into one `UnitCluster`.
- `mechanical_unit_contracts` (`contracts.rs`) produces:

```yaml
id: manufacturer-001-contract-001
name: ManufacturerNameValidation
scope: Unit
source_cluster: manufacturer-001-cluster-001
owned_behaviors: [manufacturer-001-b001, manufacturer-001-b002]
required_tests:
  - "Name longer than 200 characters is rejected."
  - "Name shorter than 1 characters is rejected."
dependencies: []
derivation: Mechanical
```

**Can this be implemented using only this contract + its dependencies (empty) + language +
framework rules, with no story/spec/behaviors/ADRs?** No — three concrete blockers, not
hypothetical ones:

1. **No file target.** Nothing says this is `services/manufacturer-service/src/models/Manufacturer.ts`
   (or a Spring `@Entity`, or wherever). Without a path there's no way to pick a tech-stack
   skill, since skill selection today is keyed off the file (`detect_layer(&step.file)`) or the
   service's declared technology — not off anything on the contract.
2. **No canonical field identifier.** `ManufacturerNameValidation` parses back to entity
   `Manufacturer` + field `Name` reasonably for this one-word case, but the rule is fragile: a
   real contract like `OrderLineItemQuantityValidation` is genuinely ambiguous between entity
   `OrderLineItem`/field `quantity` and entity `Order`/field `lineItemQuantity` — there's no
   delimiter in a PascalCase string. The contract has no separate `entity`/`field` fields to
   disambiguate.
3. **No mandatory/optional signal.** `min_length: 1` implies "this field can't be empty," which
   in turn implies "mandatory" — but that's an inference from the prose threshold, not a stated
   fact. A model would have to guess whether a missing-value case should throw or should be
   treated as legitimately blank.

What it *doesn't* need, and correctly doesn't have: the story narrative, the full scenario list,
or the ADRs — the two `required_tests` strings already carry precisely the behavioral content
(field, direction, threshold) a Green-phase implementation needs for *this specific unit*. That
part of the design's premise holds up under this trace. The gap is entirely in *grounding*
(file/layer/identifier), not in *behavioral completeness*.

## Q4 — Language-independence test

Would this same contract change if `manufacturer-service` were Spring Boot instead of ASP.NET,
or `manufacturer-registration-portal` were React instead of Vue? **No** — every field on
`Contract` (`id`, `name`, `scope`, `source_cluster`, `owned_behaviors`, `required_tests`,
`dependencies`, `derivation`) is already free of any tech-specific vocabulary. The
`required_tests` prose ("Name longer than 200 characters is rejected") reads identically
regardless of target stack.

This is the one place the current design already matches its own goal: contracts are
language-independent by construction, and the tech-specific translation is already correctly
isolated in `canopy-llm/src/skills/tech_stack.rs`'s per-layer skills (confirmed:
`render_for_layer`/`render_for_planning`/`render_all_layers` are the only places technology
vocabulary — Jakarta annotations, zod schemas, factory-function conventions — enters a prompt).
The problem isn't that tech-specific information leaks into contracts today; it's that contracts
don't yet carry the *non-tech-specific* grounding (file target, layer, field identifiers) a skill
would need in order to act on them at all.

## Q5 — Smallest possible experiment

**Experiment name:** Single-Contract Isolated Implementation Probe

**Inputs:**
- One real unit contract, hand-selected for minimum ambiguity (e.g. the
  `ManufacturerNameValidation` contract traced above, or a fresh construction contract) —
  generated for real by running Stages 0-1/3-4 against `manufacturer-001` (not hand-traced this
  time), so the probe uses a genuine pipeline artifact, not a manual reconstruction.
- A manually-authored, minimal augmentation of that one contract with exactly the three fields
  identified as missing in Q3: `implementation_target` (file path), `layer`, and an explicit
  `field`/`entity` pair (not parsed from `name`). This augmentation is hand-added *outside* the
  pipeline for this experiment only — it is the thing being tested, not a proposed schema change
  yet.
- The relevant tech-stack skill for one target layer (e.g. the Node/Express "model" skill),
  rendered via `render_for_layer("model")` exactly as `step.rs` does today.
- Nothing else: no story text, no full scenario list, no ADR summary, no OpenAPI spec.

**Success criteria:**
- A model given only {augmented contract, one skill's layer rules} produces a file that (a)
  compiles/type-checks, and (b) whose behavior genuinely satisfies both `required_tests`
  statements (verified by writing the two tests independently from the same contract, not
  copied from the implementation).
- The output requires no invented information beyond what the contract + skill state (e.g. it
  doesn't guess a field name, doesn't invent an unrelated field).

**Failure modes to watch for:**
- Model invents plausible-but-wrong surrounding structure (extra fields, wrong constructor
  shape) because nothing in the augmented contract constrains the *rest* of the entity beyond
  this one field — a single validation contract is inherently a partial view of its owning
  entity.
- Model produces correct logic but in a shape sibling contracts (e.g. the construction contract
  for the same entity) wouldn't compose with, since this experiment deliberately doesn't test
  multi-contract composition.
- The three added fields turn out insufficient once real generation is attempted — e.g. type
  information for the field might still be needed and wasn't included in the augmentation.

**Expected learnings:**
- Whether `implementation_target` + `layer` + explicit `field`/`entity` are actually *sufficient*
  additions, or whether the probe surfaces a fourth missing piece (e.g. field type) not visible
  from static code reading alone.
- Whether a model, freed from the current large layer-conditional prose rules in `step.rs`,
  produces cleaner output with just a contract + skill — a direct test of the design doc's
  "nothing left to filter" claim.
- A concrete, evidence-based shape for what a minimally-augmented `Contract` schema should look
  like, before committing to wiring the whole `implement` pipeline around it.

This is deliberately scoped below "prototype single contract implementation" (Q7 option A) —
it's a probe of the *schema's sufficiency*, using one hand-augmented contract, not a rewire of
`canopy implement` to consume `contracts.yaml` for a real story end-to-end.

## Q6 — Failure modes ranked by risk

1. **Contract missing behavioral detail beyond the owning field itself (highest).** A single
   unit contract is a slice of one field or one system-generated property — it says nothing
   about the rest of the entity it belongs to. Implementing it "in isolation" still requires
   knowing enough about sibling fields to produce a coherent constructor/class, which no single
   contract states. This is the sharpest version of the Q3 finding.
2. **Dependency ambiguity in mechanical derivation.** Confirmed live, not hypothetical, but with
   a correction made after a closer look (see below): the real `manufacturer-001` ADR-006 has
   `decision: "ManufacturerRegistered"` — no `" on topic "` substring. `parse_event_adr`
   (`behaviors.rs:394-400`) requires that exact substring to extract anything; on this real ADR
   it returns `None`, so `mechanical_event_behaviors` would silently produce **zero** event-shape
   or publication behaviors for this story today.

   This is **not** a parser bug or model non-compliance, on closer inspection —
   `architectural_questions_prompt` (`spec.rs:218-222`) explicitly says to omit the topic "if no
   Topic Naming Convention ADR exists," and `cmd_init` (`canopy-cli/src/commands/init.rs:40-64`)
   makes a Topic Naming Convention ADR mandatory (`select_required`, non-skippable) for any
   project whose architecture style is event-driven — which is the only architecture-style option
   offered today, so a *freshly-initialized* project always gets one. This dogfooding project's
   `.canopy/decisions/` (adr-001 through adr-006, no adr-000, no "Topic Naming Convention" title
   anywhere) predates that mandatory step — it's a stale fixture from before `cmd_init` enforced
   it, not a live defect a fresh project can currently hit.

   What *is* still a real, generalizable gap, independent of this one stale fixture: **no audit
   exists to catch a domain-event ADR that produced zero event-shape/publication behaviors**,
   for whatever reason (a stale project, a hand-edited ADR, a future architecture-style option
   that doesn't force the convention). `audit_behavior_coverage` checks every *scenario* produced
   a behavior, not every domain-event ADR. This is worth its own mechanical audit — see the
   recommendation below — not because today's code has a bug, but because the mechanical
   derivation has a silent failure mode with no detector.
3. **Language-specific leakage (lowest, per Q4).** The current schema is already clean here by
   construction — least likely of the six to actually manifest, provided new fields added to
   close the Q2/Q3 gaps stay equally tech-neutral (a raw file path is fine; a Java-package-shaped
   string baked into the contract itself would reintroduce this risk).
4. **Hidden architecture assumptions.** The unit-contract dependency rule ("depends on the
   construction contract for the same subject, when one exists") is the *only* mechanical rule at
   unit scope — genuinely sound for construction→persistence/publication ordering, but it's one
   rule covering one relationship shape; anything else (e.g. one unit contract needing a shared
   value object another unit contract also constructs) has no mechanical path today and would
   silently fall through with `dependencies: []`.
5. **Responsibility ambiguity.** Lower risk than it first appears: Stage 3's clustering by
   `(subject, kind)` and Stage 4's "structural constraint" (a contract may only contain behaviors
   from its own cluster) already make cross-responsibility bleed structurally impossible, not
   just discouraged by prompt wording — this is the one place the design's stated guarantee
   ("there is no path by which a validation behavior could ever reach the repository contract")
   held up under inspection of the actual code, not just the doc's claim about it.
6. **Implementation prompt still depends on upstream artifacts (confirmed fact, not a risk to
   assess).** Already established directly in Q2: `plan.rs`/`step.rs` read `story`/`spec`/`adrs`
   today, not contracts, at all. Listed last because it's not a risk of the *contract schema*
   being wrong — it's simply the current, known, undisputed state of the wiring.

## Q7 — Recommendation

**B: strengthen contract generation first — specifically, close the three gaps identified in Q3
(implementation target, layer, explicit field/entity) — before attempting A.**

Reasoning, grounded in what was actually found above, not intuition:

- The behavioral *content* of contracts already appears sufficient for narrow, single-field unit
  contracts (Q3) — the prose statements carry real threshold values, and the clustering
  structural constraint genuinely prevents cross-responsibility bleed (Q6 #5). This part of the
  design doesn't need rework.
- What's missing is narrow and enumerable, not a fundamental schema redesign: a file path, a
  layer tag, and disambiguated field/entity identifiers. All three are mechanically derivable —
  none require new LLM judgment, matching this project's own "compute facts mechanically" rule.
  This argues against jumping straight to a full prototype (A) before the schema even carries
  enough to attempt one meaningfully, and against a ground-up redesign (C) — nothing found here
  suggests the `Behavior → Cluster → Contract` shape itself is wrong, only incomplete.
- Q5's probe is the concrete next step this recommendation implies: test the three-field
  augmentation on one real, freshly-generated contract before deciding whether those three
  fields are actually sufficient or whether a fourth (e.g. field type) is still missing. That
  probe is scoped well below "wire the whole pipeline" and produces evidence either way within
  a single, bounded experiment.
- The ADR-006/no-topic case (Q6 #2) turned out, on closer inspection, not to be a live code
  defect — the dogfooding project's `.canopy/decisions/` predates `cmd_init`'s mandatory Topic
  Naming Convention ADR, so a freshly-initialized project can't currently reproduce it. What *is*
  real and generalizable is the missing detector: **done** — `audit_behavior_coverage`
  (`canopy-llm/src/prompts/behaviors.rs`) now also checks that every domain-event ADR for a
  story's entity produced at least one `EventShape` behavior, via a new
  `adr_event_coverage_findings` helper, covered by 4 unit tests. Pure audit, no LLM involved, no
  rewriting of any generated artifact — it only surfaces a finding.

## Addendum — corrected diagnosis and status (2026-07-14)

After the initial write-up above, closer inspection of `canopy-cli/src/commands/init.rs` showed
the ADR-006 case is a stale-fixture artifact, not a reachable defect in current code (see Q6 #2's
revised text and the audit note just above). The audit fix is implemented and tested.

Of the three schema options weighed for the grounding gap (Q3) — minimal (relabel `subject`,
still ambiguous for compound names), recommended (split `subject` into explicit `entity`/`member`
at the point of origin, no LLM), most explicit (also lift constraint values into typed data) —
**recommended (Option 2) was chosen and is now implemented**: `Contract` gained
`kind: Option<BehaviorKind>` (a direct copy of `UnitCluster.kind` — this doubles as the
contract's language-independent "layer"), `entity: Option<String>`, and `member: Option<String>`;
`Behavior` gained the same `entity`/`member` pair, populated at the exact point `entity`/
`field.name` were previously being concatenated into `subject` (`mechanical_validation_behaviors`,
`mechanical_construction_behaviors`, `mechanical_event_behaviors` in `behaviors.rs`), never by
re-parsing `subject` afterward. `subject`/`name` are unchanged. Scenario-derived (LLM) behaviors
leave both `None` — the model was never asked for a structured split, and inventing one by
parsing its `subject` output would reintroduce the same ambiguity. Covered by 3 new unit tests
in `contracts.rs` exercising a validation contract, a construction contract, and an integration
contract. No prompt or skill text was touched by this change.

Deliberately deferred, per the agreed sequencing: no consumer reads these new fields yet — file-
target computation still needs the structured kind→directory mapping (the shared prerequisite
noted above) before `canopy implement` could use any of this, and that reassessment of whether a
contract is sufficient implementation input comes only after that prerequisite lands.

## Addendum 2 — kind→directory mapping implemented and verified (2026-07-14)

The shared prerequisite is done: `canopy-llm/src/skills/file_targets.rs` adds
`abstract_layer_for_kind(kind) -> &str` (a fixed, tech-agnostic table — Validation/Construction→
"model", Persistence→"repository", EventShape→"event", Publication→"infrastructure",
Orchestration→"service", HttpRequest/HttpResponse→"route", ErrorTranslation→"middleware") and
`resolve_implementation_target(tech, pkg_path, service_name, layer, entity, event_name)`, a
per-tech-family match transcribing each stack's already-documented directory convention from
`tech_stack.rs`'s prose skills into queryable Rust — no LLM call anywhere in it. 8 unit tests.

**Verification against real `manufacturer-001` data** (services.yaml: `manufacturer-service` =
Spring Boot, `manufacturer-registration-portal` = React), using the two contracts hand-traced in
Q3 above:

| Contract | kind | entity | resolved target |
|---|---|---|---|
| `ManufacturerNameValidation` | Validation | Manufacturer | `services/manufacturer-service/src/main/java/.../domain/Manufacturer.java` |
| `ManufacturerConstruction` | Construction | Manufacturer | `services/manufacturer-service/src/main/java/.../domain/Manufacturer.java` (same file — correct: a JPA `@Entity` class carries both validation annotations and its constructor) |

Confirms the core claim: **yes, a file target is mechanically derivable from `contract.kind` +
`contract.entity` + tech-stack conventions**, with `pkg_path` supplied the same way `plan.rs`
already computes it today (scaffold-detected or a documented fallback) — no new input invented.

Two real limits surfaced while building this, not smoothed over:
- **`entity` alone doesn't cover every kind.** An event-shape contract's file is named after the
  *event* (`Contract.subject`, e.g. `ManufacturerRegistered.ts`), not the entity — `subject` had
  to stay in the loop for this one case. A publication contract's file (`EventPublisher.ts`) is
  fixed and entity-independent — neither `entity` nor `subject` matters there.
- **Coverage is uneven across stacks, by design, not by bug.** Node/Express (the stack most of
  this project's skill prose was written against) resolves every layer. Spring Boot resolves
  model/repository/service/route but not event/infrastructure/middleware — `spring_boot_skill`
  itself doesn't document those yet, so returning `None` is accurate, not a gap in this new
  module. React resolves only its api-client layer (a form-only frontend story has nothing
  behind it). Angular's orchestration and http-request/response contracts resolve to the *same*
  file (`<feature>.service.ts`) — a real architectural difference (Angular doesn't split the two
  concerns the way Node/React do), not a resolution failure.

This means the manufacturer-001 story's real stack (Spring Boot + React) can mechanically place
its validation/construction contracts today, but an event-shape/publication contract for that
same story — which would exist once the stale ADR-006 fixture is regenerated with a real Topic
Naming Convention ADR — has no mechanical target yet on Spring Boot. That's a real, separately-
scoped follow-up (extending `spring_boot_skill`'s own documented layout to cover event-driven
JVM services), not a defect in the kind→directory mapping itself.

**Q3 revisited — the three original blockers, checked against what's landed:**

1. *No file target.* **Resolved**, mechanically, for the layers each stack's skill already
   documents (see the table above and the coverage note) — not yet resolved for the
   undocumented (family, layer) pairs, which is now a visible, named gap instead of a silent one.
2. *No canonical field identifier* (the `OrderLineItemQuantityValidation` ambiguity). **Resolved**
   by Option 2 — `entity`/`member` are explicit fields now, never re-parsed from `name`/`subject`.
3. *No mandatory/optional signal.* **Still open** — neither Option 2 nor this file-target work
   touched it. A validation contract's `required_tests` prose still only implies mandatoriness
   (e.g. `min_length: 1` reads as "can't be empty"); there's no explicit boolean anywhere on
   `Contract`. Not in scope for either committed slice — worth flagging as the next concrete gap
   if the isolated-implementation experiment (Q5) is run next and hits it.

## Addendum 3 — Q5 executed: mandatory/optional signal sufficiency probe (2026-07-14)

**Experiment name:** Mandatory/optional signal sufficiency probe (`canopy-llm/examples/
contract_isolation_probe.rs`) — a narrower, targeted version of Q5's original probe, aimed
specifically at the one open blocker from Q3 revisited above.

**Inputs:** two real, mechanically-derivable unit contracts from `manufacturer-001`'s actual
entity schema — `ManufacturerNameValidation` (mandatory field, `min_length: 1`) and
`ManufacturerPhoneNumberValidation` (optional field, `min_length: 0` — vacuous, so *no*
lower-bound behavior exists in the contract at all). Each `Contract`/`Behavior` value was
hand-constructed to exactly match verified mechanical output (same shape as this project's own
`unit_validation_contract_carries_kind_entity_and_member` test), not re-derived — the point was
to isolate what the model sees, not to re-test derivation. Given to the model: `entity`,
`member`, `kind`, `required_tests` verbatim, and the Spring Boot skill's `model`-layer rules via
`skill_for_technology`. No story, no spec, no ADRs, no other fields. Run against the local
Qwen2.5-Coder-14B server this project already dogfoods against.

**Success criteria:** the model infers `name` requires a non-blank value (the only signal is the
"shorter than 1 characters" prose) AND leaves `phoneNumber` nullable (no lower-bound behavior
exists to imply otherwise).

**Result — 3 runs, identical prompts, matching this project's own reproducibility-sweep standard:**

| Run | `name` (mandatory) | `phoneNumber` (optional) |
|---|---|---|
| 1 | `@NotBlank` + `@Size(min=1,max=200)` — correct | `@Size(max=20)` only — correct |
| 2 | `@NotBlank` + `@Size(min=1,max=200)` — correct | `@NotBlank` + `@Size(max=20)` — **wrong, over-constrained** |
| 3 | `@Size(min=1,max=200)` only, no `@NotBlank` — **wrong**: `@Size` alone does not reject `null` in Jakarta Bean Validation, only a blank/empty string — a missing `name` would silently pass | `@Size(max=20)` only — correct |

**Every run got at least one of the two fields wrong**, and no two runs agreed on which one.
This is not a close call: the same prompt, unchanged, produced three different outcomes. The
originally-hypothesized failure mode (over-constraining the optional field) *did* occur (run 2),
but so did an under-constraining failure on the *mandatory* field that hadn't been anticipated
going in (run 3) — a real miss the prose-only signal doesn't protect against either direction.

**Conclusion: the mandatory/optional gap is genuinely load-bearing, not a theoretical nicety.**
Confirmed live, not assumed — this settles Q3's third blocker as a real, not speculative, gap.

**Smallest contract extension implied by this result:** a `mandatory: Option<bool>` field,
symmetric with `entity`/`member`'s Option-2 shape — populated exactly where `is_mandatory` is
already a known local variable in `mechanical_validation_behaviors` (`behaviors.rs`), `Some(true)`
for a `schema.mandatory` field, `Some(false)` for `schema.optional`, `None` for every non-
Validation kind (mandatory/optional isn't a meaningful concept for construction, event-shape, or
publication). Zero LLM involvement, same mechanical-propagation shape already proven for `entity`/
`member`.

**Implemented.** `Behavior.mandatory`/`Contract.mandatory` landed exactly as scoped above —
`Some(is_mandatory)` for every validation behavior, `None` for construction/event-shape/
publication and for scenario-derived behaviors, propagated to `Contract` via the same
first-`Some`-found lookup already used for `entity`/`member`. Deliberately scoped to a plain
boolean, nothing richer (no separate "why," no default-value semantics) — matching the explicit
decision to avoid introducing presence semantics beyond what this one experiment's evidence
actually demanded. Covered by two new/updated unit tests: a mandatory validation contract
(`name`) asserts `mandatory: Some(true)`, an optional one (`phoneNumber`) asserts
`Some(false)`; the construction and integration contract tests assert `None`.

**What this result says about the contract model overall:** the finding was not "contracts are
insufficient" — it was "contracts were almost sufficient, and the one missing fact was
identifiable through a reproducible experiment, then closed with a small, mechanical,
zero-LLM addition." Three fields added across this whole investigation (`kind`, `entity`/
`member`, `mandatory`), each one demonstrably necessary (Q3's blockers, then this probe) and
none speculative — a materially different outcome than either "contracts need a ground-up
redesign" or "contracts were already complete."
