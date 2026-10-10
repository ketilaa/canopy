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

Read what is there before writing a one-off script. If a question needs a script twice, it belongs in
`report.rs` or `analyze_runs`, with a test.

## The method

1. **Group, do not narrate.** Describe failures as recurring classes with a frequency, never run by run.
   A class counts once per run. Say how many runs and how many batches the count rests on.
2. **Find the dominant remaining cost.** Assume solved problems are solved. The next experiment attacks
   the largest class that survives, not the most interesting one.
3. **Separate who failed.** Planning failure (the plan was invalid or ambiguous), implementation failure
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
| Prompt wording | "state the concrete implementation choices" | see above |
| Prompt wording | "every step carries its test" | ignored in 8 of 10 drafts, retry included |
| Added context | facts and dependency list stamped into every step | no visible effect alone (JPA classes still written) |

Pattern: removing or tightening beat instructing, and an instruction for something the model should decide
did not hold either; the shape carried it. Validity saturates quickly (3/3), after which only downstream
results (steps the Implementer passes) measure anything.

Ambiguity hotspots:

| Phrase in a plan | What went wrong | Resolved by |
|---|---|---|
| "entity", "repository interface" | the Implementer wrote JPA classes the project does not have | the storage fact plus asking for the concrete mechanism (record, collection class) |
| a controller step with no stated test style | the model wrote an HTTP-level test that never names the controller; the no-test gate refused it 5 to 10 times (6 of 7 stuck runs) | open: a gate decision, see the report |

## Things the analysis must not assume

Known server and harness facts (cite them; do not re-derive):

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
3. **What is already solved**: one line, so it is not revisited.
4. **Hypothesis** about the cause, marked as hypothesis or confirmed, and which intervention mechanism it
   points to.
5. **One next experiment**: the change, the smallest n that can discriminate, the prediction, and what result
   would falsify the hypothesis. Low cost, one variable.
6. **Needs approval?** yes or no, and why.
7. **Gaps**: observability the analysis lacked, if any.

An answer that cannot name its evidence says "insufficient evidence" and asks for the missing observation
instead of recommending.
