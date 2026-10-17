# Change log: system prompt, task descriptions, planner output

Every change to what the model is told is recorded here with the evidence for it, so a later session can see
why a line is there or gone. Scope: the Implementer's system rules and retry text (`src/prompt.rs`), task and
plan-step descriptions (`tasks/*/plan.yaml`, the `Task` constants in `examples/run_step.rs`), and what the
Planner produces or is told (`canopy-planner/`).

Rules for an entry:
- Add it in the same commit as the change; name the commit.
- **Evidence** is a measurement or a read trace (batch, n, run directory or report section), not an impression.
  A change made on a hypothesis says so and has status `untested` until a paired batch (n=10 or more) reports.
- Record the prediction and what would falsify it before the batch; fill in the result after.
- A reverted or refuted change stays in the log, with the result.

- **Generality** says how far the evidence reaches: the stories, stacks, model and n it was measured on, and one
  of `none` (no measurement), `single-run`, `one-story` (n>=10 on one story), `multi-story` (n>=10 on two or
  more stories, one stack), `multi-stack` (also a second stack). Evidence from one story, one stack and one model
  (Qwen3-14B Q4_K_M) is never written as general: say what has not been varied. A change is only a rule for new
  work at `multi-story` or better; below that it is a setting for the story it was measured on.

Status values: `untested`, `kept` (measured, adopted), `refuted` (measured, not adopted), `reverted`.

---

## 2026-10-17

### C1. Retry after a build failure no longer tells the model to find files
- **Where:** `src/prompt.rs` (`TOOL_READ_RULES` left out of a retry that shows the previous files). Commit `502d7ff`.
- **Why:** the system rule "ALWAYS use glob and read_file" contradicted the retry text "NEVER look for the files you
  submitted". 17 of 20 retry failures in the single-step runs were tool-budget or repeated-call loops.
- **Evidence:** `docs/reports/order-plain-java.md`: loops 17/20 to 2/12 retry failures; single step 0/10 to 4/10
  scored (part of the 4/10 is sampling noise).
- **Generality:** `one-story` at best (order story, plain Java, one model); the single-step arm was a separate
  batch with its own first-attempt luck. The loop mechanism (rule contradiction) is visible in the prompt text and
  is not story-specific; the size of the pass-rate gain is.
- **Status:** kept.

### C2. Hand-written order plan, two coarse steps
- **Where:** `tasks/order/plan.yaml`. Commit `5dc9aff`.
- **What:** `Order` (record, `Order.place` factory with discount and quantity rule) with `OrderTest`; then
  `OrderService` (HashMap, `placeOrder`, `findById`) with `OrderServiceTest`. No `Product`, no repository
  interface, one package. Concrete signatures and test values in each instruction, as in the widgets plan.
- **Why:** course step 1 (`docs/design/implementer-first-course.md`): start as simple as possible. The Planner's
  plans for this story (4 to 7 steps) scored 0/20; the single step 4/10.
- **Evidence:** the widgets hand plan reached 10/10 with this instruction style. For the order plan: run 1 passed
  (one run only, `scored: true`). No batch yet.
- **Result (batch A, 2026-10-17, `--prompt current`, n=10):** 10/10 scored, every step passed first attempt
  (2 attempts in total per run, i.e. one per step). Each run had 1 to 2 refused submits, all from the import
  audit (a missing `BigDecimal`/`UUID` import), none from a gate. 0 of 10 runs assert the message "Quantity must
  be positive" in any test (no `getMessage` anywhere): the instruction asks for it, nothing checks it.
- **Generality:** `one-story` (order, plain Java, Qwen3-14B Q4_K_M, one plan). Plan shape (two steps, no repository interface) is untested on any other story;
  the widgets plan's 10/10 is a different story and a different style of stack (Spring).
- **Status:** kept as the order baseline (10/10 once; the course needs a second batch).
- **Known gap seen in run 1, not yet changed:** the instruction asked for the message "Quantity must be
  positive" to be asserted; the model's `OrderTest` only used `assertThrows` on the type. No gate checks it.

### C3. Prompt variants `lean` and `persona` (switchable, default unchanged)
- **Where:** `src/prompt.rs` (`PromptVariant`), `run_step --prompt current|lean|persona`. Commit `caf0aef`.
- **What `lean` removes from the system rules:** "ALWAYS use glob and read_file…", "ALWAYS read an existing file
  with read_file before you change it", "ALWAYS include a test for every behaviour…", "ALWAYS put one public type
  in each file…", "ALWAYS write tests with JUnit 5… same package", "NEVER change pom.xml or any file the task does
  not require". It keeps: finish with `submit_files`; keep existing code of a file you change; (scoped steps)
  submit exactly the listed files. The retry text in the user message is unchanged.
- **What `persona` adds to `lean`:** the opening line "You are a senior Java developer who writes small, clean,
  tested code. You do exactly what the task says, no more."
- **Why, per removed rule:** read rules: the scoped step already shows its files, and the rule sent 3 of 20
  first attempts into `glob` loops in an empty repository (report, open questions); run 1 made no read call.
  Test rule: every plan instruction names its tests. One public type: `javac` enforces it. JUnit/package: the
  project context says JUnit 5 and the paths fix the package. pom.xml: the project context says it, the scope
  lists the files, and the scorer checks `pom.xml` untouched. These are hypotheses, not measurements.
- **Why persona:** user suggestion; it carries the scope guard in the same line. Risk: invites extras.
- **Prediction (before the batches):** `lean` scores at least as well as `current`; if `lean` is 2 or more
  passes below `current` at n=10, one removed rule was doing work and gets bisected. `persona` is judged against
  `lean`, not `current`.
- **Result, batch B (`--prompt lean`, n=10, same binary/plan/tag as A):** 10/10 scored, every step passed first
  attempt. Refused submits (import audit): 15 over 10 runs against 12 in A, same cause (missing `BigDecimal` or
  `UUID` import); 3.2k to 4.9k prompt tokens per run in both arms. The first run's system prompt was read and
  shows five lines (the lean rules); the user prompt is identical to A's. Prediction (B at least as good as A)
  held; it is not distinguishable from A in this story: both arms are at the ceiling, so B cannot be shown
  better, only not worse. 0 of 10 B runs assert the exception message either (as in A).
- **Result, batch C (`--prompt persona`, n=10, same binary/plan/tag):** 10/10 scored, every step passed first
  attempt. Import-audit refusals: 11 over 10 runs (A 12, B 15). Run 1's system prompt was read: the persona
  sentence is joined to the head line, then the lean rules. No Javadoc or extra classes in any arm (every run
  created exactly the four listed files). One run (5) made two `glob` calls in the same turn as its first submit
  (the persona and lean prompts have no read rule, so this is the model's own habit); harmless. 0 of 10 runs
  assert the exception message, as in A and B. Prediction (persona no worse than lean, no extras) held; like B
  it cannot be shown better because the baseline is at the ceiling.
- **Generality of the persona result:** `one-story`, and weaker than `lean`'s: on a 10/10 baseline a persona
  has nothing to improve, so this says only that it does no harm here. Whether it helps needs a story or plan the
  baseline does not already pass.
- **Generality:** `one-story` for `lean` (n=10, order story, plain Java, Qwen3-14B Q4_K_M, one plan): the six
  removed rules were not needed here. A second story is required before they are treated as removable in general
  (the Spring widgets plan has modify steps and a framework, where "keep the existing code" and the JUnit/package
  rules may matter more). Original line, kept for the still-open `persona` arm: When measured it will be one story (order), one plan, one stack (plain Java Maven,
  JUnit 5), one model; a second story (course step 2) is needed before `lean` is a rule rather than a setting.
- **Status:** `lean`: kept as a setting for the order plan (10/10, equal to `current`); `persona`: no harm on the order plan (10/10), no measured benefit. Batches: A (`current`, done), B (`lean`, done), C (`persona`, done), n=10 each, same
  binary, same plan, same repository tag.
- **Result:** see below once the batches finish.

### C4. Order plan step 1 as a checklist of musts (plan variant `plan-checklist.yaml`)
- **Where:** `tasks/order/plan-checklist.yaml` (step 1's instruction only; step 2, files and coverage are the same
  as `plan.yaml`, checked by a unit test). Scorer: `Check::TestContains` in `src/structure.rs`, `ORDER_MUSTS` in
  `examples/run_step.rs`. Commit: see the git log entry "Add a checklist variant of the order plan".
- **What:** the prose list of test examples becomes a numbered checklist of 8 tests, introduced with "It MUST
  contain one test for each item… you must not leave any item out. Before you submit, go through the list…".
  Two items are new musts: items 6 and 7 say the test checks the exception message equals "Quantity must be
  positive"; item 5 is a new case, 100.00 x 5 = 450.00 (exactly 500.00 gets the discount).
- **Why:** batches A, B, C (30 runs, three prompt variants) all passed the gates while 29 of 30 `OrderTest`s did
  not assert the message the instruction asked for (1 of 30 contained the literal) and none tested the 500.00
  boundary (0 of 30 contain 450.00; this one was not asked for). The gates cannot see either gap.
- **Scorer change:** `scored` for `order-planned` now also requires `OrderTest` to contain the literal "Quantity
  must be positive" and "450.00". Stricter than A–C: re-scored retrospectively, A, B and C would be 0/10, 0/10 and
  at most 0/10 on the 450.00 must. Compare only runs scored with the same scorer; the per-check results are in each
  run's summary. The literal check proves presence, not that the message is asserted; it is a floor.
- **Prediction (before the batch):** with the checklist, at least 8 of 10 runs contain each literal and every
  step still passes first attempt (no loss against 20/20). **Falsified if** either literal is in fewer than 6 of
  10 runs: instruction detail then does not reliably reach the tests, and the next arm is the harness tracking the
  musts and sending a retry that names the missing one (needs a proposal and approval first, per `CLAUDE.md`).
- **Arms:** D0 = `plan.yaml`, D1 = `plan-checklist.yaml`; both `--prompt current`, n=10, same binary and repository
  tag, new scorer. D0 also checks the scorer on the old plan.
- **Generality:** `none` yet. When measured: one story, one stack, one model, one must per kind (a message, a
  boundary); a different kind of must (an interaction between classes, a state change) is not covered.
- **Layout bug caught in run 1 of the first D1 attempt (batch aborted, not counted):** the instruction was first
  written as a YAML folded scalar (`>`), which joined checklist items 1 to 5 into one line and broke items 6 to 8
  across lines; the prompt the model saw was not the checklist described above (run 1 still passed with the
  literals). Fixed by writing the instruction as a literal block (`|`); a unit test now checks that the items stay
  on separate lines. D1 was restarted from run 1. Lesson: read the rendered prompt, not the YAML.
- **Status:** untested.
