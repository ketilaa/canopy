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
- **Status:** done, see "G1 interim results" and "G1 final results" below.

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
- **Status:** run 2026-10-17, see the result below. (Earlier note: the scale hint (C8) and the checklist (C11) were the larger levers.)
- **Result R3 (L3 plans, `lean`, `--retry restart`, n=10 each; reference `feedback` from G1, step starts passed of started):**

  | Story | `restart` hidden | `restart` steps passed | `feedback` hidden | `feedback` steps passed |
  |---|---|---|---|---|
  | order | 0/10 | 6/13 | 0/10 | 6/13 |
  | loans | 0/10 | 0/10 | 0/10 | 2/11 |
  | shipping | 0/10 | 11/20 | 1/10 | 12/20 |
  | stock | 0/10 | 21/30 | 0/10 | 21/30 |

  Prediction: order `restart` at least 15 points above `feedback` per step started: **falsified** (0 points: 6 of 13 both).
  The falsifier (fewer than 10 points more) holds, and the replay's advantage (restart beating the retry as sent) does not survive
  the real harness on any of the four stories: restart is equal on order and stock and one or two steps lower on loans and
  shipping. The other predictions held: hidden pass 5 of 10 or fewer everywhere (0, 0, 0, 0), and none reached 8. Retries
  identical to the failed answer were not counted in this run.
  Reading: the L3 failures are systematic (a dropped message, boundary or scale), not draws that a fresh try fixes: three
  independent draws are not independent in outcome. Restart is not worth adopting; the flag stays off. Generality: `multi-story`
  (4 stories, one model, current prompt era); the replay was 7 cases and misled.
- **Status:** done.

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

### C10. A Java fact stated up front in the project context (`--context-extra`, measurement)
- **Where:** `run_step --context-extra "<text>"` appends a line to the Project context of every step (no prompt
  file changed; the text below is the experiment's variable). Commit: see the git log entry "Add --context-extra".
- **Text (cell "fact"):** "Java fact: BigDecimal.equals compares the scale, so 0 and 0.00 are not equal. Give a
  method's BigDecimal result the same scale in every branch, and write expected values in tests with that scale."
- **Why:** C8/C9: repair at retry rescued order L3 (8 of 13 retries) but almost nothing on loans, where the first
  attempt is already wrong in 10 of 10 runs; prevention at the first attempt is the other place to act. The project
  context already states stack facts of this kind for other tasks (namespace, available libraries), so this is the
  same tier: a fact, not a rule about one bug. (`CLAUDE.md` warns against rules for one observed failure; the
  evidence here is two stories and one Java fact, which this experiment tests, it does not adopt.)
- **Prediction (before the runs):** loans L2 (`lean`, baseline: step 1 passed first attempt in 0 of 10 runs): at least
  4 of 10 runs pass step 1 at the first attempt. Order L3 (`lean`, baseline: 8 of 13 step starts failed the first
  attempt): at most 3 of 10 step-1 first attempts fail; hidden pass stays at 2 of 10 or fewer (the plan still omits
  message and boundary). **Falsified** if loans L2 passes step 1 at the first attempt in 1 or fewer runs: a model
  that ignores the error text and the hint also ignores a fact stated up front, and the way forward is not more
  words but a different mechanism (a check against the plan's contract, or a restart).
- **Result, loans L2 (`lean`, n=10, fact in the project context):** hidden 0/10; step 1 passed in **0 of 10 runs
  at the first attempt** (baseline 0 of 10); retries identical to the failed answer 19 of 20 (baseline 20 of 20);
  0 recovered. Prediction (at least 4 of 10 pass at the first attempt) **falsified**, and the logged falsifier holds
  (1 or fewer). The model that ignores the error text, and mostly the hint, also ignores a fact stated up front.
- **Result, order L3 (`lean`, n=10, same fact):** step 1 failed its first attempt in **1 of 10 runs** (baseline
  `lean` 7 of 10; with the retry hint only 6 of 10, as the hint acts after the first attempt); runs that completed both
  steps 6/10 (baseline 3/10, hint 8/10); steps passed 15 of 19 starts (baseline 6 of 13); retries identical to the
  failed answer 4 of 8; hidden pass 0/10 (the boundary fails in 6 of 6 completed runs, the message in 3). Prediction
  (at most 3 of 10 step-1 first attempts fail): held (1); hidden at most 2 of 10: held (0).
- **Reading, the two stories together:** the same stated fact removes the order failure (forgetting `setScale`: 7 to 1
  of 10) and does nothing for loans (writing `BigDecimal.ZERO` in the test and in an early return: 10 of 10 either
  way). It is one fact in the model's reach for one failure and out of reach for the other, so a library fact in the
  context is not a general cure; whether it is worth its tokens depends on which mistake the model makes.
- **Generality:** `none`/`one-story` per cell; two stories, one library fact, one model.
- **Status:** works on order L3, `refuted` on loans L2; not adopted anywhere.

### G1 interim results (2026-10-17, `lean` unless stated; the loans and shipping cells; n=10)
| Cell | hidden passed | runs that completed all steps | step starts passed | first-attempt step failures | retries identical / recovered |
|---|---|---|---|---|---|
| order L2 `current` (E2) | 8/10 | 8 | 16/18 | 2 | 4 of 4 / 0 |
| order L2 `lean` | **9/10** | 9 | 18/19 | 2 | 2 of 3 / 1 |
| order L3 `current` (E3) | 0/10 | 1 | 2/11 | 9 | 16 of 18 / 0 |
| order L3 `lean` | 0/10 | 3 | 6/13 | 8 | 14 of 15 / 1 |
| loans L2 `lean` | **0/10** | 0 | 0/10 | 10 | 20 of 20 / 0 |
| loans L3 `lean` | 0/10 | 1 | 2/11 | 10 | 18 of 19 / 1 |
| shipping L2 `lean` | 5/10 | 5 | 14/19 | 7 | 7 of 12 / 2 |
- **lean vs current (prediction 1):** order L2: lean 9/10 against current 8/10; order L3: both 0/10 with the same
  failure mode and the same identical retries. Nothing in four comparisons says the extra rules help; lean is at
  least as good where it was measured. (Stories other than order have only `lean` so far.)
- **detail generalises? (prediction 2):** order L2 9/10 and L3 0/10 hold. **Loans L2 is 0/10**: not because the plan
  lacks detail but because every first attempt fails on one Java fact (BigDecimal scale, see C8-C10) that no stated
  fact or hint fixed. Shipping L2 is 5/10, one short of my bar (6 of 10); its failures are a mix of that scale fact
  (`expected <4.00> but was <4>`) and wrong values the model computed (a base price of 2.50, free shipping not
  applied). So "L2 carries enough" holds for order only; for loans it fails for a reason that is not the plan.
- **Reading so far:** the model's first attempt is the limiting step, in two of three stories it fails on the same
  Java fact, and the repair mechanisms (feedback, hint, fact, sides) mostly do not change that. What worked
  was a retry hint on the one-line order plan (C8). Remaining cells are queued: shipping L3, stock L2 and L3.

### C11. Loans at level L1: the checklist with the feature's own example values (`tasks/loans/plan-checklist.yaml`)
- **Where:** `tasks/loans/plan-checklist.yaml`; same steps, files and covered scenarios as the other loans plans
  (a unit test pins this). Step 1 lists 5 tests and step 2 lists 6, with the values taken from the feature's
  example tables and scenarios (`new BigDecimal("0.00")`, `"0.50"`, `"5.00"`, the dates, the messages), introduced
  with "MUST ... one test for each item ... do not leave any out", as in the order checklist (C4).
- **Why:** loans L2 is 0/10 because every first attempt writes `BigDecimal.ZERO` where the example says 0.00, and no
  hint or fact moved it (C8-C10). A Planner reads the scenarios, whose example tables carry exactly these values,
  so a checklist with concrete values is a plan shape it can produce mechanically. Order L1 (D1) passed 9/10.
- **Prediction (before the run, `--prompt lean`, n=10):** loans L1 passes the hidden test in at least 7 of 10 runs,
  and step 1 passes its first attempt in at least 7 of 10. **Falsified** if hidden pass is 2 or fewer: then even
  the concrete values do not get the model past the scale mistake, and the cure has to be a harness mechanism,
  not plan text.
- **Result (loans, `lean`, n=10, `plan-checklist.yaml`):** hidden **10/10**; every run completed both steps (20 of 20
  step starts passed); 1 first-attempt failure (a compile error in `LoanService`, recovered by a changed retry);
  0 identical retries. Against loans L2 (same prompt, prose plan): hidden 0/10, 10 of 10 first attempts failed,
  20 of 20 retries identical. Prediction (hidden at least 7 of 10, first attempts at least 7 of 10): **held, by a
  wide margin** (10/10, 19 of 20 first attempts). The falsifier (2 or fewer) did not occur.
- **Reading:** concrete test values, listed one per item and copied from the feature's example tables, removed the
  failure that no hint, fact or restart could touch: the model wrote `new BigDecimal("0.00")` where the checklist
  said so. A Planner can produce this plan shape mechanically from the Gherkin examples. With order (C4, 9/10) this
  is two stories in which L1 >> L2 or L1 >= L2. The cost is plan length: the loans checklist plan is about twice
  as long as its L2 plan; each step still carries 5 to 6 listed tests.
- **Generality:** `one-story` for the number (n=10), with order as a second story where L1 also passed.
- **Status:** kept as the loans plan shape that passes.

### C12. Shipping at level L1: the checklist with the feature's example values (`tasks/shipping/plan-checklist.yaml`)
- **Where:** `tasks/shipping/plan-checklist.yaml`; same steps, files and coverage as the other shipping plans (unit
  test). Step 1 lists 3 tests, step 2 lists 10, values copied from the feature's example tables.
- **Why:** C11: concrete values fixed loans (0/10 to 10/10). Shipping L2 is 5/10 and its failures mix the scale fact
  and wrong computed values; this tests the same plan shape on a third story, of a different kind (tiered
  computation).
- **Prediction (before the run, `lean`, n=10):** shipping L1 passes the hidden test in at least 8 of 10 runs (L2 5/10, L3
  1/10). **Falsified** if it is 5 or fewer: then concrete values are not a general lever, and the loans result
  was about its particular failure.
- **Result (shipping, `lean`, n=10, `plan-checklist.yaml`):** hidden **6/10** (L2 5/10, L3 1/10); 9 of 10 runs completed
  both steps (L2: 5 of 10); steps passed 19 of 20 starts (L2: 14 of 19); first-attempt failures 1 (L2: 7).
  Prediction (at least 8 of 10): **not held** (6); the falsifier (5 or fewer) did not occur either.
- **What the three hidden failures are (read from the code):** all three, in `run-02`, `run-06` and `run-07`: a parcel
  of 1 gram cost 2.50 instead of 4.00. The model computed the surcharge as `ceil((weight - 1000) / 500.0)`, which is
  -1 below 1000 g, and never applied the rule it had been told in prose ("up to 1000 grams the cost is the base
  price"). Its own tests, which were exactly the checklist's values, never tried a weight below 1000 g, so
  nothing failed. The checklist copies the values of the feature's example tables, and the feature never shows a
  weight under 1000 g: the examples did not cover the whole rule, the model implemented what the examples showed.
- **Reading:** concrete values make the gates pass (shipping 9/10 completed, loans 10/10) and fix the model's own
  tests; they do not make the code right where the examples are silent. Together with C6 (order L3: gates green,
  behaviour wrong): the unresolved gap is behaviour the plan and the examples do not mention but the feature's rule
  states.
- **Generality:** `one-story` for the number; three stories now show the same ordering of the detail levels for the gates
  (L1 completes at least as often as L2, both far above L3), and the hidden-test result is not uniform (order 9/10 and
  loans 10/10 at L1, shipping 6/10).
- **Status:** kept as the shipping plan shape with the best completion; its hidden-test gap is the finding.

### G1 final results (2026-10-17; n=10 per cell; Qwen3-14B Q4_K_M; plain Java Maven, JUnit 5; one stack, one model)
Hidden acceptance passed / runs that completed all steps (step starts passed of started). L1 = checklist with the
feature's example values, L2 = prose rules, L3 = one line per step. `cur` = `--prompt current`, `lean` = `--prompt lean`.

| Story | L3 | L2 | L1 |
|---|---|---|---|
| order (2 steps) | 0/10 cur (2/11 steps), 0/10 lean (6/13) | 8/10 cur, **9/10** lean | 9/10 cur (D1) |
| loans (2 steps) | 0/10 lean (2/11) | **0/10** lean (0/10) | **10/10** lean (20/20) |
| shipping (2 steps) | 1/10 lean (12/20) | 5/10 lean (14/19) | 6/10 lean (19/20) |
| stock (3 steps) | 0/10 lean (21/30) | **10/10** lean (30/30) | not run |

- **Prediction 1 (lean vs current):** compared only on order (9 vs 8 at L2; 0 vs 0 at L3): not falsified. The `current`
  arms for loans, shipping and stock were not run (queued, cut for time), so for those stories it is untested.
- **Prediction 2 (detail generalises):** L3 at most 5 of 10 on each story: held (0, 1, 0). L2 at least 6 of 10 on each
  story: held for stock only (10); loans 0, shipping 5. The falsifier (L3 at 8 or more on two stories) did not
  occur; the other (L2 at 4 or fewer on two stories) did not either (only loans), but my claim that "L2 carries
  enough" is not general: it held for order and stock and failed for loans and, by one run, for shipping.
- **Prediction 3 (what L3 loses):** of the hidden-test failures of L3 runs that completed (order 8 of 8 on the
  boundary and 4 on the message; loans 3 on messages or exception types; shipping 1 on the message; stock 3 on messages,
  types or immutability), almost all name a message, an exception type or a boundary a one-line step drops: held. The
  gate failures of L3 are another matter: scale (`0` vs `0.00`) in order and loans, a test using Mockito (not on the
  classpath) and wrong computed values in stock and shipping.
- **What did generalise:** (a) L3 fails on all four stories; (b) the first attempt is the limiting step, and on the
  stories that fail, retries repeat the failed answer most of the time (order L3 14 of 15, loans L2 20 of 20, loans L3
  18 of 19, shipping L3 12 of 16, stock L3 10 of 20); (c) a layered, immutable design (stock L2) was no problem.
- **What did not generalise:** L2's sufficiency; and the checklist's hidden-test result (loans 10/10, order 9/10,
  shipping 6/10).
- **Not run (cut for time):** R3 (restart retry in the real harness), `current` arms of the new stories, stock at L1.
- **Generality:** `multi-story` for the level ordering of the gates (4 stories, one stack, one model); `one-story`
  per number.

### C13-C16. What in the checklist matters: values or wording; a rule-coverage item; stock at L1 (four batches, queued on the maintainer's request)
- **Plans (all `--prompt lean`, n=10, same binary and tags; a unit test pins that each keeps its story's steps, files and
  coverage):**
  - C13 `tasks/loans/plan-values-only.yaml`: the loans checklist's numbered items with their concrete values, without
    the "MUST contain one test for each item ... check the list before you submit" wording.
  - C14 `tasks/loans/plan-wording-only.yaml`: the "MUST ... check the list" wording and numbered items, with the items
    described in words and no concrete values or dates.
  - C15 `tasks/shipping/plan-checklist-rules.yaml`: the shipping checklist (6/10, three failures all a weight under
    1000 g costing a negative surcharge) plus one item for the rule the examples never exercised: a weight of up to
    1000 grams costs the base price, with values (EUROPE 999 g, WORLD 500 g) different from the hidden test's (DOMESTIC
    1 g). The item is derivable from the prose rule mechanically ("one item for each rule, including the side of the
    threshold the examples do not show").
  - C16 `tasks/stock/plan-checklist.yaml`: the stock plan at L1 (5, 3 and 7 listed tests with the feature's values), to
    see whether the checklist helps or hurts a story that passes at prose level (L2 10/10).
- **Why:** the checklist bundles two ingredients (values, wording) and I never separated them, so a Planner cannot yet be
  told which to produce; shipping's remaining failures suggest the checklist should also cover rules whose examples are
  missing; and the checklist may cost something on a story that does not need it.
- **Predictions (before any of the four runs):**
  - C13 values only passes the hidden test in at least 7 of 10 runs; C14 wording only in at most 2 of 10 (loans L2, prose
    without values, is 0/10). **Falsified** (that values are the active ingredient) if C13 is 4 or fewer or C14 is 6 or more.
  - C15 passes in at least 8 of 10 runs (checklist 6/10, all its failures the sub-1000 g case). **Falsified** if 6 or fewer:
    a rule-coverage item does not close the gap.
  - C16 passes in at least 9 of 10 runs (L2 10/10, the checklist should not hurt). **Falsified** if 7 or fewer: the
    checklist costs something on a story that already passes.
- **Generality:** `none` yet; one model, plain Java, four small greenfield stories.
- **Result C13/C14 (loans, `lean`, n=10 each; reference: checklist = values and wording 10/10, prose without values 0/10):**

  | Plan | Hidden passed | Runs completing both steps | First-attempt step failures | Retries identical / recovered |
  |---|---|---|---|---|
  | values and wording (C11) | 10/10 | 10 | 1 of 20 | 0 of 1 / 1 |
  | **C13 values only** | **10/10** | 10 | 2 of 20 | 0 of 2 / 2 |
  | **C14 wording only** | **0/10** | 0 | 10 of 11 | 20 of 20 / 0 |
  | prose, no values (L2) | 0/10 | 0 | 10 of 10 | 20 of 20 / 0 |

  Predictions: C13 at least 7 of 10: held (10). C14 at most 2 of 10: held (0). The falsifiers (C13 four or fewer, C14
  six or more) did not occur. On loans the active ingredient of the checklist is the concrete values; the
  "MUST contain one test for each item ... check the list" wording adds nothing (C14 fails exactly as prose does: the
  same `expected <0.00> but was <0>`-type scale mismatch in 27 of 30 failure lines) and is not needed (C13 passes without it).
  Generality: `one-story`, one failure mode (scale); order (C4) did not separate the two ingredients.
- **Result C15 (shipping, `lean`, n=10, checklist plus the rule-coverage item):** hidden **9/10** (checklist 6/10, prose
  5/10); all 10 runs completed both steps, 20 of 20 steps passed, 0 first-attempt failures (checklist: 1; prose: 7).
  Prediction (at least 8 of 10): held. The falsifier (6 or fewer) did not occur. The one failure (run-08) is the same
  defect as before, a 1-gram parcel costing 2.50: a rule-coverage item with values from other zones (999 g, 500 g)
  fixed the gap in 9 of 10 runs and did not generalise to DOMESTIC 1 g in one. The item is derivable from the prose rule
  ("up to 1000 grams the cost is the base price"): one more item for the threshold side the examples do not show.
  Generality: `one-story`; the item names the gap I found in this story's hidden test, so this is evidence that coverage
  items for unexemplified rules work, not that a Planner would find them unaided.
- **Result C16 (stock, `lean`, n=10, checklist plan):** hidden **6/10** (prose L2 10/10); all 10 runs completed all 3 steps
  (30 of 30 steps passed), 4 first-attempt failures all recovered by a changed retry (0 identical). The four hidden failures
  are compile errors in the hidden test against the fixed API: `StockItem` has no `onHand()`. In those four runs the
  model wrote `StockItem` as a `final class` with `getOnHand()`, although every version of the instruction says "Create
  the record StockItem ... with the components String sku and int onHand" (checklist 6 records, 4 classes; prose 10 of 10
  records). The checklist items speak of "has onHand 15" and never write the accessor call. Prediction (at least 9 of 10):
  **falsified**, and the falsifier (7 or fewer) holds: on a story that passes at prose level the checklist cost 4 runs, by
  changing the API the plan specified.
- **Reading C16:** a long, value-dense step can dilute a structural constraint that a shorter step keeps (record, accessor
  names). The hidden test is an API contract; the plan stated the API once in prose and then illustrated behaviour in items
  that never used the accessor syntax. Not separated here: dilution by length against the "has onHand" wording. Candidate
  fix to test: write each item as the call it asserts (`new StockItem("SKU-1", 10).receive(5).onHand() is 15`).
- **Summary of C13 to C16:** (1) concrete values are the active ingredient of the checklist (loans 10/10 with values only, 0/10
  with wording only); (2) a coverage item for the side of a rule the examples do not show closed shipping's gap (6/10 to 9/10);
  (3) a checklist can cost API fidelity (stock 10/10 to 6/10). Generality: each `one-story`, one model, plain Java.
- **Status:** done.

## 2026-10-17: C17, stock at the checklist level with an API block

- **Change (plan only, no prompt change):** `tasks/stock/plan-checklist-api.yaml` is `plan-checklist.yaml` (same steps, files,
  coverage) with, in each step, a line `API the tests use: ...` naming the constructor, accessors and method signatures the
  tests call, and every item written as the call it asserts (`new StockItem("SKU-1", 10).receive(5).onHand() is 15`).
- **Why:** C16 lost 4 of 10 runs to `StockItem` written as a class with `getOnHand()`, the items never using the accessor.
- **Prediction (before the run):** at least 9 of 10 runs pass the hidden test (checklist 6/10, prose 10/10). **Falsified** if 7 or
  fewer: then the cost is not the missing accessor syntax and the next candidate is length/dilution.
- **Generality:** `none` yet; one story, one model.
- **Result C17 (stock, `lean`, n=10, checklist plan with API block):** hidden **10/10** (checklist 6/10, prose 10/10); all 10 runs
  completed all 3 steps, 26 of 30 steps passed at the first attempt, 4 needed a second. No run used `getOnHand()`. Prediction (at
  least 9 of 10): held; the falsifier (7 or fewer) did not occur. Side finding: all 10 wrote `StockItem` as a `final class`, none as
  the record the instruction names (checklist 4 of 10 classes, prose 0 of 10), with `sku()` and `onHand()` accessors, which is all the
  hidden test calls. So the earlier loss was the accessor names, not the class form: stating the API as the calls the tests make
  fixed it, and the model still ignored "record". Generality: `one-story`, one model; the API block here was written by hand from
  the hidden test's calls, so it shows the form works, not that a Planner would derive it.
- **Status:** done.

## 2026-10-17: Plan audit (analysis, not a prompt or plan change)

- **Tool:** `examples/plan_audit.rs <feature> <plan>...` lists the literals a feature states (quoted strings, numbers, dates) that
  appear in no step instruction of a plan. Substring match, so small integers (`1`, `3`) match too easily.
- **Success criterion (stated before looking):** the count of missing literals orders the cells like the hidden-test pass rate (L3
  worst, L2 or L1 best) on at least three of four stories.
- **Result (missing literals, L3 one line / L2 prose / L1 checklist; hidden pass rate in brackets):**

  | Story | L3 | L2 | L1 |
  |---|---|---|---|
  | order (19 literals) | 15 (0) | 14 (9) | 5 (9) |
  | loans (16) | 13 (0) | 9 (0) | 0 (10) |
  | shipping (23) | 11 (1) | 6 (5) | 0 (6) |
  | stock (14) | 14 (0) | 10 (10) | 0 (6); with API block 0 (10) |

  Held on order, loans and shipping (L3 worst, L1 fewest missing); **failed on stock**, where L1 has the fewest missing literals and
  the lower pass rate. Three of four meets the criterion. What it cannot see, by construction and as seen: the shipping rule
  coverage item changes 6/10 to 9/10 and the audit reads 0 both times; stock's cost was API shape, not a literal; L2 against L3 on
  loans is 9 against 13 missing and 0 against 0 passed.
- **Reading:** the audit measures how much of the feature's concrete content reaches the plan, which tracks plan detail, and detail is
  what separated L3 from L1 everywhere. It is a necessary-condition check (a plan missing most literals fails), not a predictor of
  the winner among detailed plans. Not wired into the Planner; a candidate for "audit, then revise" in course step 3 only together
  with checks for API shape and rule coverage.
- **Generality:** `none`; four stories, twelve cells, one model, a coarse criterion.
- **Status:** done.

## 2026-10-17: T1-T3, a fifth greenfield story with a different shape (tickets: a state machine with a time window)

- **Story:** `tasks/tickets/` (feature, task, hidden test, baseline repo `dogfooding/java-tickets` tag `baseline-tickets-001`): a
  single-journey ticket with an enum status (ISSUED, VALIDATED, CANCELLED), a 90-minute validity window, four guarded transitions
  with exact messages, a repository and a service (3 steps like stock). Different from the four earlier stories: states and
  time instead of arithmetic or counting. The hidden test was checked against a reference implementation (6 of 6 pass) before any
  run. The boundary (exactly 90 minutes is valid) is not exemplified in the feature, like shipping's 1000 g.
- **Plans (all `--prompt lean`, n=10; a unit test pins that they share steps, files and coverage):** T1 `plan-oneline.yaml` (L3),
  T2 `plan-signatures.yaml` (L2, prose rules with exact messages and the check order), T3 `plan-checklist-api.yaml` (L1+, the shape
  written down in `docs/design/implementer-next-plan-2026-10-17.md`: prose rules, an `API the tests use:` line, numbered items with
  concrete values written as calls, a coverage item for the boundary with values different from the hidden test's: 14:00 to 15:30).
- **Why:** to test the plan shape on a story it was not derived from. Caveat: I wrote the plans and the hidden test; the plans were
  written from the feature and the shape, not from the hidden test's code, but the API names are the ones both sides share.
- **Audit (missing literals of 15; the audit reads `LocalDateTime.of(...)` as missing the date and `00`):** T1 15, T2 9, T3 2.
- **Predictions (before any run):** T1 passes the hidden test in at most 3 of 10 runs (**falsified** if 6 or more); T3 in at least 8
  of 10 (**falsified** if 5 or fewer: the shape does not generalise); T2 strictly between, and not above T3. A failure of T3 should
  be a message, a boundary or the API, which is what the shape targets; if it is something else (the enum, the time arithmetic), the
  shape misses a category.
- **Generality:** `none` yet; one model, plain Java; the first story the shape was not fitted to.
- **Result T1-T3 (tickets, `lean`, n=10 each; hidden passed / runs completing all 3 steps / steps passed at the first attempt):**

  | Plan | Hidden | Completed | First-attempt steps | Audit (missing of 15) |
  |---|---|---|---|---|
  | T1 L3 one line | 0/10 | 3 | 8 of 16 started (9 passed in all) | 15 |
  | T2 L2 prose | **10/10** | 10 | 30 of 30 | 9 |
  | T3 L1+ (API, values, boundary item) | **10/10** | 10 | 30 of 30 | 2 |

  Predictions: T1 at most 3 of 10: held (0). T3 at least 8 of 10: held (10); the falsifier (5 or fewer) did not occur, so the plan
  shape works on a story it was not fitted to. T2 strictly between and not above T3: **falsified** (10 = 10; the prose plan already
  carries this story, as on order and stock). T3 failures: none to read.
  T1 failures: 7 of 10 runs never got past step 1 (a test or the code throwing `IllegalArgumentException` where the rule is
  `IllegalStateException`, the exception type a one-line step drops, in 5 attempts of 5 per run seen); 3 runs completed and failed
  the hidden test. The failure class is the one the shape names (a dropped exception type and message), not a new one.
- **Reading:** on tickets the detail that matters is the exception types and messages in the prose; the API line, the numbered values
  and the boundary item added nothing measurable on top of L2 (the 90-minute boundary was met by "more than 90 minutes after" in the
  prose, unlike shipping's "up to 1000 grams" whose other side the model got wrong). So the shape is sufficient here but not shown
  necessary; the stories where L2 is not enough are loans (scale), shipping (threshold side) and, with the checklist, stock (API).
  The audit again orders L3 worst and does not separate L2 from L1+.
- **Generality:** `multi-story` for "L3 fails, a detailed plan passes" (five stories now); `one-story` for the tickets numbers; I wrote
  the story, the plans and the hidden test, one model.
- **Status:** done.

## 2026-10-17: Analysis of the residual first-attempt failures on order and shipping (fresh runs, no change)

- **Runs (all `--prompt lean`, n=10, same binary; the transcripts were read, the run directories are scratch):**

  | Cell | Hidden | Steps passed at first attempt | Earlier number |
  |---|---|---|---|
  | shipping L1 (checklist + rule item) | 10/10 | 20 of 20 | 9/10 (C15) |
  | shipping L2 (prose) | 3/10 | 16 of 20 started | 5/10 |
  | order L1 (checklist) | 10/10 | 20 of 20 | 9/10 on `current` (D1) |
  | order L2 (prose) | 7/10 | 14 of 17 started | 9/10 |

  The same cell moves by two or three runs between batches (shipping L2 5 to 3, order L2 9 to 7), so a difference of 2 in n=10 is
  noise; the L1 cells now read 10/10 twice.
- **Class 1, shipping L2: `each started 500 grams` computed with floor division.** 7 of 10 runs wrote `(weightGrams - 1000) / 500` (or
  the same without a `> 1000` guard), 3 wrote `Math.ceil`; the ceiling runs passed 3 of 3, the floor runs 0 of 7. The model's own
  tests use 1500, 2000, 2500, 3000 only, values where floor and ceiling agree, so the gate passes and the hidden test (1001 g
  should cost 5.50, got 4.00; 1501 g should cost 7.00, got 5.50) fails (runs 05, 07, 08). Where the model wrote a test with the
  prose's own example the gate caught it (expected 7.00 but 5.50, run 03). Without the `> 1000` guard a weight under 1000 gives a
  negative step and a cost of 2.50 (expected 4.00; runs 06 and 09, the defect C15 saw). One gate failure (run 04, a rejected-weight
  test) is a different defect, not diagnosed. All 10 L1 runs used a round-up form (`+ 499`, `ceil`, remainder check). Cause: the
  prose gives the ranges (1001 to 1500 adds 1.50) and the model still chose the common truncating idiom; the L1 items 1001 and 1501
  with their costs pin the intended result.
- **Class 2, order L2: the model's own test contradicts itself at the threshold.** In all 3 failing runs (05, 06, 07) the step's
  own test file asserts two different totals for the same input: `place(.., 500.00, 1)` expects 500.00 in the "no discount" test and
  450.00 in the "discount" test (the code is right, the test is wrong), or a 10 times larger expected value (4500 for 450.00). The
  retry shows the failing assertion and the model resubmits the same contradiction in all three attempts (0 of 3 runs recovered). L1
  gives the values, so the model has nothing to invent at the threshold: 20 of 20 steps first attempt.
- **Attribution:** planner/plan detail in both cases. The implementation is correct or incorrect in the way the plan's missing
  values allow; the harness recovery is not the bottleneck (a retry that sees "expected 500.00 but was 450.00" does not change the
  test). Both classes are the same as before (a boundary the plan does not give a value for), now with a mechanism for each.
- **What this does not show:** whether the whole checklist is needed or only the boundary values. L1 adds many more values than the
  failures need.
- **Next experiment (not run; prediction first):** L2 plus one sentence each, not a checklist: shipping "1001 grams costs 5.50 and
  1501 grams costs 7.00"; order "500.00 gets the discount and 499.99 does not, an order of 500.00 totals 450.00". n=10 per story.
  Predict at least 8 of 10 on both; falsified at 5 or fewer on either (then the checklist's structure, not the boundary values,
  is the ingredient). Generality of this analysis: `two-story` (order, shipping), one model, one batch each; the class mechanism comes
  from reading the code of 10 runs per cell, not from a count alone.
- **Status:** analysis done; experiment queued.

## 2026-10-17: B1/B2, boundary values only (L2 plus one sentence), shipping and order

- **Plans:** `tasks/shipping/plan-boundary.yaml` is `plan-signatures.yaml` (L2) plus "So a DOMESTIC parcel of 1001 grams costs 5.50
  and one of 1501 grams costs 7.00."; `tasks/order/plan-boundary.yaml` is L2 plus "so an order of 500.00 totals 450.00 and an
  order of 499.99 totals 499.99;". No checklist, no MUST wording, no API line, no extra items. `--prompt lean`, n=10 each.
- **Why:** the analysis above: both residual L2 failure classes are a boundary the plan gives no value for; the checklist adds much more.
- **Prediction (before the runs):** at least 8 of 10 on both (L2 now: shipping 3/10, order 7/10; L1: 10/10 both). **Falsified** at
  5 or fewer on either story: then the checklist's structure (numbered items, one per test) and not the boundary values is the
  ingredient. The shipping values (1001, 1501) are in the hidden test, so a pass shows the values work, not that a Planner finds them.
- **Generality:** `none` yet; two stories, one model.
- **Result B1/B2 (`lean`, n=10 each):**

  | Story | L2 | L2 + boundary sentence | L1 checklist |
  |---|---|---|---|
  | order | 7/10 | **8/10** (17 of 19 steps first attempt) | 10/10 |
  | shipping | 3/10 | **3/10** (14 of 20 steps first attempt) | 10/10 (with rule item) |

  Order: prediction (at least 8) held, the falsifier did not occur; the 2 failures are again the model's own test asserting two
  totals for the same 500.00 input (`expected 500.00 but was 450.00`), despite the sentence saying 500.00 totals 450.00.
  Shipping: prediction **falsified** (3 of 10, falsifier 5 or fewer). What changed and what did not: the ceiling class is gone in the runs
  that read it (the 3 passing runs and run-05's compile all use `Math.ceil`), but 5 of the 7 gate failures (runs 03, 06, 07, 08, 09) are
  `expected 4.00 but was 2.50`, the weight under 1000 g costing a negative surcharge (4.00 minus 1.50), the side of the rule the
  boundary sentence does not mention; run 01 and run 05 are other defects (free shipping and a rejected weight, 91.00 for a 1000 g
  expectation). So on shipping the boundary values on one side moved the failure to the other side; this is the C15 gap again, and
  the L1 plan closed it only because it lists both.
- **Reading:** a single sentence of values per story is not the checklist's effect. Order's L2 gap is partly closed by it (7 to 8,
  within noise) and shipping's not at all. The values must cover each side of each rule, which is what a Planner has to derive: for a
  threshold or step rule, an item on each side and at the boundary. The structure (numbered items, one test per item) is not yet
  separated from coverage: next B3 below. Generality: `two-story`, one model.
- **Status:** done.

### B3. Shipping: both sides of the rules in one sentence each, still no checklist (prediction first)
- **Plan:** `tasks/shipping/plan-boundary-both.yaml` = `plan-boundary.yaml` plus "A parcel of 500 grams or of 1000 grams costs the
  base price: DOMESTIC 4.00." (the side the L2 failures got wrong). `--prompt lean`, n=10.
- **Prediction:** at least 8 of 10. **Falsified** at 5 or fewer: then the checklist's structure (one numbered item per test, with the
  MUST wording) is an ingredient, not only values on every side.
- **Generality:** `none`; one story. The 500 g value is not the hidden test's (it uses 1 g and 1000 g).
- **Result B3 (shipping, `lean`, n=10):** hidden **2/10** (B2 3/10, L1 10/10); 2 runs completed both steps, 11 of 19 steps passed at
  the first attempt. Prediction (at least 8): **falsified**, and the falsifier (5 or fewer) holds. The sub-1000 g defect is gone (no
  `expected 4.00 but was 2.50` in any run) and a different one took its place: in 8 of the 8 gate-failing runs (01 to 07, 10) the step's
  own `testFreeShipping` fails with `expected 0.00 but was 4.00`. The code is right in the runs read (`DOMESTIC && orderValue >= 100.00`
  returns 0.00, round-up steps); the model's test asserts the wrong value for the just-below case the step's own text tells it to test
  ("free shipping at and just below 100.00"): `cost(DOMESTIC, 500, 99.99)` expected 0.00 in one line and 4.00 in the next (run 02, run
  03), or 5.50 for a 1001 g parcel. It is the self-contradicting test of order L2: the plan names a test case whose expected value it
  never gives, and the model invents one.
- **Reading B1 to B3:** each boundary sentence fixed the side it named and the next unnamed side failed. What the L1 plans do that
  none of the sentences did is give an input and an expected value for **every** case the step's test sentence mentions. So the
  ingredient is probably complete input/output pairs for each case the plan asks the step to test, not the numbered structure and not
  boundary values on one side. A Planner's rule would be: for every case the plan tells the step to test, state its expected value.
  Not yet separated from the checklist's structure: B4 below.

### B4. Shipping: an expected value for every case the test sentence names (prediction first)
- **Plan:** `tasks/shipping/plan-boundary-all.yaml` = `plan-boundary-both.yaml` plus "A DOMESTIC order of 99.99 is not free: a 500 gram
  parcel costs 4.00." The test sentence ("base prices, the weight steps and their boundaries, the rejected weights and their message,
  and free shipping at and just below 100.00 and for the other zones") is unchanged: now each case has a stated value (base prices
  given, steps 1001/1501, 500 and 1000 g, rejected weights and message given, 100.00 free, 99.99 4.00, other zones the base price).
  `--prompt lean`, n=10.
- **Prediction:** at least 8 of 10. **Falsified** at 5 or fewer: then the numbered checklist structure (one item per test, MUST
  wording) is an ingredient.
- **Generality:** `none`; one story; the 99.99 value is the hidden test's own boundary value (it asserts 99.99 and 100.00), so a
  pass shows values on every named case work, not that a Planner finds them.
- **Result B4 (shipping, `lean`, n=10):** hidden **5/10** (B3 2/10, L1 10/10); all 5 runs that completed both steps passed; 11 of 16
  steps first attempt. Prediction (at least 8): **falsified**, and the falsifier (5 or fewer) holds, though 2 to 5 is a real
  improvement. Of the 5 gate failures, 4 are again `testFreeShipping expected 0.00 but was 4.00`, with the plan now saying
  "a DOMESTIC order of 99.99 is not free: a 500 gram parcel costs 4.00": the model's test still asserts `cost(DOMESTIC, 500, 99.99)`
  is 0.00 (runs 01, 08), or leaves a placeholder test (run 06). One is a weight step (run 04, 1001 g expected 5.50, got 4.00).
- **Reading:** the plan's test sentence, "free shipping at and just below 100.00", reads as "free at 100.00 and free just below 100.00";
  the model follows it over the rule sentence and over the added value. That is a contradiction inside the plan that I wrote (the same
  sentence is in the L2 plan, where it has been there for every shipping L2 run), not a missing value. By the project's own habit
  (read the prompt for contradictions before adding anything) the next step is to fix the sentence, not add more values: B5.

### B5. Shipping: the test sentence reworded so that it cannot be read as "just below is free" (prediction first)
- **Plan:** `tasks/shipping/plan-boundary-all-reworded.yaml` = `plan-boundary-all.yaml` with the test sentence's "free shipping at
  and just below 100.00 and for the other zones" replaced by "free shipping for DOMESTIC at an order value of 100.00, the normal
  cost just below it (99.99), and the other zones at 100.00". Nothing else changes. `--prompt lean`, n=10.
- **Prediction:** at least 8 of 10. **Falsified** at 5 or fewer: then the wording is not the cause and the checklist structure
  remains a candidate.
- **Generality:** `none`; one story; the sentence has the same shape in the order plan ("the threshold").
- **Result B5 (shipping, `lean`, n=10):** hidden **10/10**; all 10 runs completed both steps, 20 of 20 steps at the first attempt, no
  retry. Prediction (at least 8): held; the falsifier did not occur. So a plan with values for every case the test sentence names
  and a test sentence that cannot be read against the rule reaches the L1 checklist's result (10/10) on shipping, without numbered
  items, MUST wording or an API line. B5 stacks four changes on L2 (1001/1501 values, the base-price side, 99.99 costs 4.00, the
  reworded sentence), so which of them is needed is not known: B6.

### B6. Shipping: the original L2 plan with only the test sentence reworded (prediction first)
- **Plan:** `tasks/shipping/plan-signatures-reworded.yaml` = `plan-signatures.yaml` (L2: 3/10 in the fresh run, 5/10 before) with
  only the same sentence replaced. `--prompt lean`, n=10.
- **Prediction:** at most 5 of 10: the floor-division class (7 of 10 L2 runs) is not touched by the wording, so the wording alone should
  not reach the L1 result. **Falsified** at 8 or more: then the sentence was the main defect of every shipping L2 plan.
- **Generality:** `none`; one story.
- **Result B6 (shipping, `lean`, n=10, L2 with only the test sentence reworded):** hidden **5/10** (L2 as written: 3/10 fresh, 5/10
  before); 6 runs completed both steps, 15 of 19 steps first attempt. Prediction (at most 5): held; the falsifier (8 or more) did not
  occur. The wording removed the free-shipping failure (no `testFreeShipping` failure in any run), and the weight failures
  remained: the model's own weight-boundary tests assert wrong expected values (`expected 4.00 but was 91.00`, `expected 15.00 but
  was 102.00`: a 30000 g case with an invented value; run 09 also 8.50 for 10.00), and run 01 used the floor formula (hidden: 1501 g
  7.00, got 5.50).
- **Summary B1 to B6 (shipping, hidden pass of 10):** L2 3 to 5; L2 + 1001/1501 sentence 3; + base-price side 2; + 99.99 value 5;
  + reworded test sentence (all four) **10**; L2 + reworded sentence only 5; L1 checklist 10. Order: L2 7, + boundary sentence 8, L1 10.
  - Two separate defects sit in the L2 plans. (1) A test sentence that contradicts the rule ("free shipping at and just below 100.00"),
    which the model follows against both the rule and a stated value: removing it removes that failure class. (2) Test cases whose
    expected values the plan does not give (30000 g, the weight steps): the model invents them, and the gate catches the wrong
    ones. Each is fixed by its own change, and only both together reach 10/10 without a checklist.
  - The checklist's structure (numbered items, MUST wording) was not needed on shipping: values for each case named in the test
    sentence plus a test sentence free of contradictions gave 10/10 (B5). Not shown on any other story.
  - A Planner rule that follows: every case the step is told to test has an input and an expected value in the plan, and the test
    sentence is derived from those cases, not written as free prose beside the rule. A deterministic check is possible: list the
    cases a test sentence names and the values the plan states.
- **Generality:** `one-story` for B3 to B6 (shipping), `two-story` for the first-order claim (order's boundary sentence 7 to 8, within
  noise); one model; plans written by hand; the 99.99 and 1001/1501 values are values the hidden test uses.
- **Status:** done. Not done: the same rewording on order (its sentence says "the threshold") and a test that the combined change
  also holds on a story with a different shape (tickets already passes at L2).

## 2026-10-17: B7/B8, order's test sentence reworded (counterparts of B6 and B5)

- **Change:** order's step-1 test sentence "the total with and without the discount, the threshold, ..." names a case (the threshold)
  without a value; the model's own test then asserts two totals for the same 500.00 input (3 of 10 L2 runs, 2 of 10 in B2).
  Reworded to "the total with the discount (an order of 500.00 totals 450.00), the total without it (an order of 499.99 totals
  499.99), ...". B7 `tasks/order/plan-signatures-reworded.yaml` = L2 plus only that sentence; B8 `tasks/order/plan-boundary-reworded.yaml`
  = the boundary plan (which already states 500.00 and 499.99 in the rule sentence) plus the reworded test sentence. `--prompt lean`, n=10.
- **Prediction (before the runs):** B7 at least 8 of 10 (L2 7/10 and B2 8/10, whose failures are this contradiction); B8 at least
  9 of 10. **Falsified** if B7 is 6 or fewer (the sentence is not the cause on order; shipping needed values too) or B8 is 7 or fewer.
  Order's L2 has no second defect that I know of (shipping's weight cases have no counterpart), so I expect the wording alone to
  suffice here; if it does, the two stories differ in what the plan must supply.
- **Generality:** `none`; order only; the test sentence is the factor changed.
- **Result B7/B8 (order, `lean`, n=10):**

  | Plan | Hidden | Completed | First-attempt steps |
  |---|---|---|---|
  | L2 (earlier, fresh) | 7/10 | | 14 of 17 started |
  | B7 L2 + reworded test sentence | **4/10** | 4 | 8 of 14 started |
  | B2 L2 + boundary sentence (earlier) | 8/10 | | 17 of 19 |
  | B8 boundary sentence + reworded test sentence | **9/10** | 9 | 19 of 20 |
  | L1 checklist | 10/10 | | 20 of 20 |

  Predictions: B8 at least 9 of 10: held. B7 at least 8 of 10: **falsified** (4, falsifier 6 or fewer). B7 failed in all 6 non-passing runs with
  `testPlaceOrderWithDiscount expected 450.00 but was 4500.00`: the test calls `Order.place(.., new BigDecimal("500.00"), 10)`, so the
  model read "an order of 500.00 totals 450.00" as a unit price of 500.00 (and chose a quantity of 10, giving 4500.00). The
  sentence states an outcome, not a call; in B8 the rule sentence ("so an order of 500.00 totals 450.00...") precedes the same
  words and 9 of 10 read it correctly, so this is a fragile reading, not a stable one (B7 4/10 against 7/10 for plain L2 is
  also within what n=10 can do, but the 6 failures share one mechanism and it is new).
- **Reading:** rewording a test sentence is not sufficient or safe by itself: the case needs its call arguments, not an
  outcome described in domain words. Shipping's values were call arguments (`1001 grams`, `DOMESTIC`, `99.99` as the order
  value); order's "an order of 500.00" was ambiguous between a unit price and a total. Refines the Planner rule: every case the step
  is told to test is stated as the call's arguments and the expected result.

### B9. Order: the cases stated as calls (prediction first)
- **Plan:** `tasks/order/plan-signatures-calls.yaml` = L2 with the test sentence "the total with the discount (unit price 125.00,
  quantity 4: total 450.00), the total without it (unit price 124.99, quantity 4: total 499.96), ..." and no other change. `--prompt lean`, n=10.
- **Prediction:** at least 8 of 10 (against B7 4/10 with the outcome-only wording, L2 7/10). **Falsified** at 6 or fewer: then calls
  as arguments are not enough either and order's remaining defect is elsewhere.
- **Generality:** `none`; order only.
- **Result B9 (order, `lean`, n=10):** hidden **6/10** (B7 4/10, L2 7/10, B8 9/10); 6 runs completed both steps, 16 of 20 steps first
  attempt. Prediction (at least 8): **falsified** (falsifier 6 or fewer). Step 1 now passes its own gate in every run (no
  `4500.00` and no 500.00 contradiction); all 4 gate failures are in step 2, `OrderServiceTest.testPlaceOrderAndFindById expected
  500.00 (or 475.00) but was 450.00`: the step's test sentence ("placing an order and finding it") gives no values, so the model
  reuses step 1's threshold input (unit price 125.00, quantity 4 from the plan text) and expects the undiscounted total. The class
  is the same, one step later: a case the plan tells the step to test, with no values, filled by an invented and wrong one.
- **Reading:** the Planner rule holds per step, and every step of the plan needs it, not the step with the rule. Shipping's B5 had
  one such sentence per step covered (step 1 had no weight rule). B10 gives step 2 a value too.

### B10. Order: the cases of both steps stated as calls (prediction first)
- **Plan:** `tasks/order/plan-signatures-calls-both.yaml` = B9 plus in step 2 "placing an order (unit price 100.00, quantity 1: total
  100.00) and finding it by its id" in place of "placing an order and finding it". `--prompt lean`, n=10.
- **Prediction:** at least 9 of 10. **Falsified** at 7 or fewer: then a remaining defect is not a missing value.
- **Generality:** `none`; order only.
- **Result B10 (order, `lean`, n=10):** hidden **10/10**; all 10 runs completed both steps, 20 of 20 steps first attempt, no retry.
  Prediction (at least 9): held. So on order too, the L1 result is reached without a checklist: L2 prose, every case a step is told
  to test stated as call arguments and the expected result, in every step.
- **Summary B7 to B10 (order, hidden pass of 10):** L2 7; + reworded sentence 4; + calls in step 1 6; + calls in both steps **10**;
  boundary sentence + reworded 9; L1 checklist 10. Failure mechanisms along the way, each one a case with no usable value: the
  threshold test with two totals (L2), an outcome read as a unit price (B7), and step 2's untested-by-the-plan case reusing step 1's
  numbers (B9).
- **The rule, now on two stories (order, shipping; one model; plans by hand):** for every step, every case its test sentence names
  is stated as the call's arguments and the expected result, and the sentence does not contradict the rule. With that, L2 prose
  reached L1's result on both (shipping B5 10/10, order B10 10/10), so numbered items, MUST wording and an API line were not what
  the checklist needed. Still untested: loans (scale, where values alone already gave 10/10, C13) and stock with its API
  failure (C17 needed the API line; accessor names are not cases). The values here are ones I took from the feature and the hidden
  test's boundaries; whether a Planner finds them is course step 3. A deterministic check is the natural first Planner gate:
  every case named in a test sentence has a stated call and result.
- **Status:** done.

## 2026-10-18: Counter-evidence to the B-series rule, from T2 (no run)

- **Observation:** the tickets L2 plan (T2, 10/10, 30 of 30 steps first attempt) names its cases without any values: "tests validating
  inside the window, validating too late, validating twice, cancelling, validating a cancelled ticket, cancelling a validated
  ticket", "tests issuing, validating, cancelling ... an unknown ticket for both validate and cancel". By the rule written after B10
  ("every case a test sentence names has a stated call and expected result") T2 should have failed. It did not.
- **What differs:** in tickets the expected result of each named case is already stated in the rule sentence as a status or an exact
  message, and the case's inputs are free (any time inside or after the window works); nothing has to be computed from the inputs. In
  shipping and order the expected value is a number computed from the inputs (a step count, a discounted total), and the inputs are
  not free (the boundary matters). The failures of B1 to B9 were all a computed expected value that the model got wrong (floor
  vs ceiling, 450.00 vs 500.00, 4500.00, 4.00 vs 0.00) or a case whose input the model had to guess.
- **Refined rule (a hypothesis from five stories, not tested):** a case needs a stated call and result when its expected result is a
  value computed from its inputs (arithmetic, thresholds, steps), or when its input is constrained by a boundary; a case whose result is a
  status or message the rule already states does not. Loans (scale) and stock (API shape) fit neither side cleanly: loans' dates were
  computed values and values-only gave 10/10; stock's cost was accessor names, fixed by an API line.
- **To test it, and what would falsify it:** take tickets L2 and make one case computed and boundary-bound without a value (for
  example a different window length and a 'more than N minutes' rule with no example time); predict a failure class of the B-series
  kind in at least 4 of 10 runs; falsified if 2 or fewer fail. Not run.
- **Generality:** `multi-story`, the observation (five stories, one model); the explanation is untested.

### T4. Tickets L2 with a boundary-bound case the rule leaves open (tests the refined rule; prediction first)
- **Plan:** `tasks/tickets/plan-signatures-open.yaml` = T2 (L2, 10/10) with one change to step 1: the expiry rule "when at is more than 90
  minutes after issuedAt" becomes "when the 90 minutes of validity counted from issuedAt have run out at the time of validation".
  Whether validating at exactly issuedAt plus 90 minutes is valid is now not decided by the wording (the hidden test says valid);
  the test sentence names no values, as in T2. This is the shipping gap (under 1000 g) in a different story and domain.
- **Prediction:** at least 4 of 10 runs fail the hidden test on the boundary (a ticket validated at exactly 90 minutes rejected);
  T2 is 10/10 with the unambiguous wording. **Falsified** (the hypothesis that boundary-bound cases need a stated value) if 2 or
  fewer fail. Also read: do the model's own tests assert the boundary either way?
- **Generality:** `none`; one story, one model; one factor (the wording of one rule).
- **Result T4 (tickets, `lean`, n=10, the expiry rule worded so that the boundary is open):** hidden **9/10**; all 10 runs completed all 3
  steps, 29 of 29 steps passed at the first attempt (one run's failed-hidden run is the only failure). Prediction (at least 4 of 10
  fail on the boundary): **falsified**, and the falsifier (2 or fewer) holds: 1 failed. The failing run (07) wrote
  `isBefore(issuedAt.plusMinutes(90))` for validity, rejecting exactly 90 minutes (`Ticket has expired` at 11:30); the other nine
  wrote an expiry test of the form `at.isAfter(issuedAt.plusMinutes(90))` or an equivalent that keeps the boundary valid.
- **Reading:** the hypothesis from the T2 counter-evidence (a boundary-bound case without a stated value fails) is **falsified** by this
  one change: leaving the boundary open cost 1 run in 10, not 4 or more, although the rule sentence no longer decides it. What
  differs from shipping's gap is the model's default: for "expired when 90 minutes have run out" the common idiom
  (`isAfter(start.plusMinutes(90))`) happens to give the intended boundary, while for "each started 500 grams" the common idiom
  (truncating division) gives the wrong one, and for a threshold test it invents a second expected value. So whether a plan must state
  a case's value depends on whether the model's default answer matches the intent, which is not visible from the plan text
  alone. The B-series rule (state call arguments and expected result for every named case) is a sufficient fix on order and
  shipping, not shown necessary in general, and its scope cannot be read off the kind of rule (computed or boundary-bound).
- **What this means for a Planner:** a rule of the form "state every case" is safe but over-includes; the information that decides whether
  a value is needed is the Implementer's measured failure on that case, not a property of the plan text. That supports the course's
  own order (the Planner is scored by whether the Implementer passes), and argues against a purely syntactic plan audit as the
  Planner's ground truth (compare the audit's miss on stock and on rule coverage).
- **Generality:** `one-story` (tickets, one rule), one model; one wording of an open boundary; the idiom explanation is read from 10 samples'
  code and not separately tested (a test would be a rule where the default idiom is the wrong one, for example tickets with a
  rule that wants exclusive ends).
- **Status:** done.

### X1/X2. The idiom test: tickets with the opposite boundary (prediction first)
- **Question:** T4 passed 9/10 with an open boundary. Is that because the model reads the wording, or because its default idiom
  (`isAfter(issuedAt.plusMinutes(90))`, inclusive) happens to match the hidden test? Hold the plan text and change the intent.
- **Task:** `tasks/tickets-exclusive/` (hidden test copy with the boundary reversed: validating at 10:30 and 11:29 is valid, at 11:30
  and 11:31 is `Ticket has expired`; checked against a reference implementation, 6 of 6; the feature file and the rest are as
  `tickets`; same baseline repo). The hidden test is the only thing that differs from T4.
- **X1 `tasks/tickets-exclusive/plan-open.yaml`:** byte for byte the T4 plan (the 90 minutes "have run out at the time of validation").
  **X2 `plan-stated.yaml`:** the same plus ", so a validation at exactly 90 minutes after issuedAt is expired too". `--prompt lean`, n=10.
- **Prediction:** the idiom explanation says the model's output does not depend on the open wording, so X1 fails the hidden test (the
  11:30 case) in at least 7 of 10 runs; X2 (the value stated) passes in at least 8 of 10. **Falsified** (the idiom explanation) if X1
  fails in 3 or fewer: the model then reads the wording, and T4's 9/10 was comprehension. If X2 passes in 5 or fewer, a stated boundary
  does not override the idiom either, which would be a stronger finding.
- **Generality:** `none`; one story, one rule, one model.
- **Result X1/X2 (`tickets-exclusive`, `lean`, n=10 each; hidden test: exactly 90 minutes is expired):**

  | Plan | Hidden | Completed 3 steps | First-attempt steps | Code idiom |
  |---|---|---|---|---|
  | X1 open wording (= T4's text) | **0/10** | 9 | 27 of 28 | `isAfter(issuedAt.plusMinutes(90))` in 9 of 10 |
  | X2 the boundary stated in the rule sentence | **0/10** | 10 | 30 of 30 | `isAfter(issuedAt.plusMinutes(90))` in 10 of 10 |
  | (T4, inclusive hidden test, same wording as X1) | 9/10 | 10 | 29 of 29 | the same idiom |

  Prediction X1 (fails in at least 7 of 10): held (all 9 completed runs fail with `expected IllegalStateException, nothing thrown` at
  11:30). The falsifier (3 or fewer) did not occur. So the same plan text gives 9/10 or 0/10 depending on which boundary the hidden test wants:
  T4's 9/10 was the idiom, not comprehension of the wording. Prediction X2 (at least 8 of 10 pass): **falsified, and the stronger branch
  holds**: with the rule sentence saying "so a validation at exactly 90 minutes after issuedAt is expired too" all 10 runs still wrote the
  inclusive idiom, all 10 passed their own tests and the gate (none of the model's tests checks exactly 90 minutes), and all 10 failed the
  hidden test. A stated boundary in the rule prose did not override the default.
- **Reading:** (1) the idiom explanation holds for this rule: the output follows the model's default for "expires after 90 minutes", and the
  prose, open or explicit, does not move it. (2) The gate cannot help: the model writes its own tests from the same idiom and never probes the
  exact boundary, so the code and its tests agree on the wrong answer. (3) In the B-series the fix was a stated value **in the cases the
  step is told to test** (shipping 1001 and 1501, order 125.00 x 4); a statement in the rule sentence (X2, and shipping's B3) is not
  that. The distinction is between a rule and a case. X3 would test it: X2 plus a test-sentence case "validating at exactly 90 minutes
  after issuedAt (11:30 for 10:00) is expired".
- **Consequence for the Planner:** a plan that states a rule correctly can still be implemented wrongly, undetected by the step's own tests. The
  plan has to turn each boundary into a case the step must test, and the case has to carry the expected result; the plan text alone does not tell
  which rules need this (T4 passed without it), so the Implementer's measured failure on the case has to decide, as the course says.
- **Generality:** `one-story`, one rule, one model; the two intents are mirror images of one wording, which is the strongest design here and
  the narrowest evidence.
- **Status:** done. X3 not run (prediction: at least 8 of 10; falsified at 5 or fewer, which would mean even a case in the test sentence does not
  move the idiom).

### X3. The boundary as a case in the test sentence (prediction first)
- **Plan:** `tasks/tickets-exclusive/plan-stated-case.yaml` = X2 (the boundary stated in the rule sentence, 0/10) plus, in step 1's test
  sentence, the case "validating at exactly 90 minutes after issuedAt (11:30 for a ticket issued at 10:00), which is expired". The rule
  sentence is unchanged from X2. Same task, hidden test and `--prompt lean`, n=10.
- **Prediction:** at least 8 of 10 pass the hidden test: the case makes the model write a test at the boundary, the test fails against the
  inclusive idiom, and the gate and retry move the code. **Falsified** at 5 or fewer: then even a case in the test sentence does not move
  the idiom, and the retry (or the first attempt) ignores its own failing test.
- **Generality:** `none`; one story, one rule, one model; one factor against X2.
- **Result X3 (`tickets-exclusive`, `lean`, n=10, the boundary as a case in the test sentence):** hidden **0/10**, and no run got past step 1:
  0 of 10 completed, 0 steps passed (10 of 10 step starts failed after 3 attempts). Prediction (at least 8 of 10): **falsified**, and the
  falsifier (5 or fewer) holds by the widest margin. What changed against X2: the model now writes the boundary test in all 10 runs
  (`testValidateAtExactly90Minutes`, `assertThrows(IllegalStateException.class, () -> ticket.validate(issuedAt.plusMinutes(90)))`),
  and writes `if (at.isAfter(issuedAt.plusMinutes(90)))` in the same answer, so its own test fails (`TicketTest.testValidateAtExactly90Minutes:22
  Expected java.lang.IllegalStateException to be thrown, but nothing was thrown`). The gate catches it, as intended. The repair does not:
  in the 3 runs whose three calls I compared (02, 03, 05) the retry answers are the same length to the token (1135/1135, 1338/1338,
  1253/1257 completion tokens) and the `isAfter` line is unchanged in all three attempts of run 02. The retry prompt shows the previous
  answer, the test name and line, the exception line and the message above; it does not show the test's source line.
- **Reading:** the model writes a test that contradicts its own code in one answer, so the idiom is not a belief the case could change: the
  test is derived from the case, the code from the idiom, and the two are generated without reconciling. The feedback names the failing
  test method but the model does not trace it to the comparison operator; "retries repeat the failed answer" (G1) holds here in the
  strongest form. This is a recovery problem on the harness side, not a plan-detail problem: the plan now carries the case, the gate
  catches the disagreement, and the repair rung (restart, retry with feedback) does not resolve it.
- **Summary X1 to X3 (exclusive hidden boundary, hidden pass of 10):** open wording 0, boundary in the rule sentence 0, boundary as a case in the test
  sentence 0 (gate failure). Inclusive hidden boundary (T4, same wording as X1): 9. What each rung did: rule prose, no effect; a case in the
  test sentence, the failure moves from the hidden test to the gate; the retry, no effect.
- **A candidate for the next rung (a proposal, not implemented; it changes harness code, which needs your go-ahead):** put the failing test's source line in the
  retry feedback, as a stated fact computed from the surefire output (`TicketTest.java:22: assertThrows(IllegalStateException.class, () ->
  ticket.validate(issuedAt.plusMinutes(90)))`), so the model can see the call that did not throw next to its own comparison. Prediction:
  X3 passes at least 6 of 10 with the source line in the feedback; falsified at 2 or fewer. A cheaper variant that needs no code: run
  X3 with `--context-extra` carrying that sentence as a fact; both are single-factor changes.
- **Generality:** `one-story`, one rule, one model; 10 of 10 is a strong result for this rule only.
- **Status:** done.

### X4. X3 with a stated diagnostic fact in the project context (`--context-extra`; prediction first)
- **Why this variant:** the proposed harness change puts the failing test's source line in the retry feedback; that line differs per run, so
  `--context-extra` (a static line in the Project section of every step and attempt) cannot carry it. The closest static fact is about how
  to read the failure, not about this rule: `When a test fails with "Expected X to be thrown, but nothing was thrown", the call in that
  test did not reach the code that throws: check each comparison on the path of that call, including whether it includes or excludes the
  value the test uses.` It does not name the operator or this story. Whether it carries the effect of the dynamic line is a separate,
  later question.
- **Run:** X3's plan (`plan-stated-case.yaml`), task `tickets-exclusive`, `--prompt lean`, n=10, with `--context-extra` set to that sentence.
- **Prediction:** at most 3 of 10 pass the hidden test (X3 0/10): a generic sentence in the Project section is a hint, the model already
  sees the failing test and an identical retry, and a sentence about reading failures is further from the comparison than the failure
  itself. **Falsified** at 6 or more: a stated diagnostic fact moves the repair, and the dynamic source line is worth building.
  Also measured: first-attempt gate failures (expected 10 of 10 as X3), retries identical to the failed answer, and any run whose retry changes the comparison.
- **Generality:** `none`; one story, one rule, one model.
- **Result X4 (`tickets-exclusive`, `lean`, n=10, X3 plus the diagnostic fact in the project context):** hidden **0/10**; 0 runs completed, 10 of
  10 step starts failed at step 1 after 3 attempts, all with `testValidateAtExactly90Minutes ... nothing was thrown`. Prediction (at most 3 of 10):
  held; the falsifier (6 or more) did not occur. The comparison line in the submitted `Ticket.java` is `if (at.isAfter(issuedAt.plusMinutes(90)))`
  in all 30 attempts (10 runs by 3). The only visible effect of the fact: 5 of 20 retries were byte-identical to the failed answer, against 10 of
  20 in X3, so the sentence perturbs the retry and does not aim it.

### Repair behaviour, measured over this session's batches (`docs/repair_behaviour.py`; 219 failed attempts that had a retry; this session's batches only, `lean`, one model)
- **Recovery is rare:** 19 of 219 retries (9%) passed the gate; 0 of 96 retries that were an identical answer did.
- **By what the retry changed (retries / recovered):** nothing (identical answer) 96 / 0; production code only 7 / 3; test only 25 / 7; both
  91 / 9. A retry that changes only one side recovers more often (10 of 32, 31%) than one that rewrites both (9 of 91, 10%), although the
  one-side group is small and selected (the model touches one side when the failure points at it).
- **By failure class (retries / identical / recovered):** a wrong expected value in the model's own test 179 / 82 / 13; an expected
  exception that was not thrown 26 / 13 / 1; other 10 / 1 / 5; compile 4 / 0 / 0. Retries that are not identical recover 19 of 123 (15%).
- **X3 and X4 specifically (the exact-boundary tickets rule):** 40 retries, 0 recovered; in all 60 attempts the comparison line and the
  exact-90 test case are unchanged. The retry changed other parts of the files (formatting, other tests) but never the one
  line the failure points at.
- **What the retry prompt contains** (read in X3 run 02): the task, the previous answer in full (so the failing test's source line is already in the
  prompt, next to the production code), the test name and line number, the exception message, "your previous answer was undone", and
  the instruction to fix the errors. So the proposed source-line addition would repeat text already present; X4's static fact moved nothing.
  The model has the test, the code and the error and does not connect them.
- **Reading:** repair does not fail for lack of information in the prompt; the model does not reason from the failing assertion to the
  comparison operator, and 44% of retries change nothing at all. More text of the same kind (a line, a generic sentence) is the wrong
  lever. Candidates that act differently, in the escalation order: (a) verify and recover deterministically: for `expected exception not
  thrown`, compute the boundary the test exercises (the call's arguments are in the test source) and report the nearest production
  conditional that the call passed through, which needs coverage data (a larger harness change); (b) change who decides: tell the retry that the
  test is the specification, so only production code may change (a stated fact about roles; it changes which file the model edits, which the
  `production code only` row suggests is the productive one, 3 of 7, and X3 shows the model otherwise edits neither); (c) sample diversity,
  measured already (restart equals feedback, R3).
- **Not recommended:** the source-line feedback (information already present, 0 of 40 on the closest static analogue). If a harness change is
  chosen, (b) first, as a measured prompt-side fact with `--context-extra`: prediction X3 passes 3 of 10 or more; falsified at 1 or fewer.
- **Generality:** the repair table is `multi-story`, one model, mostly stock/shipping/order/tickets batches of this session, not a fixed sample;
  X3/X4 are `one-story`. The 'recovered' count treats a retry whose gate passed as recovered, whatever the later steps did.
- **Status:** done. No harness change made.

### X5. X3 with "the test is the specification" as a stated fact (`--context-extra`; prediction first)
- **Fact:** `The tests are the specification: when a test fails, change the production code to satisfy it, not the test.` Static line in the
  project context; it names no operator, rule or story. It changes who is allowed to be wrong, which X3 and X4 never did (the model edited
  other parts of its files and left the comparison alone; the `production code only` retries recovered 3 of 7).
- **Run:** X3's plan (`plan-stated-case.yaml`), task `tickets-exclusive`, `--prompt lean`, n=10.
- **Prediction:** at least 3 of 10 pass the hidden test (X3 and X4 0/10). **Falsified** at 1 or fewer. Also read: does the comparison line
  change in any retry, and do any runs instead delete or weaken the exact-90 test (the failure mode a role statement could create)?
- **Generality:** `none`; one story, one rule, one model. A prompt-side hint with a possible side effect on tests; not a harness change.
- **Result X5 (`tickets-exclusive`, `lean`, n=10, X3 plus "the tests are the specification"):** hidden **0/10**; 0 runs completed, 10 of 10 step starts
  failed at step 1 after 3 attempts, every one `testValidateAtExactly90Minutes ... nothing was thrown`. Prediction (at least 3 of 10): **falsified**
  (falsifier 1 or fewer). The comparison line is `if (at.isAfter(issuedAt.plusMinutes(90)))` in all 30 attempts; 8 of 20 retries were identical
  answers, 2 changed production code only (0 recovered), 10 changed both. No run weakened the failing test into passing (the same boundary test
  fails in all 10; a first look counted tests that write the boundary as 11:30 instead of `plusMinutes(90)` as "dropped", which was wrong).
- **Summary X1 to X5 and what they say (exclusive boundary, hidden pass 0 of 10 each, except T4's inclusive mirror 9/10):** a rule stated in the
  prose (X2), the case in the test sentence (X3), a diagnostic fact (X4) and a role fact (X5) all left the comparison unchanged in every
  attempt, 0 of 60 attempts across X3 to X5. Prompt-side levers are exhausted for this rule on this model: the plan can carry the case
  (X3) and the gate can catch the disagreement, and the model cannot resolve it.
- **Why the model writes a test that contradicts its own code:** the harness asks for the production code and its test in one answer. The
  test is derived from the case, the code from the idiom, and nothing makes the code answer to the test; the retry sees the failure but
  starts again from its previous answer, which contains both. This is a property of the one-answer protocol, not of the wording.
- **A candidate that changes the protocol (a proposal, not implemented; it needs your go-ahead):** split the step into two calls: first the test
  file from the case, then the production code with the test file as read-only context and the instruction to make it pass, with the
  retry editing the production file only (the harness enforces it by restoring the test file, the "make the wrong answer unrepresentable"
  rung). The legacy red/green TDD loop did this for TypeScript; the isolated Implementer does not. Prediction: X3 passes at least 5 of
  10 (0/10 now); falsified at 2 or fewer, which would mean the idiom survives even a test it can see in a separate call. A cost to measure
  at the same time: the number of calls and tokens per step (about 2x), and whether stories that pass today (order L1, stock, tickets T3)
  still pass 10/10.
- **Generality:** `one-story`, one rule, one model; but 60 attempts across three variants and an identical outcome.
- **Status:** done. No harness change made.

## 2026-10-18: the two-call protocol behind a flag (`--protocol test-first`; harness change, default unchanged)

- **Where:** `Protocol::{OneAnswer, TestFirst}` and `Implementer::with_protocol` in `src/implementer.rs`; `run_step --protocol one-answer|test-first`.
  `OneAnswer` is the default and is byte-for-byte the earlier behaviour (all earlier tests pass unchanged: 302 in the lib, workspace green).
- **What `TestFirst` does, for a tool-mode step whose scope creates both a `src/test/` file and a production file (any other step runs as before):**
  1. Call 1 (`navigate-tests`): the step's own task plus "In this call write only the test file(s) ... The production code is written
     afterwards, in a separate call, against your tests."; scope create = the test files only; no build.
  2. The harness keeps the test files for the rest of the step and writes them to disk itself at the start of every attempt (the snapshot
     removes them when an attempt fails, and they are written again).
  3. Call 2 (`navigate`, retries `navigate-retry-N`): the task plus "The test file(s) this task asks for already exist and are fixed ... Write
     only the production code ... Do not submit a test file."; scope create = the production files; the test files are shown under "Files for
     reference (read-only)"; gates run with `StepKind::Refactor` (the test-coverage gate would otherwise refuse a production-only submission; the
     tests were gated in call 1). A submission that contains the test file is refused by the scope gate.
  4. A retry after a failed build redoes call 2 only, with the previous production answer and the failure; call 1 is repeated only while it has
     produced nothing (a process failure before any tests exist), and then it is shown that failure.
  Four unit tests pin this (order of calls and what each prompt lists, the retry redoing only the production call with the tests rewritten from
  what the model wrote, the refusal of a test file in call 2, the first call repeated when it never produced anything).
- **Known costs and risks (to read in X6):** about twice the calls per step on a first attempt; a wrong test cannot be repaired any more (the
  order L2 class where the model's own test is wrong would now fail instead of sometimes recovering); the task text still contains "Add
  TicketTest ..." in call 2, followed by the sentence that the tests exist (a contradiction in the prompt that I could not remove without
  parsing the instruction; watch for call 2 submissions that include tests); a test that does not compile cannot be corrected by call 2.
- **Smoke test (1 run, `tickets-exclusive`, `plan-stated-case.yaml`, `lean`):** the mechanism behaves as designed (call 1 wrote the test file,
  with `testValidateAtExactly90Minutes` asserting an exception at `plusMinutes(90)`; calls 2 to 6 wrote production code only; the harness rewrote the
  test file before each retry, `restored 3 file(s)` then `wrote src/test/...`). The run failed all 3 attempts on the same boundary: the model
  saw the exact-90 test as read-only context in the production call and still wrote `at.isAfter(issuedAt.plusMinutes(90))` three times. One
  run is not a measurement, but it is the outcome the falsifier describes.

### X6. The two-call protocol on X3 (prediction first; not run yet)
- **Run:** X3's plan, task `tickets-exclusive`, `--prompt lean`, `--protocol test-first`, n=10.
- **Prediction (written before the smoke test was read, unchanged):** at least 5 of 10 pass (X3 0/10). **Falsified** at 2 or fewer. After the
  smoke test I expect the falsifier (the idiom survives a test the model can see in a separate call); this entry stays as written so the
  result can be read against it.
- **Also to measure with it:** calls and tokens per step (about 2x), and the regression check on stories that pass today (order L1,
  tickets T3, stock L1+API, shipping B5), each n=10, `test-first` against their one-answer numbers; and order L2 (7/10), where the model's own
  test is sometimes wrong, to see the cost of a test that cannot be repaired.
- **Status:** queued.
- **Result X6 (`tickets-exclusive`, `lean`, `--protocol test-first`, X3's plan, n=10):** hidden **0/10**; 0 runs completed, 10 of 10 step starts failed
  at step 1 after 3 attempts. Prediction (at least 5 of 10): **falsified**, and the falsifier (2 or fewer) holds. 7 runs fail only at
  `testValidateAtExactly90Minutes ... nothing was thrown` (the comparison line is `at.isAfter(issuedAt.plusMinutes(90))` in 27 of 30
  production answers); 12 of 20 retries are identical to the failed production answer, 0 recovered.
- **What the protocol did change:**
  - The idiom moved once in ten runs: run 08 wrote `issuedAt.plusMinutes(90).isBefore(at) || issuedAt.plusMinutes(90).equals(at)` in attempts 1
    to 3 (the intended boundary) and then still failed, see next point. So with a fixed, visible test, 1 run in 10 reached the right comparison; one-answer
    X3, X4 and X5 reached it in 0 of 30 runs.
  - The cost of fixed tests showed in 3 runs (01, 04, 08): the model's own test is wrong in a second place (`testOriginalTicketUnchanged` calls `cancel()`
    on a validated ticket and expects no exception; `testCancel` compares a cancelled ticket with an ISSUED one) and the production call cannot repair
    a wrong test. Run 08 had the right boundary and failed on this alone. In the one-answer protocol the retry could have edited that test.
- **Reading:** a test the model can see, in a separate call, does not make it reconcile its comparison with the test (27 of 30 answers); the model's
  idiom for `expired after N minutes` is strong against an explicit assertion in its own context. The protocol is the wrong lever for this rule, and it
  trades away test repair. It stays behind the flag, default off.
- **Not run, by the condition set in the entry above:** the regression batches (order L1, tickets T3, stock, shipping B5, order L2); they only matter if the
  protocol helps. The order L2 prediction stands as a hypothesis: it would lose runs where the model's test is wrong (3 of 10 here show that class).
- **What is left for this rule (the exact boundary on this model):** (a) accept it as a model limit and say so in the Planner's scoring: such a case is
  not a plan-detail failure, it is not repaired by retries, a role fact, a diagnostic fact, a case in the test sentence or a two-call protocol, so a plan
  cannot buy it back; (b) the one lever that acts on the model's output without asking it to reason: sample several production answers against the fixed
  test and keep the first that passes (best of N, verified by the build, so no trust in the model). Run 08 suggests the right comparison is reachable
  (about 1 run in 10); a prediction for it: with N=8 production samples per step and the test fixed, X3 passes at least 4 of 10, falsified at 1 or fewer. It
  costs N calls per failing step. (c) A different model, which this setup cannot test here.
- **Generality:** `one-story`, one rule, one model; 100 attempts across X3 to X6.
- **Status:** done. The flag remains, off by default.

## 2026-10-18: P1-P3, the `current` system prompt on loans, shipping and stock (the deferred arms of G1)

- **Why:** G1's prediction 1 (the lean system prompt is not worse than `current`) was compared only on order (L2: 9/10 lean, 8/10 current). The
  arms for loans, shipping and stock were cut for time. The fresh `lean` L2 numbers are the reference: loans 0/10, shipping 3/10 (5/10 earlier),
  stock 10/10 (n=10 each; same plans `plan-signatures.yaml`, same binary, same model).
- **Run:** `--prompt current` on each story's L2 plan, n=10 each.
- **Prediction (before the runs):** on each story `current` is within 2 runs of `lean` (loans 0 to 2, shipping 1 to 5, stock 8 to 10): the extra
  system rules do not change the result, as on order and on every earlier comparison. **Falsified** if `current` beats `lean` by 3 or more
  on any story (then the lean prompt loses something there), or if `current` is 3 or more below `lean` (then `current` hurts).
- **Generality:** `none` yet; the first comparison on these three stories; one model.
- **Result P1-P3 (`--prompt current`, L2 plans, n=10 each; reference `lean` from this session's fresh runs):**

  | Story | `current` hidden | `current` steps first attempt / later / failed | `lean` hidden (steps first attempt) |
  |---|---|---|---|
  | loans | 0/10 | 0 / 0 / 10 | 0/10 |
  | shipping | 1/10 | 11 / 0 / 9 | 3/10 (earlier 5/10) |
  | stock | 10/10 | 25 / 5 / 0 | 10/10 (30 of 30) |

  Prediction (each within 2 of `lean`: loans 0 to 2, shipping 1 to 5, stock 8 to 10): held on all three. The falsifiers (`current` 3 or more
  above or below `lean`) did not occur against the fresh reference; against shipping's earlier 5/10 `current` is 4 lower, which is the spread
  that cell has between batches (5 then 3 for `lean`). The one difference in kind is stock: `current` needed a second or third attempt in 5 of
  30 steps where `lean` passed 30 of 30 at the first, with the same final result.
- **Reading:** with this one more comparison on three stories the system prompt (`current` against `lean`) does not change the hidden-test
  result on any story where it has been compared (order, loans, shipping, stock; tickets and the rest were run `lean` only). G1's prediction 1
  stands for all four, with the note that `current` costs attempts on stock. Generality: `multi-story` for "the extra system rules do not matter
  at the hidden-test level", one model, one stack.
- **Status:** done.

## 2026-10-18: S0, the baseline of the Planner scored by the Implementer (course step 3 starts here)

- **What:** `canopy-planner/score_planner.py` runs `run_planner` for N plans and then `run_step` once on each plan, reading the hidden-test verdict.
  The Planner is the current master one (draft call, retry, the verifier; no design pass), `--prompt lean` for the Implementer. `run_planner --api`
  adds the story's fixed API (`tasks/<story>/api.txt`: names and signatures, no behaviour) to the story text, because a plan chooses its own class names
  and the hidden tests are written against a fixed API; without it a plan could be right and still score zero. The API is therefore an input the
  Planner is given, as the spec step would give it, not something it derives. Pilot (stock, 2 plans, 2026-10-18): both valid (one on the first draft), 8
  and 10 steps; the Implementer stopped at step 3 and step 2 (0 of 2); planning took 25 to 50 s per plan.
- **Stories and reference (hidden pass of 10, hand-written plans, `lean`):** order L2 7 to 9, L1 10; loans L2 0, L1 10; shipping L2 3 to 5, L1 10; stock L2 10;
  tickets L2 10 (T2), one-line plans (L3) 0 to 1 on all stories.
- **Prediction (before the batch):** at least 8 of 10 plans valid on every story; hidden pass at most 2 of 10 on every story, i.e. the Planner as built
  scores at the L3 line and not at the hand-written L2 line, as order did in the earlier Planner batches (plan validity 10/10, Implementer 0 to 1 of 10).
  **Falsified** if the Planner reaches 5 or more on any story (then the plan shape is not what the hand-written plans add), or if fewer than 6 of 10 plans
  are valid on any story (then the baseline cannot be read).
- **Generality:** `none` yet; five stories, one model, one Planner version, n=10 each; the API input is a human decision here.
