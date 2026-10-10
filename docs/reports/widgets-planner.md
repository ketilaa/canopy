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
