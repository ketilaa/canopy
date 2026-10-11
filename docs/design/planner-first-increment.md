# The Planner, first increment: story + Gherkin in, an Implementer-ready plan out

Status: proposal, 2026-10-10. Nothing here is built. Starts option 2 of
`docs/open-questions/implementer-and-planner-into-the-canopy-repl.md` (standalone Planner, built and
measured on its own, the way the Implementer was). Not a REPL change: the REPL reads the plan format this
increment produces, and is designed afterwards.

## The question this increment answers

Can the local 14B model produce a plan the Implementer passes as often as it passes the hand-written
one (widgets v6: 10/10)? The hand-written plans carry knowledge a small model might not produce: which test
inputs are invalid, how a test wires a service, the exact setup line, a step small enough for the
2048-token completion cap. Every plan fix so far added one of those (`docs/retrospectives/2026-10-10.md`).
The increment is done when we can say, from paired sweeps, which of those the model gets right unaided and
which the harness has to supply.

## Scope

In:

- Input: a story (prose) and a Gherkin feature file, for one story, against one existing Java / Spring Boot /
  Maven repository.
- Output: `plan.yaml` in the format `Plan::from_yaml` already reads, plus a `covers` field per step, plus a
  run directory with every intermediate artifact.
- One benchmark: the widgets task. The hand-written plan is the golden reference; the hidden HTTP test is the
  score. The Gherkin already exists, embedded in the plan's `story:`; it moves to
  `tasks/widgets/feature.feature`.
- An example runner (`run_planner`), not a REPL command.

Out (each is a later increment, with a trigger):

- REPL integration, `spec`/ADR changes, story artifacts. Trigger: the plan format stops changing.
- Repository exploration by tool calls. Trigger: the second benchmark (`vat`, a modify-heavy plan against an
  existing repository). In this increment the repository is a greenfield scaffold, so a compact facts block is
  enough.
- Other stacks. The Spring layers are one stack profile, kept as data so a second can replace it.
- Planning from a vague story. The Gherkin is the acceptance criteria; if it is missing or unparseable the
  Planner refuses.

## What the Planner produces

The existing `plan.yaml`, unchanged in meaning, with two additions:

```yaml
id: widgets
story: <story text>                # kept for the record; the Implementer is never shown it
feature: tasks/widgets/feature.feature
steps:
  - id: widgets-1-domain
    instruction: ...               # what the Implementer is told
    kind: behavior_change
    create: [...]
    modify: [...]
    read_only: [...]
    covers: [valid-name-creates-a-widget-with-a-generated-id]   # new: scenario ids
```

`covers` ties each step to the scenarios it makes pass. It is the only new field the Implementer ignores and
the verifier reads. A human approves the plan by reading it; `plan.md` is rendered from the YAML for that, not a
second source of truth.

## Pipeline

```
story + feature.feature + repository
        │
  1. parse the Gherkin                    deterministic
  2. collect repository facts             deterministic   (also becomes the Implementer's project context)
  3. draft the plan                       one schema-constrained model call
  4. verify the draft                     deterministic
  5. on failure: one retry with the named problems, then stop and report
        │
  plan.yaml  +  plan.md  +  run directory
```

Following the escalation order: the schema makes malformed output unrepresentable (tier 0), facts are
computed rather than asked for (tier 1), the verifier checks ground truth and the retry names what is wrong
(tier 2), and prompt text is last and measured (tier 3).

### 1. Gherkin parser

Own small module (Feature, Background, Scenario, Scenario Outline with Examples tables, steps; tags and
doc strings ignored). Every scenario gets a stable id: a slug of its title, with the outline row appended for
expanded rows. Output is a typed `Feature`. Errors name the line. An outline expands to one entry per Examples
row, with the cell text kept verbatim, because the rows are the bounded set the model must account for
(house style: enumerate, do not review holistically).

### 2. Repository facts

Computed, never asked: build tool, `java.version` and Spring Boot version from `pom.xml`, the dependencies
that matter (web starter, test starter), the root package from the `@SpringBootApplication` class, the
source and test roots, and the file tree under `src`. Rendered as the same text that is today hand-written as
`context` in `run_step.rs`; the Planner stamps it into every step so the Implementer's project context stops
being per-task prose. Java rules stay in `java.rs`.

### 3. The draft: schema and prompt

One structured call, thinking off, `response_format` only (never with `tools`). What the model writes is
intent; what the harness can derive is not asked of it (principle:
`docs/principles/compute-facts-mechanically.md`, extension of 2026-10-10). Per step the model gives:

- `id` and `instruction` (free text: the part that carries the knowledge, and the part the sweeps measure);
- `classes`: the production classes it creates or changes, as qualified names, grammar-constrained to the
  root package;
- `test_classes`: required and non-empty, each name ending in `Test`, so a step without a test cannot be
  written;
- `covers`: at least one scenario id, an enum of the ids parsed from the feature.

The harness supplies everything else: the plan id, story and feature path; every step's kind (always a
behaviour change; a refactor step is exempt from the test rule, so the model may not declare one); the
repository facts as every step's context; the source path of each class (`java.rs`); whether each file is
created or modified (modified when the repository has it or an earlier step creates it); and what a step
reads (every other production class its instruction names as a whole word). A draft that sets any of these
is rejected. How the shape got here, batch by batch: `docs/reports/widgets-planner.md`.

The prompt gets the story, the enumerated scenarios (outline rows listed), the facts block (build,
dependencies, a computed storage line, existing classes by name) and the step contract in the house style
(ALWAYS rules next to what they govern). The hand-written widgets plan is **not** the example, because it
would make the benchmark a recitation; no concrete example nouns.

### 4. The verifier (the main deliverable of the deterministic half)

Before any model time is spent on the plan, reject it when:

- it fails `Plan::from_yaml` or `Plan::verify_against` (both exist);
- a scenario id is covered by no step, or a step covers none (new, in `plan.rs`);
- a `create` path is not under the source or test root, or its package directory disagrees with the root
  package;
- a production class is created by a behaviour step with no test created or modified in the same step (the
  Implementer's no-test gate would refuse it later; refusing now is cheaper);
- a step lists more than N files (starting point: 4, taken from the widgets controller split), a proxy for the
  completion cap;
- a `read_only` file is not created by an earlier step or present in the repository (already covered by
  `verify_against`).

On failure, one retry whose text names each problem and the step it belongs to. A second failure ends the run
as `invalid`, with the draft kept.

These are audits: they accept or reject, they never rewrite the model's output.

## Arms

The open question is whether the model can fill the plan unaided, so the increment measures it directly
rather than assuming recipes are needed.

- **Arm A, free instructions** (above): the model writes the instruction text. Cheap to build. It is the
  baseline.
- **Arm B, recipes and slots** (only if A fails): the harness renders instruction text from validated fields,
  per layer, from a stack profile. The model fills slots, harness-enumerated from the Gherkin: one test case
  slot per Examples row, the row text copied verbatim, the model supplying the concrete value. Test wiring
  (`new WidgetController(new WidgetService(new InMemoryWidgetRepository()))`) is rendered from the dependency
  order of the steps, not generated. This moves the three recurring costs (invalid inputs, wiring, step size)
  out of the model.

Arm B is not part of this increment's build list; it is the next increment if the failure analysis of A
says so. That is the escalation order applied to the Planner itself: do not build the recipe layer until the
simpler arm has been measured and its failures read.

## Measurement

Ground truth is the Implementer plus the hidden test, not how the plan reads.

| Level | Measure | Cost |
|---|---|---|
| Plan | valid on the first draft; valid after one retry; which verifier rule rejected it | no Implementer run |
| Plan vs golden | per step: same layers, same file paths, every Examples value present as a concrete test case | mechanical, no model |
| End to end | the Implementer runs the generated plan; hidden test passes; steps passed on first attempt | one Implementer run per plan |

Sweep shape: n=10 plans, one Implementer run each, on the GPU server. Ten plans cost less than ten runs of
one plan and test the variation that matters. Compared with the standing baseline (hand plan, n=10: 10/10).
Ten per arm cannot separate small differences (the retrospective's own caveat), so the claim is limited to
"clearly short of the baseline, or not".

Record the harness commit, server properties and `mvn -version` in every batch, as `run_step` does. Do not
change the JDK, the prompt or the verifier mid-sweep. Add a Planner section to the model file; failures are
classified by which of the three recurring costs they are (wrong inputs, wrong wiring, oversize step) before
anyone reads the code the Implementer wrote.

## Where the code lives

A new crate, `canopy-planner`, depending on `canopy-implementer` for `Plan`, `Generator`, `LlmClient`, the
observer and the JDK/package helpers. The dependency points one way (the Implementer never knows the Planner),
and the Planner cannot drift from the plan format because it uses the same type. `canopy-implementer` keeps
"no canopy dependencies"; the new crate is another isolated one, not yet used by `canopy-cli`. `CLAUDE.md`'s
codebase-structure section gets a line when the crate exists.

Changes to `canopy-implementer`: `PlanStep.covers` (optional, defaulted), `Plan.feature` (optional), and a
`Plan::verify_coverage(&[ScenarioId])`.

## Build order (TDD, red then green, a commit at each green checkpoint)

1. **Move the widgets Gherkin out of the plan's `story:`** into `tasks/widgets/feature.feature`; add `covers`
   to the hand plan; the existing "every plan we ship is valid" test learns to check coverage. Red first: the
   test fails until the hand plan declares its coverage.
2. **`canopy-planner` crate and the Gherkin parser.** Tests: the widgets feature, an outline's rows expanded
   with verbatim cells, a missing Examples table, a malformed line (error names the line), stable scenario ids.
3. **Repository facts.** Tests against `tempfile` repositories, then once against `dogfooding/java-widgets`
   at `baseline-00-widgets`; the rendered text is compared with the context `run_step.rs` has today.
4. **The verifier.** One test per rule, each a plan that breaks only that rule, plus the hand-written
   widgets plan as the passing case (it must verify against the baseline repository).
5. **The draft call.** Schema built from the parsed scenarios (so the `covers` enum changes per feature); a
   scripted `Generator` returns canned drafts: good, malformed, uncovered scenario, over-wide step. Assert the
   retry text names the problem and that two failures end as `invalid`.
6. **`run_planner` example.** `--task widgets --runs N --url`, writes the run directory (parsed feature, facts,
   every request and response through the observer, draft, verification result, `plan.yaml`, `plan.md`), and
   `--then-implement` hands the plan to the existing `Orchestrator` and scores with the hidden test, reusing
   `run_step`'s reset and scoring; extract that part rather than copy it.
7. **Sweep, then write up.** First a plan-only batch (no Implementer time) to read the verifier's rejections;
   then n=10 end to end. Results into the model file, a report under `docs/reports/`, and the open-question
   entry updated. A retrospective if the day crosses the project-level milestone (first model-written plan
   executed).

Steps 1 to 4 need no model and can be finished and committed before the GPU server is involved.

## Decisions to make before step 5

1. **Own crate or a module in `canopy-implementer`?** Recommended: own crate (above). Cheaper alternative: a
   `planner` module in the Implementer crate, which keeps one `Cargo.toml` but makes the Implementer depend on
   the Planner's world.
2. **Thinking on or off for the draft?** The Implementer runs thinking off for schema-constrained work. Planning
   is more judgement than transcription, and a thinking-on arm may be worth one batch. Recommended: start with
   off for comparability, add one thinking-on batch if A falls short.
3. **A second Gherkin-bearing benchmark.** Widgets alone risks tuning to one story. `vat` needs a Gherkin
   written for it (the existing task is one sentence). Recommended: write it now but run it in the next
   increment, together with exploration tools.

## Risks

- **The benchmark is tiny.** One story, one stack, a greenfield repository. A pass says the pipeline works,
  not that planning is solved; the writeup must say so.
- **Gherkin prose is not values.** "a name of 51 characters" has to become a concrete test input, and no
  mechanical check proves the model did it right. The end-to-end hidden test is the only check; Arm B's slots
  make the gap explicit.
- **Contamination.** The widgets hand plan sits in the repository the model is not shown, but a prompt example
  that resembles it would leak the answer. Placeholders only.
- **Verifier as the failure.** The no-test gate once refused a correct submission and sent the model into a
  read loop. A verifier rule that rejects good plans will do the same, so every rejection in the plan-only
  batch is read before the end-to-end sweep.

## Pre-registered: the redundancy audit replay (2026-10-11)

Candidate rule: a step that modifies a production class an earlier step of the same plan created must cover at
least one scenario that no earlier step touching that class covers; otherwise it is redundant (the Implementer
refuses such a step with "must change in this step", having nothing new to change). Replay over the stored plans
before it is enforced anywhere. Thresholds, fixed before the run:

- It would reject at most 40% of the valid stored plans (above that it is not a gate; audit (d) rejected 9 of 10).
- Of the executed steps that were refused with "must change in this step", at least 70% are flagged.
- Of the flagged steps that were executed, at least 50% hit that refusal or failed.
