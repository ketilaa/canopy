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
- **Result (2026-10-17, `--prompt current`, n=10 each, new scorer):**

  | | D0 `plan.yaml` | D1 `plan-checklist.yaml` |
  |---|---|---|
  | Scored (gates + both musts) | 0/10 | 9/10 |
  | `OrderTest` contains "Quantity must be positive" | 2/10 | 10/10 |
  | ... and asserts it with `assertEquals("Quantity must be positive", ...)` | 1/10 | 10/10 |
  | `OrderTest` contains "450.00" (the boundary) | 0/10 | 10/10 |
  | Steps passed | 20/20 | 19/20 (run 3 failed step 1) |

  D0 passed every gate in all 10 runs and fails the scorer only on the musts (as A, B, C would have: 3 of 40
  old-plan runs had the message literal in all four batches, 0 of 40 the boundary). Prediction held for the
  musts (10/10 against the 8/10 bar). "Every step still passes first attempt" did not hold: run 3 failed step 1
  after three attempts. Cause, read from its transcript: the model wrote a message check through a shared field
  and a helper (`getExceptionMessage()` reading a `thrownException` field no test had set), which threw a
  NullPointerException in two tests; the retry feedback named the exact NullPointer and attempts 1, 2 and 3 were
  **byte-identical answers**. One failure in 10 against 0 in 10 is not distinguishable from noise (and D1 has
  twice as many things to do in step 1), but the mechanism is plausible: a harder must produced a more elaborate
  test, which was the part that broke.
- **Prevalence of the identical retry (measured afterwards, no change made):** over every stored Implementer
  run with a retry (518 retries, `canopy-implementer/runs/` 2026-10-08..12 plus today's batches), 223 (43%)
  resubmitted byte-identical files to the failed attempt before them: 146 of 228 after a test failure (64%), 64 of
  232 after a compile failure (28%), 13 of 43 after a "stuck". **0 of the 223 recovered** (the build is
  deterministic, so an identical submission fails identically); the 295 retries that changed their files recovered
  107 (36%). Caveats: all but 2 of the retries are from the widgets stories on earlier prompts and retry text;
  the order-plan batches of 10-14..16 are not stored, and today's batches had only 2 retries (both identical).
  Generality: `one-story` family (widgets, Spring) plus the order plan, n large for widgets, tiny for order.
  **Comparability warning (added the same day, on the user's correction):** those runs carry far more context noise
  than the current prompts (long per-step contexts with conventions, designs, scenario text, stereotype and
  namespace lines; the contradictory read/retry rules). Their rates (43% identical retries, 61% first-attempt pass,
  22% for a restart, 43% recovery) are history, not evidence about the current Implementer, whose order plan
  passes first attempt 100 of 101 times. Do not carry the numbers over.
- **New failure class seen (not acted on):** a retry that resubmits the identical files after feedback that names
  the error. The harness has a `NoChange` stage for "same as what is in the repository", but a restored workspace
  hides this case. Whether to detect it (and what to tell the model) is a separate change, to be proposed first.
- **Generality:** `one-story`: order story, plain Java, Qwen3-14B Q4_K_M, n=10 per arm, two musts (a message, a
  boundary), both literal. The checklist form made the model cover every listed item; a different kind of must
  (an interaction between classes, a state change) is not covered, and the literal check proves presence, not
  correctness (`assertEquals` with the message was counted separately and agrees: 10/10).
- **Status:** `plan-checklist.yaml` kept as the order plan for the next experiments (musts covered, one failure
  in 10 to watch). The harness-tracking arm is **not needed for these musts** (compliance 10/10); it stays an
  option for musts the checklist does not get followed on.

### R1. Retry replay: what should a retry show about the failure? (measurement, no prompt change yet)
- **Where:** `examples/retry_replay.rs`, `src/replay.rs` (commit `47bf380`). Changes nothing the Implementer says;
  it replays stored retry prompts against the model in three variants and builds each changed answer.
- **Why:** 43% of stored retries resubmitted the failed files unchanged and none of those recovered (see C4's
  prevalence note). Hypothesis (the user's): the previous answer, shown in full in a fresh retry conversation,
  anchors the model into copying it; a retry should not carry the code that does not work.
- **Variants** (system rules and the rest of the prompt as recorded; the two "read files" rules are removed from the
  system prompt in all three, as a retry that shows files has it): `current` = previous answer plus the error as
  sent; `fresh` = previous answer left out, the error kept; `located` = previous answer kept plus a
  `## Where it failed` block quoting the source lines the error points at.
- **Prediction (before the batch):** `current` repeats the failed files in at least 40% of answers (history: 43%);
  `fresh` repeats in under 10% (it has nothing to copy); `located` repeats less than `current`. Recovery
  (compile and tests pass) is the real measure: `fresh` at least as good as `current`.
  **Falsified if** `fresh` recovers at least 10 points fewer than `current` (it rewrites from scratch and
  reintroduces bugs, or spends its tool budget exploring), in which case the previous answer is helping and the
  next step is a better feedback text (`located`), not removing the code.
- **Deviations from the real harness (known):** the import audit and scope refusals are not applied (raw answer);
  cases come from earlier prompts and stories (mostly the Spring widgets story, 2026-10-08..12) plus 2 from today;
  n per variant is small.
- **Smoke run (n=1 per variant, 2 cases, not a result):** a replay with only one model turn per answer counted
  `glob`/`read_file` openings as "no answer"; fixed by carrying out tool calls against the rebuilt repository.
  In `fresh` the model goes looking for the files it is to create (glob/read) far more than in `current`: an
  effect of the variant, counted against the tool budget as in the real harness.
- **Result (20 stored retry cases, 10 test-stage and 10 compile-stage, x 2 samples each; seed 7; 2026-10-17):**

  | variant | answers | identical to the failed files | changed | **passed** | no answer (tool budget / prose) |
  |---|---|---|---|---|---|
  | `current` | 40 | 16 (40%) | 21 | 8 (20%) | 3 |
  | `fresh` | 40 | 0 | 38 | **15 (37.5%)** | 2 |
  | `located` | 40 | 14 (35%) | 19 | 7 (17.5%) | 7 |

  Cases recovered by at least one of two samples: `current` 5/20, `fresh` 11/20, `located` 4/20 (fresh-only 7
  cases, current-only 1). One-sided Fisher: `fresh` vs `current` p=0.07, `fresh` vs `located` p=0.04. Build
  outcomes of the changed answers: `current` 8 pass / 8 test fail / 5 compile fail; `fresh` 15 / 6 / 17;
  `located` 7 / 9 / 3. Predictions: `current` repeats >= 40%: held (40%, at the bar); `fresh` repeats < 10%:
  held (0%); `fresh` recovers at least as well as `current`: held (+17.5 points; the falsifier was -10).
  `located` repeating less than `current`: not distinguishable (35% vs 40%), and it recovered no better.
- **Reading:** the repeat disappears when the failed code is not shown (0 of 40), which is consistent with
  anchoring, and recovery roughly doubles. Showing the lines the error points at did nothing for recovery and
  made the model spend its tool budget more (6 of 40 answers) than the other variants. Caveat that matters:
  the replay applies no import audit, and 17 of the 38 changed `fresh` answers fail to compile (the model rewrites
  every file and forgets imports); in the real harness that audit refuses those within the attempt, so `fresh`'s
  real recovery is probably higher, and `current`'s (5 compile fails) less affected. Not measured.
- **Other caveats:** 20 cases drawn from 551; mostly the Spring widgets story on earlier prompts; 2 samples per
  case are not independent (the case-level counts above are the cautious view); one model, one run of the batch.
- **Generality:** `one-story` family (widgets, Spring) with n=40 per variant. No order-plan cases (it almost
  never fails on the current prompts: 100 of 101 step runs passed first attempt today).
- **Comparability warning (added on the user's correction):** all 20 cases come from the noisy-context era
  (long convention-laden step contexts, the read-rule contradiction). R1 is a paired comparison on identical
  prompts, so the *difference between variants* is internally valid, but its size, and the recovery rates, belong
  to that noisy context; it may not hold on the current clean prompt, where a failure is rare. The same applies
  to the leave-one-out analysis (a restart from zero passing at 22% on failing steps): that is a property of
  noisy-era steps. Nothing from R1 or the prevalence note justifies a change to the current Implementer yet.
  To replicate, the cases must come from the current prompt (see below).
- **Status:** measured on noisy-era cases; no change adopted; replication on current-prompt failures is the next
  step (a source of such failures is needed: the whole order story as one step failed 6 of 10 times on the
  current prompt in the 2026-10-16 batch SB).

### C5. A hidden acceptance test for the order story (scorer; the model never sees it)
- **Where:** `tasks/order/OrderHiddenTest.java`, wired into `ORDER_PLANNED` in `examples/run_step.rs`; the two
  literal musts of C4 are now reported as `own_tests_cover` with each run and no longer decide `scored`.
- **What it checks:** the feature against the plans' fixed API: price times quantity, no discount below 500.00
  (also 499.95), 10 percent off from 500.00 (also exactly 450.00), zero and -1 rejected with the message "Quantity
  must be positive", orders get own ids and keep customer, product and quantity, lookup by id, unknown id empty.
  Totals are compared by value, not scale.
- **Why:** until now `scored` meant "the model's own tests pass"; nothing checked that the production code is
  right, so a plan with less detail could pass every gate with wrong behaviour.
- **Validation of the test itself:** passes against a reference implementation; fails on four mutants (`>`
  instead of `>=`: 2 red; another message: 2 red; zero allowed: 4 red; 5 percent: 2 red).
- **Retrospective result:** applied to the final files of all runs that completed both steps in batches A, B, C,
  D0 and D1: **49 of 49 pass** (D1 run 3 never completed). The production code was correct in every run, also in
  the 29-of-30 runs whose own tests ignored the exception message; the weakness was in the model's tests, not its
  code, at this level of instruction detail.
- **Generality:** `one-story`, one stack, one model.
- **Status:** kept (scorer).

### C6. Factor 1 of course step 2: instruction detail on the coarse order plan (new evidence on the current prompt)
- **Where:** `tasks/order/plan-signatures.yaml` and `plan-oneline.yaml` next to `plan-checklist.yaml`. Same two
  steps, files and covered scenarios (a unit test pins this); only the instruction text differs. All three keep
  the class and method signatures (the contract the hidden test needs).
  - L1 `checklist` (existing, D1): rules in prose plus a checklist of 8 tests with values.
  - L2 `signatures`: signatures and every rule in prose (discount, rounding, scale, the message), tests named by
    topic without values.
  - L3 `oneline`: one sentence per step, as a Planner might write it: signatures, "10 percent from 500.00",
    "rejects a quantity that is not positive"; no scale, no message, no test content.
- **Why:** the course asks what a good plan step is, and the current plan never fails, so it teaches nothing. The
  user ruled out whole-story-in-one-step plans and old noisy-context runs as sources of failures; this builds new
  evidence on the current prompt at realistic step size. Failed attempts from these batches are kept as the
  cases for the retry replay (R1) on the current prompt.
- **Prediction (before the batches), `--prompt current`, n=10 per arm, same binary and tag:** hidden acceptance
  passes L1 9 or 10 of 10 (D1 rescored: 9 with 1 incomplete), L2 at least 8 of 10, L3 **at most 7 of 10**; the
  likely misses at L3 are the message (it is only in the feature file, which a step does not see) and
  rounding/scale. First-attempt pass of a step falls from about 95% at L1 to 80% or less at L3.
  **Falsified if** L3 passes the hidden test in 9 or 10 of 10: instruction detail is then not a lever on this
  story and the next factor or a second story is the way to find where plans fail.
- **Generality:** `none` yet. One story, one model; the hidden test encodes this story only.
- **Result (2026-10-17, `--prompt current`, n=10 per arm; L1 = D1 rescored with the hidden test):**

  | | L1 `checklist` | L2 `signatures` | L3 `oneline` |
  |---|---|---|---|
  | Instruction text (both steps) | 3,200 chars file | 2,169 | 1,649 |
  | Runs that completed both steps | 9/10 | 8/10 | 1/10 |
  | **Hidden acceptance passed** | **9/10** | **8/10** | **0/10** |
  | Step first-attempt failures | 1 of 19 step starts | 2 of 18 | **9 of 11** |
  | Retries after a compile/test failure | 2 | 4 | 18 |
  | ...identical to the failed answer | 2 | 4 | 16 |
  | ...recovered | 0 | 0 | 0 |
  | Own tests contain "450.00" / the message literal | n/a (asked) | 8/8, 0/8 | 0/1, 0/1 |

  Predictions: L1 9 or 10 of 10: held (9). L2 at least 8: held (8, exactly at the bar). L3 at most 7: held, far
  past it (0). First-attempt pass at L3 80% or less: held (2 of 11 step starts passed, 18%). The falsifier (L3 at
  9 or 10) did not occur.
- **What failed at L3 (read from the transcripts):** 9 of 10 runs failed step 1 with the same mode: the model's own
  test expects `1080.00` and its code returns `1080.000`, because the one-line step never says the total has two
  decimals (24 failure lines of this kind, 3 more `expected <N.N> but was <N.N>`). The one run that passed its gates
  failed the hidden test on two defects the gates cannot see: the message `"Quantity must be positive."` (a
  full stop added) and `>` instead of `>=` at the 500.00 threshold (100.00 x 5 gave 500.00). Details the Planner
  must carry from the feature into a step: the exact message, the two-decimal scale, and the boundary's
  inclusive/exclusive wording. At L2 (all of these in prose, tests named by topic) 8 of 10 completed and all 8 passed
  the hidden test; the two L2 failures were a discount-threshold test the model wrote wrongly for its own code.
- **Retries on the current prompt, new evidence:** over the three batches there were 24 retries after a compile
  or test failure; **22 were byte-identical to the failed answer and none of the 24 recovered.** Of the first-attempt
  failures at L3, all but 2 then repeated their answer, up to the attempt limit. This replicates the identical-retry
  finding from the noisy-era history (C4) on the current prompt, in 7+ failing runs of one story; the failure mode
  is narrow (mostly one scale mismatch), so how often it happens elsewhere is unmeasured. These 24 retry prompts
  are a current-prompt test bed for R1.
- **Generality:** `one-story` (order, plain Java, Qwen3-14B Q4_K_M, n=10 per arm, one plan shape of 2 steps). The
  ranking L1 >= L2 >> L3 is clear here; where between L2 and L3 the threshold lies, and whether other stories
  need other details, is not measured. The hidden test encodes this story only.
- **Status:** measured; C6 is a finding about plan detail, not a change to the Implementer. `plan-checklist.yaml`
  and `plan-signatures.yaml` stay as the passing plan shapes for the order story, `plan-oneline.yaml` as the
  failing one.

### R2. Retry replay on current-prompt failures, with a restart-from-zero variant (measurement)
- **Where:** `examples/retry_replay.rs` (new variant `blank`; the import, scope and classpath audit are now applied
  to replayed submissions, and a refusal goes back to the model as in a real attempt; identical prompts are
  de-duplicated). Commit: see the git log entry "Replay retries on current-prompt failures".
- **Cases:** the 24 retry prompts from batches D1, E2, E3 of 2026-10-17 (current prompt, order story, coarse plans),
  which are 12 distinct prompts, all test-stage failures: 9 variants of one scale mismatch (`expected <N.NN> but was
  <N.NNN>`), 2 discount-threshold tests, 1 message-helper NullPointer. Narrow by construction; n per variant
  = 12 cases x 4 samples = 48. Replaces R1's noisy-era cases as the basis for any decision.
- **Variants:** `current` (previous answer + error, as sent), `fresh` (error only), `located` (previous answer + the
  failing source lines), `blank` (the attempt-1 prompt again: a restart from zero with no trace of the failure).
- **Prediction (before the batch):** `current` repeats the failed answer in at least 70% of answers (history of
  the three batches: 22 of 24) and recovers at most 10%. `fresh` repeats under 10% and recovers at least 15
  points more than `current`. `blank` recovers about the first-attempt pass rate of these steps (low, they were
  selected for failing) and no more than `fresh`. `located` is no better than `current`.
  **Falsified, for the restart idea, if** `blank` recovers at least as many as `fresh` (then the failure text adds
  nothing and the simplest recovery is a restart). **Falsified, for the anchoring idea, if** `fresh` repeats the
  failed answer in 10% or more or recovers no more than `current`.
- **Generality:** `one-story`, narrow failure mode, one model. Says whether a retry can work on the failures of
  the current prompt that we have; says nothing about other kinds of failure.
- **Result (stopped early to free the GPU for G1: 7 of 12 cases complete, 4 samples each, 28 answers per
  variant; the unfinished cases are not counted):**

  | variant | answers | identical to the failed answer | passed (compile + tests) | cases recovered at least once |
  |---|---|---|---|---|
  | `current` (as sent) | 28 | 27 (96%) | 1 (4%) | 1/7 |
  | `fresh` (error only) | 28 | 0 | 4 (14%) | 1/7 |
  | `located` (+ failing lines) | 28 | 27 (96%) | 1 (4%) | 1/7 |
  | `blank` (restart from zero) | 28 | 0 | **10 (36%)** | **4/7** |

  Predictions: `current` repeats at least 70% and recovers at most 10%: held (96%, 4%). `fresh` repeats under 10%:
  held (0%); `fresh` recovers at least 15 points more than `current`: **not held** (14% vs 4%, +10). `located` no
  better than `current`: held. `blank` no better than `fresh`: **falsified: 10 passes against 4** (one-sided
  Fisher about p=0.06 on answers, with 28 answers drawn from 7 cases). By the logged rule this supports the restart
  idea over the failure-text idea on these cases: with the failed code in the prompt the model copies it, without it
  the model still fails the same way most of the time, and a clean restart does about as well as the base rate
  of the step.
- **Where `blank` wins and loses:** it recovers 4 of 4 on the one case from a step that usually passes (an L2
  plan, first-attempt pass 8 of 10) and 0 to 1 of 4 on the six cases from the one-line plan (first-attempt pass 2 of
  11): `blank` recovers at about the step's own first-attempt rate, which is the independence a restart relies on.
  Restart therefore helps where the plan is adequate and the failure was bad luck, and does not rescue a plan
  that lacks the details (see C6). Audit refusals occurred in 18 of the 112 conversations and the model fixed them.
- **Caveats:** 7 cases, 6 of them the same scale mismatch from one plan; one model; 4 samples per case are not
  independent. Not a basis for changing the Implementer by itself; it is a reason to make a restart a candidate in
  the next recovery experiment inside the real harness rather than only in a replay.
- **Generality:** `one-story`, one narrow failure mode, current prompt.
- **Status:** measured (partial); the real-harness test of a restart-from-zero retry is a candidate, not done.

### G1. Generalising within Java: three new greenfield stories, detail levels, and lean vs current (user's direction, 2026-10-17)
- **Direction (from the user):** generalise within the Java ecosystem before raising the bar by modifying existing
  code; the decisions for the next three hours are mine. So: more greenfield Java stories of different shapes, with
  the same measuring method, before any modify-existing-code step.
- **New material:** `--task story:<name>` in `run_step` loads `tasks/<name>/task.yaml` (instruction, context, repo,
  tag, hidden test, minimum tests); three stories, each a plain Maven Java 21 / JUnit 5 repository in `dogfooding/`
  (git-ignored) tagged `baseline-<name>-001`, with a Gherkin feature, a hidden acceptance test and two plans:
  - `loans`: stateful rules with dates (`Loan` record with due date and late fee; `LoanService` with limit,
    exclusivity, return). 2 steps.
  - `shipping`: tiered computation (`Zone` enum with base price; `ShippingCalculator` with weight steps, rounding up
    "each started 500 g", a free-shipping boundary, a weight range). 2 steps.
  - `stock`: layered, immutable (`StockItem` record, `StockRepository` interface, `InMemoryStockRepository`,
    `StockService` through constructor injection). 3 steps.
  The plans are `plan-signatures.yaml` (L2: signatures and every rule in prose, exception types and messages
  included, tests named by topic) and `plan-oneline.yaml` (L3: one sentence per step, signatures and the numeric
  rules, without messages, exception types or scale), the same factor as C6.
- **Hidden tests validated before use:** each passes a reference implementation and fails every non-equivalent
  mutant tried (loans 4 of 4 killed, plus one equivalent mutant; shipping 5 of 5; stock 5 of 5).
- **Matrix (n=10 per cell, same binary and tags):** order L3 and L2 on `lean` (compared with E3 and E2 on `current`);
  for each new story L2 on `lean`, L3 on `lean`, L2 on `current`.
- **Predictions (before any of it runs):**
  1. *lean vs current.* On order L3 (current: hidden 0/10, 9 of 11 first attempts failed) `lean` is within 1 of
     `current` on hidden passes; a `lean` result of 4 or more of 10 would mean the extra rules were hurting. On order
     L2 (current 8/10) `lean` scores 7 or more. On each new story at L2, `lean` is at most 1 pass below `current`.
     **Falsified (lean is not safe to adopt)** if `lean` is 3 or more passes below `current` on the L2 cell of any story.
  2. *detail generalises.* L2 passes the hidden test in 6 or more of 10 runs on each new story; L3 in 5 or fewer on
     each. **Falsified** if L3 reaches 8 or more of 10 on two stories (detail is not the general lever) or L2 is 4 or
     fewer on two stories (the claim that L2 carries enough is order-specific).
  3. *what L3 loses.* The hidden-test failures of L3 runs that complete name a message, an exception type or a scale
     (details a feature states and a one-line step drops), not an algorithmic mistake, in at least 60% of cases.
- **Generality:** that is the point: `none` until the matrix reports; after it, at best `multi-story` (four stories,
  one stack: plain Java Maven, one model).
- **Result (loans L2, `lean`, n=10, `--scale-hint-sides`):** hidden 0/10; step 1 passed in 0 runs; retries identical
  to the failed answer **18 of 20**; 0 recovered. Prediction (at least 3 of 10 pass step 1): **falsified**, and the
  logged falsifier (2 or fewer runs pass step 1) holds. Read together with C8 (plain hint: 1 run recovered, 9 of
  19 identical): on loans the hint arms recovered 1 of 39 retries against 0 of 20 without a hint, and identical
  retries were 27 of 39 against 20 of 20; the difference between the two hint wordings is within what n=10 can
  resolve, and naming the sides did not help.
- **Reading:** a stated fact at retry time rescued order L3 (8 of 13 retries) and almost nothing on loans (1 of
  39). In order the repair is one-sided (add `setScale` in the code); in loans the first attempt is the problem
  (10 of 10 runs write `BigDecimal.ZERO` in the code and expect it in the test), and the retry mostly repeats it.
  Prevention at the first attempt, not repair, is where loans needs help.
- **Status:** `refuted` for the sides wording (the plain hint stays as C8); the flag stays for reproducibility.

### C7. Retry mode `restart` (flag, default unchanged)
- **Where:** `RetryMode` in `src/implementer.rs`, `run_step --retry feedback|restart`. Commit: see the git log entry
  "Add a restart retry mode".
- **What:** with `restart`, a retry after a failed compile or test sends the first attempt's prompt again, word for
  word (no previous answer, no error); the workspace is restored as before. Failures of process (tool budget,
  repeated call, prose) keep their steering text. `feedback` (default) is the old behaviour. A unit test pins that
  the restart prompt equals the first one and contains nothing of the failure.
- **Why:** R2 on current-prompt failures: the retry as sent repeated the failed answer 27 of 28 times and passed
  1 of 28; a restart from zero passed 10 of 28 (the failure text without the code 4 of 28). The user's reasoning:
  do not pollute the context with repetitions of what does not work.
- **Evidence limits:** a replay on 7 distinct failures (6 the same scale mismatch), one model, not the real harness.
  The real-harness test is R3: L3 plans (which fail often) on the four stories, `--retry restart` against the
  same plans with `feedback` already measured in G1.
- **Generality:** `none` in the harness; `one-story` in the replay.
- **Status:** untested in the real harness (flag off by default).

### R3. Restart retry in the real harness, on plans that fail naturally (queued; prediction written first)
- **Design:** the L3 (`plan-oneline.yaml`) plan of each story, `--prompt lean`, `--retry restart`, n=10, against the
  same plan, prompt and binary with `--retry feedback` from G1. Order: order, loans, shipping, stock. A
  restart is the first attempt's prompt again, so a step's attempts are three independent draws instead of one draw
  and two copies.
- **Measures:** steps passed within 3 attempts per step started; end-to-end hidden pass; retries identical to the
  failed answer (expected 0 under restart by construction, but a temperature-0.6 model could still repeat itself);
  retries recovered.
- **Prediction:** order L3 has a first-attempt pass rate near 20% (2 of 11 under `current`; `lean` 5 of 13 step
  starts passed); with three independent draws a step should pass within 3 attempts about 1-(0.8)^3 = 49% of the
  time against about 25% with feedback. So on order L3 `restart` completes at least 15 points more steps than
  `feedback` per step started. Where the L3 first-attempt pass rate is 70% or more (to be seen), `restart` and
  `feedback` are within 1 run of each other. Hidden pass stays at 5 of 10 or fewer on every L3 plan: a restart does
  not supply details the step lacks.
  **Falsified** if on order L3 `restart` completes fewer than 10 points more steps than `feedback`: the replay's
  advantage does not survive the real harness. If it improves hidden pass to 8 or more of 10 on any L3 plan, my
  claim that L3 loses details a restart cannot recover is wrong for that story.
- **Generality:** `none`; at best `multi-story` for the retry mechanism (4 stories, current prompt era, one model).
- **Status:** queued after G1 (run order in the queue: after the six `lean` cells of loans, shipping, stock).

### C8. A BigDecimal scale hint on test failures (flag, off by default)
- **Where:** `hints::scale_hint` in `src/hints.rs`, `Implementer::with_scale_hint`, `run_step --scale-hint`. Commit:
  see the git log entry "Add a BigDecimal scale hint".
- **What:** when a failed test line reads `expected: <A> but was: <B>` with A and B the same number written at
  different scales, the harness adds one sentence of fact to the feedback: `BigDecimal.equals` compares the scale
  too; give the value one scale in every branch of the code (an early `BigDecimal.ZERO` has scale 0) and write the
  expected value with it, or compare with `compareTo`. It is a stated fact in the same family as `subject_hint` and
  `import_hint`: the harness works it out, the model's files are never touched.
- **Why (evidence on the current prompt):** the scale mismatch is the dominant first-attempt failure of the plans
  that fail. Order L3 (`current`): 24 of 27 failure lines `expected <1080.00> but was <1080.000>`; order L3
  (`lean`): 21 of 22; loans L2 (`lean`): 10 of 10 runs failed step 1 at the first attempt, 9 of them
  `expected: <0> but was: <0.00>` (code returns `BigDecimal.ZERO` early and `setScale(2)` otherwise; the test expects
  `BigDecimal.ZERO` for both), 20 of 20 retries identical to the failed answer. Two stories, one failure mode,
  a Java library fact the model does not apply, and the error text alone does not make it fix it.
- **Prediction (before the runs):** on loans L2 (`lean`, baseline hidden 0/10, step 1 passed 0 of 10 runs) with the
  hint, at least 3 of 10 runs pass step 1 within three attempts and retries identical to the failed answer fall
  from 20 of 20 to 12 of 20 or fewer. On order L3 (`lean`, baseline 0/10, 8 of 13 first attempts failed) with the
  hint, at least 3 of 10 runs pass step 1 within three attempts; hidden pass stays at 2 of 10 or fewer, because
  that plan also omits the message and the boundary.
  **Falsified if** on loans L2 the identical-retry share stays at 18 of 20 or more: the model ignores a stated fact as
  it ignores the error, the anchoring on its own code is the cause, and only a restart or a different repair path
  can help.
- **Generality:** `none` yet. Evidence for the need is two stories on plain Java; the fix targets one Java library
  fact (BigDecimal scale), and says nothing about other facts a model might lack.
- **Result, loans L2 (`lean`, n=10, `--scale-hint`, same binary otherwise):** hidden 1/10 (baseline 0/10); step 1
  passed in 1 run (baseline 0); retries identical to the failed answer **9 of 19** (baseline 20 of 20); 1 of 19
  retries recovered (baseline 0 of 20). Prediction: at least 3 of 10 runs pass step 1: **not held** (1). Identical
  retries at most 12 of 20: held (9 of 19). The falsifier (identical at 18 of 20 or more) did not occur: the
  hint does break the copy in about half the retries. What the model then does (read from run 2): it fixes the test
  side only (`BigDecimal.ZERO.setScale(2, ...)` in the expected value) and leaves the code's early `BigDecimal.ZERO`,
  so the next failure is the mirror image, `expected: <0.00> but was: <0>`, and the hint repeats. The hint names
  the fact but not which side produced which value. 
- **Result, order L3 (`lean`, n=10, `--scale-hint`) against order L3 (`lean`, `feedback`, no hint):**

  | | no hint | `--scale-hint` |
  |---|---|---|
  | Runs that completed both steps | 3/10 | **8/10** |
  | Steps passed / step starts | 6 / 13 | **18 / 20** |
  | First-attempt step failures | 8 of 13 | 10 of 20 |
  | Retries identical to the failed answer | 14 of 15 | **5 of 13** |
  | Retries that recovered | 1 of 15 | **8 of 13** |
  | Hidden acceptance passed | 0/10 | 0/10 |

  Prediction: at least 3 of 10 runs pass step 1 within three attempts: held (8 of 10 completed both steps); hidden
  stays at 2 of 10 or fewer: held (0). The hidden failures of the 8 completed runs are the details the one-line plan
  does not carry: the boundary (`100.00 x 5 should total 450.00`) in 8 of 8, the message in 4 of 8. So the hint
  fixes a Java fact the model lacked and the gates now pass, while the code is still wrong where the plan was
  silent: gates green, behaviour wrong. That is the gap a plan with the details (C6) or a check against the feature
  must close; the hint cannot.
- **Status:** partly confirmed (copy broken, recovery not), sharpened in C9.

### C9. The scale hint names the sides (flag `--scale-hint-sides`)
- **Where:** `hints::scale_hint_with(digest, sides)`, `Implementer::with_scale_hint_sides`, `run_step
  --scale-hint-sides` (implies the hint). Commit: see the git log entry "Name the sides in the scale hint".
- **What:** adds one sentence to the C8 hint: "The code returned B; the test expected A." (In `assertEquals(expected,
  actual)` the actual value is what the production code returned.)
- **Why:** C8 on loans L2: the model repaired the test and left the code, then failed the other way round.
- **Prediction:** on loans L2 (`lean`) the sides variant passes step 1 in at least 3 of 10 runs (C8: 1 of 10) and
  has identical retries at 8 of 20 or fewer. **Falsified** if it passes step 1 in 2 or fewer runs: the model's fix
  of one side per attempt is not an information problem and the next candidate is a restart.
- **Generality:** `none`; one story, one failure fact.
- **Status:** queued.
