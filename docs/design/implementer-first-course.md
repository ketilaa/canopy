# Implementer first: the course from 2026-10-17

Status: active. This is the plan to resume from, written so that a session with no memory of the work can
pick it up. Read, in order: the "Prompt Style and Working Course" section of `CLAUDE.md`, this file,
`docs/reports/order-plain-java.md` (the evidence), `docs/principles/prompt-house-style-legacy.md` (the retired
rules, kept as knowledge).

## Why we changed course

A standalone probe showed the 14B model writing a correct class and its tests in one shot from a short
prompt, while the Planner and Implementer, built up under the Prompt House Style, struggled. Experiments on a
plain-Java order story (`docs/reports/order-plain-java.md`) found:

- Plan **validity** is gameable: tightened schemas took first-draft validity from 4/10 to 10/10 and the
  Implementer pass rate stayed at 0 to 1 in 10. Validity is not a success metric.
- The largest Implementer bug was two prompt rules contradicting each other (read files / never look for
  files on a retry). Removing them moved the single step from 0/10 to 4/10.
- Plans fail by compounding: a step passes first time about half the time and a failed first attempt
  recovers about one time in six, so a 4 to 7 step plan almost never completes. On this small story the single
  step (4/10) beat every plan arm (0/20).

The Planner was therefore being tuned against a downstream stage that is not reliable, and the house style
had become the default answer. The house style is retired as rules; the solution space is open.

## The course

1. **Implementer first.** Strip down to the bare bones until the Implementer passes 10/10 (n=10) on a
   hand-written plan for the order story. Exit criterion for this step: 10/10 scored on that plan, in two
   separate batches, with the first run's output read before the batch is reported.
2. **Generalize.** Only after step 1: add a second story and vary one plan factor at a time to learn what
   a good plan and a good plan step are (see "Factors" below). Exit criterion: a stated, measured description
   of a plan shape the Implementer passes on both stories.
3. **Planner last.** Teach the Planner to produce the plans that were measured to pass, scored by whether
   the Implementer passes them. Not started; do not touch the Planner before steps 1 and 2 are done.

## Step 1 in detail (where to start)

Baseline to beat: single step 4/10 scored with the retry fix; plan runs 0/20.

1. **Write the order plan by hand** at `canopy-implementer/tasks/order/plan.yaml`, in the format of
   `canopy-implementer/tasks/widgets/plan.yaml` (the hand plan that reached 10/10 on the widgets story). Start
   coarse (2 to 3 steps), put each behaviour in the step that implements it, and write each instruction the
   way the widgets plan does. Run it with
   `run_step --task order-planned --plan canopy-implementer/tasks/order/plan.yaml --runs 10 --url <model server> --model Qwen/Qwen3-14B-GGUF:Q4_K_M --out <dir>`.
2. **Log every prompt/plan change** in `canopy-implementer/docs/prompt-and-plan-changelog.md` (what, why, evidence, status).
   **Classify every failure from the traces** (`events.jsonl` in each run directory: `attempt_failed` events
   carry the stage and feedback). Fix by subtraction first: read the system prompt, the step prompt and the
   retry prompt together for rules that contradict each other or tell the model to do something pointless in
   that situation. Change one thing, re-run n=10, keep it only if the pass rate moves.
3. **Known candidates, none yet tried:**
   - The first-attempt rule "ALWAYS use glob and read_file to find and read the files you need" is wrong in an
     empty repository (3 of 20 single-step first attempts looped on `glob`).
   - A different interaction protocol, like the probe's: persistent files plus a `check` tool that returns
     compile and test errors, instead of an atomic `submit_files` with rollback and a 12-call budget.
   - Context window: the model server runs with 8K; a retry prompt carries the previous answer.
   - How the retry prompt presents errors; whether a failed attempt should keep its files.
   - The test-first step order (red phase) and its gates: whether each gate pays for itself.
4. **Also measure the single step at n=30** with one binary, to know the real number behind 4/10.

## Factors for step 2 (one at a time)

Step size (files per step); where behaviour sits (data class vs service); instruction detail (one line,
signatures, signatures plus test values); whether a step names its tests; test-first or implementation-first
inside a step; number of steps (1, 2, 3, 6) for the same story; whether scenario text is in the step context
(the scoping experiment showed no effect, so this is low priority). Use the order story and the widgets story.

## Practicalities

- Build and run: `cargo` is not on the default PATH in the maintainer's shell; the toolchain directory has to be
  added to PATH. Examples are built with `cargo build -p canopy-implementer --example run_step` (and
  `-p canopy-planner --example run_planner`). **Rebuild after any rename or edit and check the binary
  (`strings`) before launching a batch; read the first run's output before calling it running.** A stale
  binary once ran 20 no-op runs.
- The model: Qwen3-14B Q4_K_M on a llama-server on a GPU-accelerated server, 8K context, one request at a time
  (so batches run one after another). The address and the start command are in the maintainer's local notes,
  not in this repository. The server is started with nohup, not as a service, and restarting it needs the
  owner's browser-approved SSH, so a batch cannot always be started unattended. Check `/health` first.
- The order repository lives under `dogfooding/` (git-ignored): a Maven project, Java 21, groupId
  `no.experis.orders`, JUnit 5 only (junit-jupiter-api and -engine 5.10.0, compiler plugin 3.11.0, surefire
  3.0.0), empty `src/main/java` and `src/test/java`, one commit tagged `baseline-order-001`. Recreate it that
  way if it is missing. The task definitions are in `canopy-implementer/examples/run_step.rs`
  (`order-planned`) and `canopy-planner/examples/run_planner.rs` (`order`); the story and feature are in
  `canopy-implementer/tasks/order/feature.feature`.
- Scoring for `order-planned`: every step passes its gates, the whole suite is green, at least 4 tests were
  added, and `pom.xml` is untouched. There is no API-independent hidden test (plans choose their own names).
- **Every run is shown in full.** The maintainer must see the complete prompt and conversation of every run,
  not only summaries. Render a run with `target/debug/examples/transcript <run dir>` (events.jsonl as a readable
  transcript: the full system and user prompt of each attempt, every model turn and tool call, every tool
  result, every gate verdict, the failure feedback) and print it as runs finish, before interpreting results.
  Check the first run of a batch before launching the rest.
- Method: paired runs, n=10 per arm, one change at a time, a written prediction and falsification condition
  before each batch, artifacts read before conclusions. The model is stochastic: single runs prove little (one
  plan scored 6/6 in one batch and 2/6 in another).

## State of the repository when this was written

- `master`: the Planner and Implementer run plain Java (`46287ad`), the Implementer retry rule fix
  (`502d7ff`), the report (`49a7c3f`), then the deprecation of the house style (this change). Commits are not
  pushed. The Co-Authored-By line missing from the first of these commit messages was added by rewriting the unpublished history on 2026-10-17 (the branch `backup/before-attribution-fix` holds the old history; delete it once nothing needs it).
- Branch `experiment/order-planner-schema` (not adopted): the schema and context-scoping experiments in
  `canopy-planner/src/draft.rs` and `prompt.rs`. They raised plan validity, not the Implementer pass rate.
  Kept for reference; do not merge as is.
- Not changed on purpose, left for the maintainer to decide: the escalation order and the "propose and stop
  for human approval" rule for code-level changes in `CLAUDE.md` (Principles and Diagnosing Dogfooding
  Runs). They still read as defaults and could narrow the solution space.
- Knowledge-capture items due from `CLAUDE.md`'s cadence and not yet done: a principle from the retry bug
  ("a rule that contradicts another injected instruction") and, when step 1 closes, a retrospective.

## Update 2026-10-17 (end of day): where the course stands

Step 1 is met on the order plan (the hand plan passes 10/10 and 9/10 on the current prompt; three prompt variants
all at the ceiling). Step 2 (generalise) is under way, in Java first, as the maintainer directed, before any
modify-existing-code story. Everything below is recorded, with predictions and results, in
`canopy-implementer/docs/prompt-and-plan-changelog.md` (entries C1 to C12, G1, R1 to R3); read that for evidence and
generality levels.

- **Judge:** every story now has a hidden acceptance test the model never sees, validated against a reference and
  mutants (order, loans, shipping, stock; `tasks/<story>/*HiddenTest.java`). Own tests passing is not behaviour.
- **Stories:** order (done), loans (stateful rules, dates), shipping (tiered computation), stock (layered,
  immutable); each has plans at three detail levels (L1 checklist with example values, L2 prose rules, L3 one line)
  where measured. `run_step --task story:<name> --plan <plan>` runs any of them.
- **Findings that carry over** (each measured on a few stories, one model, one stack; see the change log for n):
  1. Plan detail decides the result: one line per step fails (order 0/10, loans 0/10, shipping 1/10); prose with every rule
     in it passes order (9/10) and half of shipping (5/10) but not loans (0/10); a checklist of tests with concrete
     values copied from the feature's example tables passed order (9/10) and loans (10/10).
  2. The limiting step is the first attempt. The model writes code and its own tests that disagree on a Java fact
     (BigDecimal scale: `0` against `0.00`). Retries then repeat the failed answer (most of the time) and almost
     never recover on the current prompt. A stated fact in the context and a retry hint helped only where the model
     had merely forgotten `setScale`.
  3. The extra system rules are not shown to matter: `lean` and `persona` equal `current` wherever compared.
  4. A restart from zero beat the retry as sent in a replay (10 of 28 against 1 of 28); the real-harness test (R3)
     was queued and, if not reported in the change log, is not done.
- **Next (suggestion):** carry the L1 shape into the Planner's design (step 3): the Planner copies the example
  values of each scenario into a numbered list of tests per step. Before that, finish the cells in the change log
  that are marked queued (stock, restart), and decide what to do about steps whose first attempt is a model blind
  spot: plan detail works, retries do not.
