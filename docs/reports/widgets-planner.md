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

### Audit (d) demoted, and the two blocked arms rerun (2026-10-11, `e240f9d`)

With (d) a recorded diagnostic, plans were valid 3 of 3 in both arms again (the unnamed-class note appeared in
5 of 6 plans, up to 14 classes in one 25-step plan). Implementer results, one run per plan:

| Arm | Valid | Steps passed | Stops |
|---|---|---|---|
| routed memory contract (`baseline-04`) | 3 of 3 | 1 of 8, 1 of 8, 2 of 25 | budget; "must change in this step" refusals; stuck |
| JPA contract (`baseline-03`) | 3 of 3 | 1 of 11, 2 of 9, 1 of 6 | no-test refusal on a repository; "must change" refusals; files outside the step's scope |
| (reference) unscoped memory contract (`baseline-02`) | 3 of 3 | 3 of 7, 2 of 11, 2 of 8 | `getName()` on records; `'void' type not allowed` |

Not a better result for routing or for JPA: the numbers are lower than the unscoped memory arm's, at n=3, one run
per plan, which cannot separate them. What the rerun did show, through `routing.json`: a step with a record and
its test (layers `test` alone) was given no contract rules, because the `test` layer made it look classified.
That is a bug in the routing (fixed in `776acf1`: fail-open needs a known layer of the code), and the contract's
own scoping was a guess: validation scoped to the service and controller layers, when for this story it lives in
the record. The routed arm was rerun with the fix.

A step-shape class recurs in both arms: files the plan lists as `modify` that the step then has no need to
change ("must change in this step", 4 refusals over 3 runs), the cost of a test being required in every step.

### Routed arm rerun with the fail-open fix (2026-10-11, `776acf1`)

Three plans (valid 3 of 3), one Implementer run each: 3 of 21, 2 of 7, 0 of 7. `routing.json` now shows the
record steps (layers `test` alone) given both rules, and the service, repository and controller steps given
only theirs, so the routing does what it is meant to. The steps passed are in the same range as every other
arm (unscoped memory 3, 2, 2; first routed run 1, 1, 2; JPA 1, 2, 1): at n=3 and one run per plan, **routing made
no measurable difference to the Implementer**. That is the expected result for a contract of three rules; the
point of routing is a contract that grows, and this experiment cannot show it either way. What it did show is
that the routing records were worth having: they found the bug.

Stops in this batch: the "must change in this step" refusals again (6 in run-01), and a compile error from a
wrong import, `LocalServerPort` taken from `org.springframework.boot.web.server` (it is in
`...boot.test.web.server`). The wrong-import class is the same shape as `javax` against `jakarta`: a symbol the
model has to place in a package it cannot look up. The import audit only knows JDK classes; a lookup against the
project's real classpath would resolve it (see `docs/open-questions/dependency-management-and-supply-chain.md`).

### The classpath import check in the gate (2026-10-11, `5e16ea0`)

Design and replay: `docs/design/classpath-class-index.md`. With the check on, twelve stored plans (conv, val,
routed3, jpa2) rerun once each: compile-stage stops 7 to 3, test-stage stops 3 to 7, steps passed 15 to 17 (noise at
one run per plan), six refusals in three runs (a test-scope JUnit import in a production file, an ambiguous
`Service`, `LocalServerPort` from the wrong package). The class of "the code does not compile because of an
import" went down; the failures behind it, behaviour tests and Spring wiring, are now what stops the runs.

### Replaying the redundancy audit (2026-10-11, `plan_replay`)

The audit for "must change in this step" (a step that modifies a class an earlier step had and covers nothing new
for it) was pre-registered and replayed over 46 stored plans and 100 executed steps. It rejected 43% of the plans
(limit 40%) and flagged 3 of the 8 refused steps (target 70%): not worth enforcing. The misses showed the real
cause: the step lists an existing test unrelated to its classes, to satisfy the required `test_classes`, and the
harness then derives `modify` for it. That pattern is in 43% of plans and in 4 of the 8 refused steps, post hoc.
Derive the tests of a step's production classes instead of asking for them; details and the proposal in
`docs/design/planner-first-increment.md`.

### Derived tests, and a loop that was in the schema (2026-10-11, `bf4009e`, `9d18a39`)

**Derived tests.** The harness now adds the test of each production class a step lists (`XTest` in the class's
package); `test_classes` is read only for a step with no classes, and `test_classes` named alongside classes is
ignored and recorded as a diagnostic. Five plans, one Implementer run each on the four valid: **0 "must change in
this step" refusals in 11 steps**, against 87 mentions in 48 steps over 19 earlier runs the same day, and none of
the four plans has a step listing an unrelated existing test in `modify` (43% of the earlier ones did). Steps
passed 2, 1, 2, 2: unchanged in size, the failure class itself gone.

**A schema with two step shapes (`oneOf`) is not usable.** It accepted derived tests for production steps and
tests-only steps in the grammar, and the server accepted it, but the free-text `instruction` looped (one sentence
repeated 62 to 151 times) in 2 of 4 plans, both attempts each. One shape, two optional lists, instead.

**Correction.** An earlier statement here and in the session that the capped planner drafts were not loops was
wrong: the check looked for repeated sentences and step ids. By repeated token, 8 of 13 capped drafts (6 of the
7 before this change) are `TestTestTestTest...` inside a `test_classes` name, because the pattern
`[A-Za-z0-9_]*Test` lets the grammar repeat `Test` without end. It was a tier-0 gap, not a model failure.
A repetition penalty (considered) would have worked on the symptom; bounding the pattern removes the
possibility. Name length, package depth and segment length, and every list are now bounded; the server accepts
the quantifiers. Ten fresh plans: **10 of 10 valid on the first draft, no capped response, no loop**, 8 to 20 s
each (the same schema produced a capped draft in 1 plan of 5 the batch before).

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

### Experiment: every earlier-created class is read-only (2026-10-11)

Change: `read_only` is every class an earlier step created plus the existing classes the instruction names (cap 8),
instead of only the mentioned ones. Three stored drafts re-derived (`rederive_plan`), 5 runs each, 15 runs, harness
`65cf0a5` and `6abed29`.

Prediction: stops on an earlier class's constructor or method fall from 3 of 3 plans to at most 3 of 15 runs;
consumer steps pass at least 8 of 15. Falsified if 5 or more of 15 still stop on an earlier class's signature.

Result: first half held, second did not. Stops on an earlier class's signature: 1 of 15 (`getWidgets()` missing on a
service) plus 2 "invalid method reference" in a repository step. Steps passed 6, 6, 5 of the 20, 21 and 40 possible,
1.1 per run against 1.3 before, so no gain. The origin was real and is closed, but it was not the dominant one.

What the stops are now (causal packet, `analyze_runs --step`): the test and the implementation disagree on a decision
nobody stated, and the test is often wrong on its own. `@SpringBootTest` appears in the traces of 4, 3 and 5 of 5
runs per plan, on plain repository and service classes, with a field `@Autowired` and then overwritten in `@BeforeEach`.
The context is shared, so the stored widgets of other tests are still there (`expected: <2> but was: <36>`).
Others: `WidgetTest` asserts a literal hash code and a `toString` it never fixed; a controller test that expects `null`
on invalid input. Chain for the repository stop: symptom `testFindAll expected 2 was 36`; proximate cause: a
`@SpringBootTest` test over a shared singleton; origin: nothing tells the model how a `*Repository` or `*Service` is
tested; earliest point: rung 1, a contract rule keyed to those layers for the implementer (and the existing
stereotype rule shows the mechanism works: services annotated 1 of 1 afterwards).

Next experiment (one variable): a contract rule for the `repository` and `service` layers, "tested as a plain unit
test: construct the class directly, no `@SpringBootTest`, no `@Autowired`", routed by name. Prediction: no
`@SpringBootTest` in repository and service tests (from about 12 of 15 runs to at most 2), and the
`expected 2 was 36` class gone. Falsified if shared-state stops remain. n = 10 runs on the same three plans.
This is a rule in the contract, not code beyond render, so it needs no gated code; the rule text needs a person's
review.

### Experiment: a contract rule for how repositories and services are tested (2026-10-11)

Change: one contract rule on the `repository` and `service` layers: ALWAYS test the class as a plain unit test,
constructed with `new` in `@BeforeEach`; NEVER `@SpringBootTest` or `@Autowired` in its test. Same three drafts,
same harness (`3ad4c8a`), 10 runs (4, 3, 3) against 15 before; only the rule differs.

Prediction: `@SpringBootTest` in repository and service tests from about 12 of 15 runs to at most 2; the
`expected 2 but was 36` class gone. Counted in the submitted files (not the trace text, which now quotes the rule).

Result: **held.** Runs whose repository or service test used `@SpringBootTest` or `@Autowired`: 7 of the 12 that wrote
one before (my earlier "about 12" counted the trace, which overstated it), 0 of the 8 that wrote one now. No
shared-state stop (`expected 2 was 36`) in 10 runs. Steps passed 15 over 10 runs, 1.5 per run against 1.1; direction
only at this n, and two of the three batches rest on 3 runs.

What stops now: the controller step. 3 of 10 stop on `Failed to load ApplicationContext` for a controller test, one
more on a missing setter the test invented (`setWidgetService`), one on truncation; the model step still stops 2 of
10 on a syntax error in a file it wrote (`')' or ',' expected`). Origin of the controller class, from the packets: the
same one, with no statement of how a `*Controller` is tested (full context versus a sliced web test, and how its
service is supplied). Next: a controller-layer rule for the same mechanism, one variable, n=10 on the same drafts.

### Experiment: a contract rule for how controllers are tested (2026-10-11)

Change: one more contract rule, on the `controller` layer: ALWAYS a web-slice test, `@WebMvcTest(<Controller>.class)`,
`MockMvc` injected, the service supplied as a `@MockitoBean`; NEVER `@SpringBootTest`. The repository and service rule
stays. Same three drafts, 10 runs (4, 3, 3), harness `beeb16d`.

Prediction: no controller test uses `@SpringBootTest`; the `Failed to load ApplicationContext` stops (3 of 10 before)
fall to at most 1; falsified at 3 or more.

Result: **held.** All 7 runs that wrote a controller test used `@WebMvcTest`, none `@SpringBootTest`, and no stop on a
context load (0 of 10). Steps passed 18 over 10 runs, 1.8 per run (1.1, then 1.5, now 1.8; direction only, n is small
and the arms share three drafts).

Stops now, one each unless stated: controller tests expecting 201 where the controller answers 200 (2 runs: a decision
about the response status that nothing states), a Java text-block syntax error in a written file (2 runs), an invented
getter on a request type, a repeated `glob` and a tool-call budget stop (planning of the step's own files), and in the
third plan, repository and service steps (a `count()` on an `Iterator`, a type inference failure, `expected 2 but was 36`).

Correction to the earlier entry: the `expected 2 but was 36` stop is not shared state. In the run where it still
occurs, the test builds a fresh repository in `@BeforeEach` and the repository holds its map in an instance field; the
36 is some other quantity the test computes. My earlier reading, from the first batch, was from the symptom and the
`@SpringBootTest` that happened to be present, not from the failing assertion. The rule's effect on `@SpringBootTest`
use stands; its effect on that particular failure does not.

Next origin to test, from the packets: decisions the test and the implementation make separately and nothing states
(response status of create, how an invalid value is reported, whether an absent entity is `null` or an exception).
Candidates are contract rules (one line each, routed by layer) or a derived fact from the feature file's scenarios.
Rung 1 first: the Gherkin feature already states the status code in its scenarios, so check whether the step is shown
the scenario it implements before writing any rule.

### Experiment: a step is shown the scenarios it covers (2026-10-11)

Change (`777c59e`): the step's context ends with the scenarios it `covers`, rendered from the parsed feature (`covers`
was ignored by the Implementer before). Same three drafts, the repository, service and controller test rules kept, 10
runs (4, 3, 3), harness `777c59e`. (A first attempt was lost when the laptop slept with the lid closed; it was
discarded and the whole experiment redone under `caffeinate`.)

Prediction: "Status expected 201 but was 200" stops fall from 2 of 10 to 0; invalid-name tests assert 400; falsified
at 2 or more status mismatches, or tests that contradict a shown scenario.

Result: **mixed, and by my own threshold not a pass.** The shown scenario reached the tests: every controller test
written (7 runs) asserts `isCreated` and `isBadRequest`, none contradicts a scenario, and "was 200" is gone (0 of 10).
But 2 of 10 runs still stop on a status mismatch, now "expected 201 but was 400" for a valid name. Steps passed 17
over 10 runs (1.7), against 1.8 before: no gain.

Chain for the status stop (packet, run `18-56-35/run-02`): symptom, `Status expected:<201> but was:<400>`; proximate
cause: the controller takes `@RequestBody String name` and returns `Widget` directly, with no `@ResponseStatus` and
the exception mapped to a `RuntimeException`; the test posts JSON; origin: the scenario states the status but not the
request shape (`a client posts a widget named "Alpha"`), and the implementation half of the step does not act on
the status it was shown. So the test now follows the scenario and the implementation does not; the two halves of
one step still disagree. Also seen: `@MockBean` (deprecated) written against the rule's `@MockitoBean` in 1 run; a
model-step test expecting a constructor to throw on an invalid name, which the implementation does not do (2 runs):
where validation lives is again a decision nothing states.

Next origin: decisions that connect the test and the implementation of the same step and are stated nowhere (request
body shape, where validation lives). The feature's wording is the stated source, so the candidate is rung 0 or 1
again: a convention rule per layer ("a create answers 201 and the created resource; a request body is a JSON
object of the fields") is a contract rule, and "validation lives in the model constructor or the service" can be
decided once in the contract. Not run.

### Experiment: two contract rules, then a line on case values (2026-10-11)

Rules added to the contract copies (same three drafts, scenario context kept): validation lives in the domain object's
constructor and throws `IllegalArgumentException` (layerless, so it also reaches the model step, which the contract
cannot classify because the class has no suffix); a controller reads a JSON object, answers a create with 201 and the
created object, and an `IllegalArgumentException` with 400.

**Arm A (the two rules), 10 runs: worse.** 9 of 10 runs stopped on "Expected IllegalArgumentException to be thrown, but
nothing was thrown" (2 of 10 before); steps passed 0.5 per run (1.7 before). Prediction falsified. The packet shows the
rule itself was followed (the constructor throws) and the test is what fails: it passes the scenario's case words as
the input, `new Widget("a name of 51 characters")`, a 23-character string. The case table cells are descriptions of an
input, not values; the scenario context added earlier put them in front of the model, and the model used them literally.
(The earlier scenario experiment had 2 of 10 of the same stop; the model step now owns the validation, so every run reaches it.)

**Arm B (same, plus one line after the cases in the step's context, only for a step that covers an outline):** "ALWAYS
write the concrete value a case describes in the test, never the words that describe it." 10 runs: "nothing thrown"
stops 9 of 10 to 1 of 10; no run passes a case description as an input; steps passed 17 over 10 runs, 1.7 per run (the
same as before the rules). Prediction held (at most 2; at least 1.7). A prompt change with a measured delta of 9 to 1 on
the same drafts, one arm each; direction is clear, effect size is not.

Remaining stops (10 runs): a controller answering 200 or 201 where the test expects 400 for an invalid name, in 3 runs of
the first plan (the `create-response` rule asks for 400 on an `IllegalArgumentException`; present in the context, not
followed: a compliance gap, not a missing fact); syntax errors in written files ('unclosed string literal', `')' or ','
expected`: 3 runs); and in the repository step of the third plan: `testFindAll` expected 2 but was 36 / 5, and
`expected Alpha but was Beta`.

**Settled: the 36.** The earlier "shared state" reading and the later "some other quantity" were both unresolved. It is
`assertEquals(2, allWidgets.iterator().next().getId().length())`: the test of `findAll` has no size to compare, because
the repository returns an `Iterable`, so the model compares the length of the first id (36, a UUID) to the count it
meant. Origin: the return type of a method that returns several objects is unstated; the test and the implementation
choose separately. Rung 1 candidate: a contract rule on the repository and service layers ("a method that returns
several objects returns a `java.util.List`"); the Roots Java signatures item is the general fix and is still parked.

### Roots Java surface, shown instead of the source of read-only files (2026-10-12)

Built (`74c112c`, `c83190d`): `roots surface <file>...` reads a Java file's declarations as written: types including
records with their components, annotations with arguments (type, method, parameter), modifiers, supertypes, fields,
constructors, methods with return types and `throws`; text rendering omits private members. External command, JSON, no
index and no schema change. Replayed over 41 distinct production files the model wrote: 0 errors, 0 empty. The
Implementer can show a step the declarations of its read-only Java files instead of their source (`--surfaces`, off by
default, in `batch.json`).

Experiment: the same three drafts and contract as the 1.7-steps arm, `--surfaces` on, 10 runs (4, 3, 3), declarations
present in the prompt in the 9 runs that had a read-only file. Prediction: steps passed at least 1.7 (no harm) and
smaller consumer prompts; falsified below 1.4 or with more symbol stops.

Result: **falsified.** Steps passed 12 over 10 runs, 1.2 per run (1.7 before). Stops on a symbol the step got wrong
(constructor arity, `cannot find symbol` on a method or class of an earlier class): 4 of 10 (about 1 of 15 and 1 of 10 in
earlier arms). Prompt size was not measured.

Chain (packet, run `06-51-19/run-03`, service step): the declarations were right (`public Widget(String name)`,
`public String getId()`), and the service called `new Widget(id, name)`. Origin: the declaration drops what the body says.
In the source the constructor assigns the id (`UUID.randomUUID()`); in the declarations `getId()` exists and nothing says
who sets it, so the model supplies one. A surface states what can be called, not what the code does, and for a small class
the body is the shortest statement of its behaviour. Same shape in the `setId` stop.

Reading: surfaces are a size tool, not an accuracy tool. They fit a class whose source is too large to show; for the
small classes of a greenfield task they remove information the model uses. Not tuned on widgets: no threshold was
chosen on this data. Next, where the value should be, is brownfield: the Planner's facts today list existing classes by
name only, and a step on a large existing class cannot be shown all of it. A task with existing code of realistic size
is the test; a size rule (source below a limit, declarations above) is the candidate, and it needs that task to be set.

### Experiment: a contract rule for collections (2026-10-12)

Rule on the repository and service layers: ALWAYS return a `java.util.List` from a method that returns several objects.
Same three drafts and contract as the 1.7-steps arm, 10 runs (4, 3, 3), harness `eafd8be`, surfaces off.

Prediction: the third plan's repository step passes in at least 2 of 3 (stopped on `findAll` in 3 of 3 before); steps
passed at least 1.9; falsified if 2 or more of those 3 still stop on `findAll`, or steps passed below 1.7.

Result: **held.** Steps passed 20 over 10 runs, **2.0 per run** (1.7 before; 1.1, 1.5, 1.8, 1.7, 2.0 across the
experiments of the last two days, same three drafts, one change each). The third plan's repository step passed in 3 of 3
and all three reached its controller step. In the submitted repository and service files `Iterable<` appears 0 times and a
`List<...>` returning `findAll`/`getAll`/`list` 21 times. Direction only; n is 10 on three drafts.

Remaining stops (10 runs): a controller answering 200 or 201 where the test expects 400, or 200 where it expects 201:
5 runs, with the `create-response` rule present in the context and not acted on (a compliance gap; nothing in the code
or the harness could make Spring map an `IllegalArgumentException` to 400 unless the model writes the handler); one
compile stop from a deprecated `@MockBean` warning (the rule says `@MockitoBean`); and 3 runs of the second plan at the
service step with "Expected IllegalArgumentException to be thrown, but nothing was thrown".

Chain for the service stops (packet `08-16-32/run-01`): the service test expects an exception for a 51-character name; the
service rejects only null and blank and otherwise calls `new Widget(name)`; the validation rule says the constructor
validates and the service does not repeat the check; the plan's model step did not state the length limit, so no
constructor checks it. The limit lives in the scenarios, and the model step covered only some. Origin: which step is
responsible for which constraint is a plan decision, and the rule that moved validation into the model made that decision
matter. Not run: show the model step the constraints of the outline cases (it covers the scenario, and the case table is
in its context), or check at plan time that the step that owns a validation covers the outline.

Note on direction. Five rules and one context line produced this sequence on one task. They are all of one kind (a
decision made separately by the test and the implementation of a step, stated nowhere), which is the concern that the
conditional hints will not stop. Whether they transfer is the measurement to make next (a different task under this
contract, counting the new rules it needs), alongside the stub-first alternative that removes the decision instead of
stating it.
