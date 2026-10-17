# Implementer: the agreed next steps (written 2026-10-17, to resume after a context clear)

Status: agreed with the maintainer, not started. Read first, in order: the "Prompt Style and Working Course" section
of `CLAUDE.md`, `docs/design/implementer-first-course.md` (with its updates at the end),
`canopy-implementer/docs/prompt-and-plan-changelog.md` (every change and measurement, with predictions and generality),
`docs/retrospectives/2026-10-17.md`.

## Direction (from the maintainer, 2026-10-17)

- **Focus on the Implementer.** Make steps with narrow scope work reliably, and understand what is needed from the
  Planner. Do not build Planner features yet (course step 3 waits).
- **Generalise within Java greenfield stories first**; modifying existing code comes after.
- Method unchanged: write the prediction and the falsifier in the change log before each batch (n=10 per cell, one factor at
  a time), show the first run's prompt and transcript, log generality (what was not varied), never trust a single run.
- Old runs from before the clean prompts (2026-10-08..12) are history, not evidence for the current Implementer.
- Ask before pushing (the last push was `9c01916`; nothing pending at the time of writing).

## What is established (one model, plain Java, four small greenfield stories; details in the change log)

- Plan detail decides the result. One line per step fails on every story. Prose rules pass order and stock, not loans, and shipping
  by one run. A checklist of numbered tests with **concrete values copied from the feature's example tables** passed loans 10/10
  and order 9/10; the values are the active ingredient (loans 10/10 with values only, 0/10 with the "MUST ... check the list"
  wording only).
- A coverage item for the side of a rule the examples do not show closed shipping's gap (6/10 to 9/10).
- A checklist can cost API fidelity: stock 10/10 (prose) to 6/10 (checklist), 4 runs wrote a class with `getOnHand()` instead of the
  specified record. Cause not separated: dilution by length against the "has onHand 15" wording (no accessor call in the items).
- The first attempt is the limiting step; retries mostly repeat the failed answer. A BigDecimal-scale hint or stated fact helped only
  the one-line order plan. A restart-from-zero retry beat the retry as sent in a replay; its real-harness test (R3) was not run.

Hidden-test results per cell, the baseline for anything new (n=10 each, `--prompt lean` unless stated):

| Story | L3 one line | L2 prose | L1 checklist (values) | other |
|---|---|---|---|---|
| order | 0 | 9 (cur 8) | 9 (cur, D1) | |
| loans | 0 | 0 | 10 | values only 10, wording only 0 |
| shipping | 1 | 5 | 6 | checklist + rule item 9 |
| stock | 0 | 10 | 6 | |

## Plan A (needs the GPU, about 9 minutes): C17, stock at the checklist level with an API block

Question: does stating the API once, explicitly, as the calls the tests will make, stop the checklist from costing API fidelity?

1. Author `canopy-implementer/tasks/stock/plan-checklist-api.yaml` from `plan-checklist.yaml` (same steps, files, coverage; a unit
   test in `canopy-implementer/src/plan.rs` pins that). In each step, before the numbered tests, add an "API" block listing what
   the tests use, including accessor calls, e.g. for step 1: `StockItem is a record: new StockItem(String sku, int onHand);
   item.sku(); item.onHand(); item.receive(int) returns StockItem; item.reserve(int) returns StockItem`. Rewrite each item as the
   call it asserts, e.g. `new StockItem("SKU-1", 10).receive(5).onHand() is 15`. Keep everything else identical.
2. Log C17 in the change log **before running**: prediction: at least 9 of 10 runs pass the hidden test (checklist 6/10, prose
   10/10); falsified if 7 or fewer (then the cost is not the missing accessor syntax and the next candidate is length/dilution).
   Generality `none`.
3. Run: `run_step --task story:stock --plan canopy-implementer/tasks/stock/plan-checklist-api.yaml --runs 10 --prompt lean --url <server> --model Qwen/Qwen3-14B-GGUF:Q4_K_M --out <dir>`
   after `cargo build -p canopy-implementer --examples` (check the binary; read the first run's output and show its prompt).
   Server address and how to start it: the maintainer's notes and the memory entry `reference_gpu_server_for_model_runs`; do not put
   the address in the repository. Check `/health` first. Count how many runs wrote `StockItem` as a record (the earlier measure).
4. Record the result (prediction held or not), commit.

## Plan B (offline, run while Plan A is on the GPU; cap 30 minutes): a plan contract-coverage audit

Question: does a deterministic audit of a plan against its feature predict the Implementer's result? If it does, it names what the
Planner must carry; if not, drop it and record that.

1. A small analysis tool, `canopy-implementer/examples/plan_audit.rs` (analysis, not Planner code): read a story's
   `feature.feature` and a plan (`Plan::from_yaml`), extract the literals the feature states: quoted strings (messages),
   decimals and integers in Examples tables and steps, exception-style words if present; report which of them appear in some step's
   instruction and which do not.
2. Score the **12 existing plan cells** (4 stories x L1/L2/L3 where an L1 exists; stock has no L1 before Plan A) against the table above.
   Success criterion, stated before looking: the count of missing literals orders the cells the same way as the hidden-test pass
   rate (L3 worst, L2 or L1 best) on at least three of the four stories. Note what the audit cannot see by construction: rules
   the examples do not exemplify (shipping under 1000 g) have no literal, so expect it to miss that gap.
3. Log it as an analysis entry (not a prompt change). Do not wire it into the Planner until course step 3.

## Decision after A and B

- If A holds: the plan shape for a narrow step is (API block with calls, rules in prose with exact messages and scale, numbered tests
  with concrete values, a coverage item for each rule's unexemplified side). Write it as the target the Planner must produce.
- If B holds: the audit is the first deterministic check of a plan; the Planner loop is "audit, then revise" with the audit as ground
  truth (not an LLM opinion).
- Then pick from the deferred list below; the first candidate is the residual first-attempt failures at L1/L2.

## Deferred, with the reason

- **R3**, the real-harness test of `--retry restart` (flag exists; prediction logged in the change log): the biggest retries finding
  came from a replay on 7 cases; run it on the L3 plans where first attempts fail.
- **`current` system-prompt arms** for loans, shipping and stock (prediction 1 of G1 is untested there), and **stock at L1** (that is Plan A).
- **A layout test** (Project first, task last, or Project in the system prompt): measured prefill is 240 ms of a 9 to 15 s call, so
  the cache gain is under 1%; adopt "static first" only with the next prompt change, and test quality only if Project grows.
- **A Reviewer stage** (LLM or acceptance tests written from the feature, run by the harness): not now; needs the audit result and a
  working repair path first.
- **Planner-side ideas** from the pasted article (critique loop, failure history, discovery before implementation, verification-first
  output): step 3, and for the modify-existing-code stage. Reject the model's self-reported confidence.
- **Brownfield (modify existing code)** stories: after Java greenfield generalisation is settled.

## Housekeeping

- Three `canopy-implementer/tasks/*/implementer-runs.jsonl` files are modified and uncommitted (harness run records, not labelled by
  variant); decide whether to commit them or ignore them.
- Run directories are scratch (`/private/tmp/...` scratchpad) and not kept; the change log holds the numbers.
- The story repositories live in `dogfooding/java-<name>` (git-ignored, tag `baseline-<name>-001`); recreate them from the
  `java-orders` pom if missing (a pom with JUnit 5.10.0, compiler plugin 3.11.0, surefire 3.0.0, Java 21).
