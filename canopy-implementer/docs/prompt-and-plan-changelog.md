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
- **Generality:** `single-run`. Plan shape (two steps, no repository interface) is untested on any other story;
  the widgets plan's 10/10 is a different story and a different style of stack (Spring).
- **Status:** untested (n=1).
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
- **Generality:** `none`. When measured it will be one story (order), one plan, one stack (plain Java Maven,
  JUnit 5), one model; a second story (course step 2) is needed before `lean` is a rule rather than a setting.
- **Status:** untested. Batches: A (`current`) running, then B (`lean`), then C (`persona`), n=10 each, same
  binary, same plan, same repository tag.
- **Result:** see below once the batches finish.
