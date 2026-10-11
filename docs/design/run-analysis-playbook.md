# Run-analysis playbook

How to read a batch of Planner or Implementer runs and decide what to do next. Written 2026-10-10 from the
first Planner hardening session (`docs/reports/widgets-planner.md`), so that what that session learned is
used instead of re-derived. The `canopy-run-analyst` subagent (`.claude/agents/canopy-run-analyst.md`)
follows this file; a person can follow it by hand. The tooling that produces the numbers lives in the
repository and is meant to travel with the harness when the Implementer and Planner are merged into
`canopy-cli`; the subagent does not.

## Tools

```
cargo run -p canopy-implementer --example analyze_runs -- [--json] <batch dir>...
cargo run -p canopy-implementer --example analyze_runs -- --step <run dir> <step id>
```

Reads stored Planner batches (`canopy-planner/runs/<batch>/run-NN/summary.json`) and Implementer batches
(`canopy-implementer/runs/<batch>/run-NN/events.jsonl`) and prints: per-run outcomes, the Implementer runs
joined to the plan they ran (through `implementer-runs.jsonl` beside the plan), failure classes ranked by
frequency with names, quotes and paths taken out, an attribution line (planner vs Implementer) and the
dominant class. The same report is in every Implementer `summary.json` under `report`
(`canopy_implementer::report::RunReport`). Other evidence, when the summary is not enough:

- `events.jsonl`: every request, response, tool call and its result, in order. `step_started` carries the
  context and scope the step was given.
- `plan.meta.json` beside a generated plan: its content id, and which step fields the model wrote versus the
  harness derived.
- `attempt-N.json` / `attempt-N.problems.json`: what the Planner's model answered and what was wrong with it.
- `batch.json`: harness commit and whether the tree was dirty, server properties, sampling, task.

`--step` prints the **causal packet** of one step of one stored run, in causal order: the instruction, the
scope the harness derived (create, modify, read_only), the context lines, the files earlier steps left, the
tool calls, each submission's head, every refusal in full and each failed attempt. Open it for every failure
class before naming a cause (method 3).

Read what is there before writing a one-off script. If a question needs a script twice, it belongs in
`report.rs` or `analyze_runs`, with a test.

## The method

1. **Group, do not narrate.** Describe failures as recurring classes with a frequency, never run by run.
   A class counts once per run. Say how many runs and how many batches the count rests on.
2. **Find the dominant remaining cost by origin, not by symptom.** Assume solved problems are solved. Count
   symptoms first, then trace each class to its origin (method 3) and re-rank by origin: several symptoms
   often share one origin, and the largest symptom is often only where it shows. The next experiment
   attacks the largest origin that survives, not the most interesting one.
3. **Trace the causal chain; the symptom is not the cause.** For every failure class write the chain, with a
   quoted line from the trace at each hop: symptom (where the run stopped) → proximate cause (what the model
   did or the gate said) → origin (what the model was given, or not given, that made that likely) → the
   earliest point at which it could have been made impossible, and at which escalation rung (0 make it
   unrepresentable, 1 compute or look it up, 2 audit and recover, 3 teach). Rules for the chain:
   - **First defect.** Walk back to the first step, file or field that was already wrong, not the last. A
     step that fails on `new Widget(1L)` has its origin in the step that wrote `Widget`, or in the step that
     was not shown it.
   - **Read raw outputs.** Open at least three raw submissions or drafts per class, capped ones included,
     and check them for repeated tokens. A counted label is not a reading.
   - **Rung 0 and 1 before a gate.** Before recommending a gate, audit or retry, say why the wrong answer
     cannot be made unrepresentable (schema, derived field) or looked up (a fact, a tool). Most classes in
     the calibration table below were closed at rung 0 or 1.
   - **Two origins are two classes.** A symptom with two origins is split and counted twice.

   Then separate who failed. Planning failure (the plan was invalid or ambiguous), implementation failure
   (the model wrote bad code for a clear step), execution failure (the harness, a gate, the environment).
   Never attribute a downstream failure to the Planner without evidence from the trace. In this project a
   stop point blamed on "unstated wiring" was in fact a gate refusing a correct test; the attribution line of
   `analyze_runs` exists because of that.
4. **Treat every model-authored field as suspect.** Ask of each: if two perfect planners solved the story
   differently, would this field differ? If not, it is derivable, and the harness should derive it (from the
   repository, from earlier steps, from the feature). A plan should describe intent: order, implementation
   choices, tests, responsibilities. It should not carry bookkeeping: kinds, file inventories, create or
   modify, dependency lists, paths. Principle and evidence: `docs/principles/compute-facts-mechanically.md`
   (extension of 2026-10-10).
5. **Look for the missing decision.** When the Implementer makes a bad choice, ask "what was left
   unspecified?" before adding a rule. A missing implementation decision is usually fixed by stating the fact
   or removing the choice, not by a new instruction.
6. **Classify every intervention** that changed a number: schema tightening, derived fact, removed model
   choice, added context, or prompt wording. Keep the table below current; it is the evidence for what works.
7. **Information present is not information consumed.** When context is added, say whether the output
   demonstrably reflects it. If not, write "present, consumption unknown".
8. **Keep an ambiguity catalogue** (below): phrases in plans that repeatedly lead to divergent
   implementations. Build it from differences between runs of the same step, not from intuition.
9. **Recommend exactly one next experiment** (output format below).

## What the evidence says so far

Interventions by mechanism (Planner, widgets story, Qwen3-14B Q4_K_M; sample sizes are small, so read these as
direction, not effect size; sources are the batches in `docs/reports/widgets-planner.md`):

| Mechanism | Intervention | Result |
|---|---|---|
| Removed model choice | step `kind` (the model labelled every step `refactor`) | the hole in the test rule closed; no regression |
| Removed model choice | create or modify, set by the harness from the repository and earlier steps | valid plans 1/10 to 4/10 |
| Removed model choice | `read_only` computed from the class names an instruction mentions | the 38/38 duplicates ended; downstream steps unchanged |
| Removed model choice | paths derived from qualified class names | 3/3 valid first drafts; downstream unchanged |
| Schema tightening | required non-empty `tests` | missing tests 8/10 to 0/10 |
| Schema tightening | path patterns by directory | a test listed as production code ended |
| Derived fact | computed storage line ("no database or JPA library is available") | with the next row, JPA mentions 30 to 0; the two are stacked |
| Removed model choice | the tests of a step's classes derived (`XTest` per class) instead of named | "must change in this step" refusals 87 mentions in 19 runs to 0 in 4; the unrelated-test pattern 43% of plans to 0 |
| Schema tightening | every part of a class name and every list bounded | token-loop drafts (8 of 13 capped) to none in 10 plans; 10 of 10 valid on the first draft |
| Schema tightening (failed) | `oneOf` for two step shapes | the free text looped in 2 of 4 plans; reverted |
| Prompt wording | "state the concrete implementation choices" | see above |
| Prompt wording | "every step carries its test" | ignored in 8 of 10 drafts, retry included |
| Added context | facts and dependency list stamped into every step | no visible effect alone (JPA classes still written) |
| Derived fact | computed "no bean validation library is available" line | `javax.validation` compile errors 2 of 7 runs before, 0 of 3 after; steps passed not better |
| Added context | a Spring stereotype convention by class name, in every step's context | 1 of 1 services written afterwards was annotated, against 0 of 2 before; "no qualifying bean" 0 of 3; steps passed no better. Direction only |

Pattern: removing or tightening beat instructing, and an instruction for something the model should decide
did not hold either; the shape carried it. Validity saturates quickly (3/3), after which only downstream
results (steps the Implementer passes) measure anything.

Calibration: symptom against origin. Each row was first read at the symptom and later traced to an origin that
was somewhere else. Use them to check a new chain: if the origin you name is the same place as the symptom,
look one hop further back.

| Symptom (where it showed) | Origin (where it came from) | Earliest point it was impossible |
|---|---|---|
| a controller step stopped on "no qualifying bean" | the service step before it did not annotate its class | rung 1: a stereotype convention by class name in the step's context |
| "must change in this step" refused 87 times | the plan named an unrelated test in a modify step | rung 0: tests derived from the production classes, not named |
| `TestTestTest...` to the token cap in 8 of 13 drafts | an unbounded pattern in the grammar | rung 0: bounded quantifiers |
| a service step guessed an earlier class's constructor | `read_only` only held the classes the instruction mentioned, so the step never saw the class | rung 1: every earlier-created class is read-only context |
| a hint told the model to change a file the step may not change | the hint knew the subject class but not the step's scope | rung 1: the hint takes the scope |

Ambiguity hotspots:

| Phrase in a plan | What went wrong | Resolved by |
|---|---|---|
| "entity", "repository interface" | the Implementer wrote JPA classes the project does not have | the storage fact plus asking for the concrete mechanism (record, collection class) |
| a controller step with no stated test style | the model wrote an HTTP-level test that never names the controller; the no-test gate refused it 5 to 10 times (6 of 7 stuck runs) | open: a gate decision, see the report |

## Things the analysis must not assume

Known server and harness facts (cite them; do not re-derive):

- A `pattern` with an unbounded `*` over a repeatable token lets the model loop (`TestTestTest...`) until the token cap;
  bound every quantifier (`{0,n}`; the server accepts them). A `oneOf` in the response schema made the free-text fields
  loop; use one shape. Check a capped output for repeated tokens, not only repeated sentences.

- A response schema and `tools` must never be sent together; the schema wins and tool calls become impossible
  (`CLAUDE.md`, escalation order).
- llama-server compiles the response schema to a grammar. `enum`, `minItems`, and `pattern` (including
  grouped alternatives) are enforced and were accepted in every Planner batch. A `description` in the schema
  is not shown to the model (believed from how the grammar is built; not measured here), so a rule must be in
  the prompt, next to what it governs.
- A refusal inside the tool conversation costs a tool call, not an attempt; a guard can itself be the failure
  (`docs/retrospectives/2026-10-10.md`). Read the gate's refusals before the model's code.
- The environment is a variable: JDK 21 for Maven, the same server command per batch
  (`canopy-implementer/docs/models/qwen3-14b-q4_k_m.md`).
- Ten runs per arm cannot separate small differences. Say so, and recommend the smallest n that can.

## Rules the recommendation must respect

- Prompt changes need a measured delta; a paired comparison beats one sample.
- Rust code that rewrites the meaning of model output is forbidden; deterministic audits, restore and retry,
  and rendering from validated fields are allowed (`CLAUDE.md`, prompt house style). A recommendation that
  adds code beyond that is a proposal for a person to approve: say so explicitly.
- Never name a dogfooding project or infrastructure detail in anything committed.

## Output format

At most one page, in this order:

1. **Dominant remaining cost**: the class, its frequency, the runs and batches behind it.
2. **Attribution**: planner, implementation or execution, with the trace evidence (quote the line).
   Give the causal chain for the dominant class (symptom, proximate cause, origin, earliest point, rung),
   a quoted line at each hop. Name any other class that shares the origin.
3. **What is already solved**: one line, so it is not revisited.
4. **Origin and earliest point it was already wrong**: the first defect in the chain, and the rung at which it
   could have been made impossible. If your recommendation is a gate, audit or retry, say why rung 0 and 1
   do not apply.
5. **Hypothesis** about the cause, marked as hypothesis or confirmed, and which intervention mechanism it
   points to.
6. **One next experiment**: the change, the smallest n that can discriminate, the prediction, and what result
   would falsify the hypothesis. Low cost, one variable.
7. **Needs approval?** yes or no, and why.
8. **Gaps**: observability the analysis lacked, if any.

An answer that cannot name its evidence says "insufficient evidence" and asks for the missing observation
instead of recommending.
