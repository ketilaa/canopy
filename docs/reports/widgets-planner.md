# Story Report — `widgets` (Planner, plan-only sweeps)

A running record of the Planner's plan-only batches against the widgets story (a story, a Gherkin
feature, the `java-widgets@baseline-00-widgets` repository). Design and build order:
`docs/design/planner-first-increment.md`. Model: Qwen3-14B Q4_K_M, thinking off, 4096 completion
tokens, on the GPU-accelerated server. Every batch is n=10 plans, one draft call plus at most one retry,
no Implementer run. A plan counts as **valid** when it parses, passes the five verifier rules and has
every scenario covered. Batch records (harness commit, server properties, sampling) are in each batch's
`batch.json`; the run directories are git-ignored.

Caveat for everything below: one story, one stack, one model, n=10 per batch. The numbers say what
this model does on this story, not what planning in general looks like.

---

## 2026-10-10 — first four batches

First live run (n=1, before the batches): all eight steps came back `refactor`, which exempts a step from
the test rule, so production code with no test passed the verifier. The model no longer chooses the kind
(see `05ba2e6`). The Implementer then failed that plan at step 1 (0 of 8): the instruction said only "a
model class with an id and a name" and the model reached for `jakarta.persistence`.

| Batch | Harness | Change under test | Valid | First draft valid | What the rejections were |
|---|---|---|---|---|---|
| 1 | `05ba2e6` | `kind` removed from the schema | 0/10 | 0/10 | 6 runs: production steps with their tests in later steps (missing test). 4 runs: one file in both `create` and `modify`. |
| 2 | `94a0905` | prompt rule "every step with production code carries a test"; overlap error names the lists | 0/10 | 0/10 | 8 runs still missing tests (the rule was in the request, confirmed in the event log); 2 overlap. |
| 3 | `27c1166` | `tests` a required, non-empty field of every step | 0/10 | 0/10 | 10 runs: a test file listed in both `create` and `tests`. No missing tests. |
| 4 | `f6c741d` | path patterns in the schema: `create`/`modify` under `src/main`, `tests` under `src/test` | 1/10 | 0/10 | 9 runs: a file in both `create` and `modify` (see below). |
| 5 | `7999338` | harness decides create or modify (option A): the draft lists `files`, `read_only`, `tests` only | 4/10 | 3/10 | 6 runs: a file in both `files` and `read_only` (38 occurrences over both attempts, all that one pair); 1 duplicate step id; 1 step over the size cap. |
| 6 | `e259632` | repository facts stamped into every step as its context | 5/10 | 2/10 | same as batch 5 (the facts are not in the Planner's prompt change; validity is within noise of batch 5) |

### What the batches showed

- **A plain instruction did not hold (batch 2).** The rule was in the prompt, the retry named the failing
  steps, and the model still planned production and test steps apart in 8 of 10 drafts. A schema field that
  cannot be empty removed the failure class completely (batch 3: 0 missing tests).
- **A field the model can misuse, it does (batch 3).** With `tests` added next to `create`, every draft
  listed the test file in both. Grammar-enforced path patterns removed that too (batch 4); the llama-server
  accepted `pattern` in the schema.
- **The remaining failure is state, not shape (batch 4).** The model writes later steps as "update `X`"
  and lists `X` in `create` and in `modify` together, and gives every test the action `create` even when an
  earlier step already created it. It does not track which file an earlier step made. Retrying with the
  named lists did not change that (9 of 9 identical on the second attempt).
- **The instructions are still thin.** The one valid plan (batch 4, run 10) has eight steps with no method
  signatures, no concrete test values and no wiring: the three costs the hand-written plan carries. Whether
  such a plan runs is the end-to-end question, not yet asked at n=10.
- **Cost.** A draft takes 20 to 40 s; a failed draft plus retry 40 to 80 s.

### End to end: the first model-written plans run by the Implementer

Valid plans from batches 5 and 6 were run once each through `run_step --task widgets-planned --plan <file>`
(same hidden HTTP test and structure checks as the hand-written plan, same GPU-accelerated server).

| Plans run | Scored | Failed at | Why |
|---|---|---|---|
| 4 (batch 5, no facts in the steps) | 0 | step 1 or 2 | 3 of 4: `jakarta.persistence` or `spring-data-jpa` classes the project does not have; 1: the Implementer looped on one read |
| 5 (batch 6, facts with the dependency list as each step's context) | 0 | step 1 or 2 | 5 of 5 the same: `jakarta.persistence` / `org.springframework.data.repository` |

The facts, including `Dependencies: spring-boot-starter-web, spring-boot-starter-test (test)`, did not stop
the Implementer reaching for JPA. The plans say "entity" and "repository interface" and never say how data is
stored; the hand-written plan said "a Java record" and "a ConcurrentHashMap". Where that decision comes from
is not answered by the Planner today (the story does not say; in the full pipeline it is an ADR). Not
changed yet.

### Hardening experiments (2026-10-10, small n; each stacked on the one before)

Plan-only run, then the Implementer on each valid plan (`run_step --plan`). "Steps" is how many plan steps
the Implementer passed before it stopped.

| Experiment | Harness | Change | Plans | Valid first draft | Implementer steps passed | Notes |
|---|---|---|---|---|---|---|
| 1a | `bcb033e` | drop a `read_only` entry that is also a changed file | 1 | 1/1 | 1 of 10 | JPA classes at step 2 (`spring-data-jpa`) |
| 2a | `071aeb4` | computed facts line: no database or JPA library, keep data in memory | 1 | 1/1 | 1 of 16 | no JPA anywhere; stuck loop at the controller step |
| 2c | `2e8cbfa` | prompt rule: concrete implementation choices (record or class, field types, collection class) | 3 | 3/3 | 2, 2, 2 | 0 JPA mentions; all three stop at the controller step |
| ro | `99f1b8a` | `read_only` removed from the draft; the harness computes it from the class names an instruction mentions | 3 | 1/3 (2 retries on the step-size rule) | 1, 2, 2 | same stop point as 2c |
| cn | `623ab1e` | the draft names classes (`classes`, `test_classes`, qualified names under the root package); the harness derives the paths; the facts list classes, not files | 3 | 3/3 | 2, 1, 1 | same stop point; the server accepts the grouped `pattern` |

What this says, with n this small: JPA disappeared once the storage fact and the concrete-mechanism rule
were in (2a, 2c; the two are stacked, so which one did it is not known). Validity is now saturated at 3/3 and
is no longer the useful measure; steps passed in the Implementer is. Computing `read_only` did not change
that (ro vs 2c).

Found by the run-analyst test (below): from `623ab1e` on, every step's context said `Existing classes:
WidgetsApplication`, even steps that run after earlier steps of the same plan had created classes. The facts
are about the repository before the plan. Each step's context now also lists the classes and test classes the
earlier steps create (derived by the harness). Not yet measured: no batch has run with it.

### Diagnosis: what stops every run (2026-10-10)

Seven stuck Implementer runs (2a, 2c, ro, and one from earlier) were read refusal by refusal. Six of the
seven fail at the controller step on one refusal, repeated 5 to 10 times until the stuck-loop guard fires:
"Not submitted. No changed test mentions WidgetController." The seventh failed on a compile error in the
service. The submitted test is a legitimate HTTP-level test (`@SpringBootTest` with a random port and a
`RestTemplate` posting to `/widgets`) that reaches the controller without ever writing its name, so the
Implementer's no-test gate refuses it and the model, which cannot see why, resubmits the same thing.

So the stop point is an Implementer gate that does not recognise a controller test written through HTTP,
**not** a gap in the plan. The hand-written plan passes the gate because it prescribes
`new WidgetController(...)` in the test, which names the class; that is why it carried the wiring text. An
earlier explanation in this report ("unstated test wiring") was a hypothesis and is withdrawn. Note what
this does to the measurements: from the controller step onward the Implementer, not the Planner, decides
the outcome, so steps passed cannot show planner improvements beyond step 2 until the gate is dealt with.
Options (not decided): accept a Spring integration test when it contains the controller's mapped path or
carries `@SpringBootTest`/`@WebMvcTest`; or keep the gate and have the plan name the wiring.

### The gate change and its rerun (2026-10-10, `fb76d26`)

The no-test gate now also counts a changed test as reaching a class when the test has a string literal
containing a URL path the class maps (the static prefix before a path variable; the bare `/` matches
nothing), and its refusal says what would count ("or a request to its path /widgets"). Four stored plans
were rerun on the same server and JDK, one run each, with nothing else changed:

| Plan | Steps passed before | After | Stop after |
|---|---|---|---|
| cn run-01 | 2 of 6 | 2 of 6 | compile: `getName()` called on a record (`Widget`) |
| cn run-02 | 1 of 10 | 1 of 10 | budget: 12 tool calls without a buildable submission |
| cn run-03 | 1 of 12 | 4 of 12 | stuck on a different gate: files the plan lists as `modify` left unchanged (6 refusals) |
| 11-52-53 | 3 of 8 | 3 of 8 | test: Spring `ApplicationContext` failed to load |

What the gate change did: the refusal "No changed test mentions WidgetController" is gone from all four runs
(it was 7 to 10 per run). What it did not do: raise steps passed in most runs (1 of 4), because the controller
step now fails on other things (a compile error from an assumed JavaBean getter on a record, a context that
does not load, a budget stop). That is the analyst's own falsification condition for "the gate was the only
cause": it was a cause, not the only one. n=4, one run each, so these are directions.

One plausible plan-shape cost, not yet shown: `test_classes` must be non-empty in every step, so a later step
that only needs to change a production class still lists a test as `modify`, and the Implementer refuses a
`modify` file the step leaves unchanged (run-03, 6 refusals).

### Experiment conv: the Spring stereotype convention in the facts (2026-10-10, `699ca12`)

Facts (and so every step's context, and the Planner's prompt) now say "Stereotypes by class name: *Controller is
@RestController, *Service is @Service, *Repository is @Repository, *Config is @Configuration." Three new plans,
one Implementer run each:

| Plan | Valid first draft | Steps passed | Stop |
|---|---|---|---|
| conv run-01 | yes | 1 of 6 | budget (12 tool calls without a buildable submission) at the service step |
| conv run-02 | yes | 0 of 8 | compile: `javax.validation.constraints` does not exist (Spring Boot 3 uses `jakarta.*`) |
| conv run-03 | yes | 2 of 8 | compile: the controller calls `getWidgets()`, which `WidgetService` does not have |

Mechanical check on the final patches: before the line, 2 of 2 runs that wrote a service class left it
without `@Service` (12-28-09, 12-33-10; the two "no qualifying bean" failures); after it, the one run that got
as far as writing a service annotated it (1 of 1). "No qualifying bean" appeared in 0 of 3 runs. That is the
predicted direction, on a single run that could show it, so it is **not** evidence that the line works; steps
passed (1, 0, 2) are no better than before. The line was present in the step contexts (13 mentions in one
trace); whether the model used it is not shown.

New recurring class: `javax.validation` imports in a Spring Boot 3 project, now in 2 of 7 Implementer runs
overall (12-29-18 and conv run-02). Derivable from the facts (Spring Boot 3.4.5), so it is the next candidate
for a stated fact. Separate and undiagnosed: a controller calling a method its service does not have.

### Experiment val: no bean validation library, stated in the facts (2026-10-10, `803171a`)

`javax.validation` imports had stopped 2 of 7 Implementer runs. The planned fix was a "use jakarta" line, but a
probe compile in a scratch copy of the baseline showed `jakarta.validation.constraints` does not exist on this
project's classpath either (only the web and test starters), so that line would have sent the model to a
different missing package. The fact stated instead is computed from the dependencies, like the storage line:
"Validation: no bean validation library is available (neither javax.validation nor jakarta.validation); check
values in code." Three new plans, one Implementer run each:

| Plan | Steps passed | Stop |
|---|---|---|
| val run-01 | 0 of 11 | compile: `org.junit.jupiter.api` does not exist (a test import in a non-test file) |
| val run-02 | 3 of 6 | a step required a file it never created (4 refusals), then a stuck loop |
| val run-03 | 0 of 7 | compile: unclosed character literal |

Validation-package compile errors: 0 of 3 runs (the fact was in the context: 11, 21 and 13 mentions), against
2 of 7 before. Steps passed are not better (0, 3, 0), and the failures are now three different things, so no
class recurs. That is the pattern of the whole session: each stated fact removed the class it targeted and the
runs then stopped on another. The one measure that kept moving was the targeted class itself.

### Experiment aud: audit the class lists (2026-10-10, `97447b0`; batch `2026-10-10T19-30-33`)

From the all-day run analysis: a production class whose name ends in `Test` is rejected (a test listed under
`classes` got a main path and failed compiling JUnit imports), and every production class a step lists must
be named in its instruction (a controller was listed that the instruction never described). Both are audits
with a retry that names the problem. Ten fresh plans, an Implementer run on each valid one.

Result: **falsified on plan validity, 1 of 10 valid, 0 of 10 on the first draft** (the stated falsification
threshold was below 5 of 10).

| Problem | First drafts | Retries (second attempt) |
|---|---|---|
| a `...Test` class listed under `classes` | 5 | 8 |
| a listed class the instruction never names | 5 (in 4 plans) | not visible: the first error returned hides the second |
| a step touching 5 files (the size cap) | 0 | 1 |

The retry named the step and the class and told the model to move it to `test_classes`; 8 of the 9 retries that
could repeat it did. The same pattern as the earlier create-or-modify audit: a model that does not follow a
plain correction after a named failure is not helped by naming it again; the shape has to carry it. The model's
reason is visible in the drafts: for a test-only step ("test the controller") it puts the test class under
`classes` because the test is the subject of the step, and the schema allows it (`test_classes` is required to be
non-empty, `classes` only has a pattern for the package).

The one valid plan was run through the Implementer: 5 of 8 steps passed, the furthest of any generated plan so
far (the previous best was 4). One run, so not a result, and it passed the audits only after a retry.

Two flaws in the experiment itself: the audits return the first error, so (a) hid (d) in the retries; and the
two audits changed the draft at once, so the e2e data from one plan says nothing about either.

### Experiment pat: a negative name pattern for `classes` (2026-10-10, `d6ea8de`; batch `2026-10-10T19-42-50`)

`classes` now only admits names that do not end in `Test` (positive character classes, no lookahead; tested
against the regex engine; llama-server accepted it). Audit (d), "every listed class is named in the
instruction", stayed a hard rejection. Ten fresh plans, an Implementer run on each valid one.

Result: **1 of 10 valid again, 0 of 10 on the first draft.**

- Audit (a) is gone: no draft lists a `Test` name under `classes`. The model went round it: test-only steps list
  a made-up name instead (`WidgetTestTestCases`, `WidgetRepositoryTestTestCases`), which then fails (d).
- Audit (d) is now the whole story: 8 of 10 first drafts failed it (2 more hit the 4096-token cap), and 5 of the
  retries that could repeat it did. The drafts show why: instructions such as "Create core classes for widget
  management" or "Create test cases for the Widget class" name no class at all, while the step lists four.
- The one valid plan (5 steps, instructions that name their classes) reached 1 of 5 in the Implementer.

Reading: (d) catches a real defect (a step whose instruction does not describe what it creates), but as a hard
rejection it is not satisfiable by this model: it cost 9 of 10 plans, and the plain-retry pattern did not hold
here either (the third time this session an audit with a named retry was not followed). In the cn and val
batches, with no (d), the same prompt gave 3 of 3 and 3 of 3 valid plans. An audit that rejects nine plans in
ten is not a gate. Options not yet taken: keep (d) as a recorded diagnostic instead of a rejection.

### Experiment grain: go with the model's grain (2026-10-11, `ae0805f`; baseline `baseline-01-widgets-grain`)

The widgets scaffold with `spring-boot-starter-data-jpa`, `spring-boot-starter-validation` and `h2` added
(Boot 3.4.5, unchanged), so the libraries the model reaches for exist. The "no database" and "no validation"
facts disappeared on their own (they are computed), and one computed fact was added: "Spring Boot 3 uses
`jakarta.*` packages, never `javax.*`". Three plans, one Implementer run on the valid ones.

- Plans: 1 of 3 valid. Two failed for reasons unrelated to the baseline: both attempts hit the completion cap
  (run-01) and a step touching 5 files (run-02). At n=3 this is not a validity result.
- The valid plan reached 0 of 8: step 1 failed on the tests. The model wrote the right namespace
  (`jakarta.validation.constraints.NotBlank`; no `javax`) and then mixed idioms: an annotation for the blank
  check and a hand-written check in the constructor for the length, while its own unit test expected
  `IllegalArgumentException` for a blank name. An annotation does not throw without a validator, so the test
  failed (2 of 3 tests).

So the namespace fact did what it was for, and having the library moved the failure from "the package does not
exist" to "annotation validation is not exception-throwing validation": a different class, not an absence of
failure. One Implementer run, so a direction. It does not show that going with the grain is better or worse;
it shows the model's idiom brings its own trap, which a stated convention (how this project validates) would
have to cover.

### Experiment contract: decisions in the repository, two contracts, one harness (2026-10-11, `f88be8a`, `c8c995d`)

The stereotype, storage and validation decisions moved out of `facts.rs` into `.canopy/contract.yaml` in the
repository (`4c9a0b3`). Two hand-written baselines differ only in the storage decision:
`baseline-02-widgets-contract-memory` (original dependencies, "keep data in memory") and
`baseline-03-widgets-contract-jpa` (data, validation and H2 starters, "persist with Spring Data JPA"). Same
harness, n=3 plans each, an Implementer run on each valid plan.

| Arm | Valid plans | Steps passed | Stop |
|---|---|---|---|
| memory contract (`f88be8a`) | 3 of 3 (all after a retry) | 3 of 7, 2 of 11, 2 of 8 | compile: `getName()` on a record or a request record (2 runs); `'void' type not allowed here` |
| JPA contract (`c8c995d`) | 0 of 3 | not run | all three plans rejected by audit (d), "a listed class is not named in the instruction" |

- The memory arm ran with the same facts as experiment val minus the hard-coded decisions, now rendered from
  the contract. Its steps passed (3, 2, 2) are higher than val's (0, 3, 0) but n=3 each, so a direction. The
  hard-coded stereotype, storage and validation decisions are gone from the Rust code; the harness line count
  for them went to zero.
- The JPA arm could not be measured: audit (d) rejected every plan (the same 8-of-10 effect as in pat). This
  is the cost of leaving (d) as a hard rejection, and it blocks the comparison the experiment was for. The
  decision to demote (d) to a recorded diagnostic is still open.
- A recurring class appears in the memory arm: the model calls `getName()` on a record (accessor is `name()`)
  in 2 of 3 runs, earlier seen as `getName()` and `getWidgets()` in the stop reasons of the cn batch. It is a
  convention (records have no JavaBean getters) that a scoped contract rule or a signature from Roots
  (`name()`) would state at the point of use.

### Experiment routed: the same contract decisions, scoped by layer (2026-10-11, `fe5d064`; baseline `baseline-04-widgets-routed-memory`)

The memory contract's decisions (stereotypes, storage, validation) in the v2 format, with the storage rule scoped
to the repository and service layers and the validation rule to service and controller; the planner and each step
are given only the entries that apply (`docs/design/contract-routing.md`). Three plans.

Result: **0 of 3 plans valid; all three rejected by audit (d)** ("a listed class is not named in the
instruction"), so no Implementer runs and no comparison with the unscoped memory arm (3, 2, 2 steps).
Audit (d) has now blocked three sweeps in a row (pat 9 of 10, the JPA contract arm 3 of 3, this 3 of 3), and
the one batch where it passed (the memory arm, 3 of 3, every plan after a retry) was luck against that rate.
The routing itself is covered by unit tests (the planner prompt omits implementer-only rules, a plan's steps get
different contexts by layer, an unknown-layer step fails open), not yet by a run. The experiment needs (d)
demoted from a rejection to a recorded diagnostic to be measurable. Decided: (d) is now a diagnostic. The draft's
class list is no longer checked against the instruction; the plan carries the unnamed classes in
`diagnostics` (summary.json, plan.meta.json), and `analyze_runs` reports the rate as a note, not a failure.

### Resolved: who decides create or modify

Whether a path is `create` or `modify` is mechanically known: `modify` if the repository has it or an earlier
step creates it, `create` otherwise. Having the model state it was where 9 of 10 batch-4 drafts failed.
Decided (option A, approved): the draft no longer has `create`/`modify`; the harness assigns them in
`draft::into_plan` from the repository and the earlier steps (see `docs/principles/compute-facts-mechanically.md`).
Batch 5: 4/10 valid, 3 on the first draft, up from 1/10 and 0/10.

### Resolved: a changed file listed again as read-only

Every remaining rejection of an otherwise well-formed draft in batch 5 was one pair: a file listed under
`files` and again under `read_only` (38 of 38 duplicates). `read_only` is no longer part of the draft; the
harness computes it (`99f1b8a`). Principle: `docs/principles/compute-facts-mechanically.md`, extension of
2026-10-10.
