# Story Report — `order` (plain Java; Planner schema experiments, Implementer plan vs single step)

A running record of the experiments run against the order story on a plain Java 21 Maven repository
(`dogfooding/java-orders@baseline-order-001`: JUnit 5 only, no framework). They started from one question:
a standalone probe showed the 14B model writing a correct class and tests in one shot from a short prompt,
so are our instructions and constraints enabling the model, or getting in its way? Model: Qwen3-14B Q4_K_M
on the GPU-accelerated server, thinking off. Harness: commits `46287ad` and `502d7ff` plus uncommitted
Planner changes described below. The run directories are scratch and not kept in the repository.

The story: a customer places an order for a product in a quantity; the total is price times quantity, less
10 percent when that net total is 500.00 or more; a quantity that is zero or negative is rejected; a placed
order can be looked up by its id. The feature has four scenarios, two of them outlines with boundary cases
(`canopy-implementer/tasks/order/feature.feature`).

Caveat for everything below: one story, one stack, one model, n=10 per arm unless stated. A small story
may not represent where planning pays. The numbers say what this model does on this story.

Scoring. A plan is **valid** when it parses and passes the verifier. An Implementer run is **scored** when
every step passes its gates (compile, tests), the whole suite is green at the end, at least 4 tests were
added and `pom.xml` is untouched. There is no API-independent hidden test: plans choose their own class
names, so acceptance is the whole suite passing.

---

## 2026-10-14..16

### Making the harness stack-agnostic (first)

The Planner could not plan a plain Java repository at all: the root package came only from a class
annotated `@SpringBootApplication`. It now falls back to the pom's own `groupId`, and the four system
prompts say "Java Maven project" (the repository facts already state the Java and Spring versions when they
exist). Both changes are committed (`46287ad`) with tests. They change the baseline for every earlier
Spring batch, so earlier numbers do not compare directly.

### Planner batches (plan only, one draft plus at most one retry)

| Batch | Change under test | Valid | First draft valid | `step_size` failures | Runs with test-only steps |
|---|---|---|---|---|---|
| P0 (n=3, first feature draft) | none, first plain-Java run | 3/3 | 0/3 | 3/3 (8 files in one step) | 2/3 |
| P1 control (n=10, fixed feature) | none | 10/10 | 4/10 | 6/10 | 5/10 |
| P2 (n=10) | `test_classes` removed; `classes` 1 to 2 | 8/10 | 7/10 | 0/10 | 5/10 |
| P3 (n=10) | + no `Test` anywhere in a class name; `instruction` capped at 1,500 characters | 10/10 | **10/10** | 0/10 | 6/10 |

The first feature draft had a scenario that took the discount as an input; it was rewritten (explicit
threshold, outlines with boundaries, a persistence scenario) and the control re-run on it, so P0 is not
comparable with the later batches.

What the batches showed:

- **A schema bound removed a counting failure completely.** Capping `classes` at half the step cap
  (a class and its derived test are two files) took `step_size` from 6/10 to 0/10 and the first-draft
  validity from 4/10 to 7/10 (P2).
- **Removing a field did not remove the behaviour.** The model writes one step per scenario and, with
  `test_classes` gone and at least one class required, put a class in each test step. In P2 those classes
  carried mangled names (`OrderServiceTestNegativeQuantity`, in 6 of 10 runs, against 7 of 10 in the
  control) because the production-class pattern only rejected names *ending* in `Test`.
- **Closing that escape moved it.** With `Test` banned anywhere in a name (P3) the mangled names are gone
  (0/10 runs) and validity is 10/10, but 24 of 56 steps are still test-only steps. Their classes are named
  after the scenario (`PlaceAnOrderForOneProduct`, `QuantityThatIsNotPositiveIsRejected`,
  `ANetTotalOf50000OrMoreGetsA10PercentVolu`: a scenario id cut at the name-length limit), one run used
  `OrderServiceIntegrationtest` (lowercase), and the Implementer would create production files with these
  names.
- **The cap hid loops, it did not stop them.** P2 had 3 runs with a draft cut off at the token limit
  (2 of them invalid after the retry: the `instruction` text looped, "The Product class will be used to…"
  39 times in one draft). With the cap (P3) 7 of 56 instructions sit at 1,500 characters, in 5 of 10 runs,
  some cut mid-sentence. The longest complete instruction anywhere was 1,146 characters.
- **One valid plan was vacuous.** P3 run 5 is two steps (`Product`, `Customer`, `Order`) and nothing that
  implements the behaviour. The verifier checks that every scenario is covered, not that anything is built.
- **Plan size fell.** Steps per plan went from 5 to 12 (mean 9.4) in P1 to 2 to 7 (mean 5.6) in P3.

Plan validity went from 4/10 to 10/10 on the first draft. It is a gameable measure: the model satisfied
each constraint in letter. It was not evidence of better plans, and the Implementer batches below show it.

### Implementer batches (the same plans run step by step against the repository)

| Batch | What was run | Scored | Steps passed |
|---|---|---|---|
| IA (n=3 plans each) | P1 plans vs P3 plans, retry rule as it was | 0/3 vs 1/3 | 5 of 20 vs 7 of 17 |
| IB (n=10) | P3 plans: old context vs scoped context (below), retry rule as it was | 1/10 vs 0/10 | 15 of 56 vs 9 of 56 |
| SA (n=10) | the story as a single step, no plan | 0/10 | n/a |
| SB (n=10) | single step, retry rule changed (below) | **4/10** | n/a |
| IC (n=10) | P3 plans: old vs scoped context, with the retry fix | 0/10 vs 0/10 | 15 of 56 vs 12 of 56 |

**Scoped context** means a step is shown only the scenarios it is the last step to cover (the step that makes
a scenario pass), not every scenario it lists in `covers`. Scenario blocks in step contexts fell from 127 to
40 over the 56 steps of the P3 plans. The model's plan was held fixed (the plans were re-derived from the
stored drafts), so only the context differed. This change is not committed: it showed no effect.

What the batches showed:

- **A prompt rule contradicted the retry, and it was the largest single bug found.** The tool-mode system
  rules said "ALWAYS use glob and read_file to find and read the files you need", and a retry after a
  compile or test failure said the previous answer was undone and "NEVER look for the files you submitted".
  In the single-step runs (no scope, so no file list) the model obeyed the system rule: 17 of 20 retry
  failures were tool-budget or repeated-call loops, and no first-attempt failure recovered. Leaving those
  two reading rules out of a retry that shows the previous files cut the loops to 2 of 12 retry failures,
  2 of 8 first-attempt failures recovered, and the single step went from 0/10 to 4/10 scored (`502d7ff`).
  The first-attempt prompt is unchanged by this edit, yet the first-attempt mix moved from 7 compile errors
  to 1, so part of the 4/10 is sampling noise. The retry effect (loops 17/20 to 2/12) is the clear part.
- **Plan runs never had that loop.** A plan step's scope names the files to create or change, so the model
  has a target. Plan retries ended as a loop in 4 of 24 retry failures before the fix (old-context plans,
  13 runs) and in 3 of 21 and 0 of 21 after it (IC, old and scoped context).
- **Plans fail by compounding.** A plan step passes first time about half the time (13 of 25 and 10 of 22
  in IC) and a failed first attempt recovers about one time in six (2 of 12 in each arm). A plan of 4 to 7
  steps needs every step to pass, and the run stops at the first failure, so almost none complete. The
  failing stages in IC were 9 compile, 8 test, 2 stuck and 1 truncated: ordinary code errors.
- **Single step beat every plan.** 4 of 10 against 0 of 20 across the two IC arms is a gap unlikely to be
  chance (one-sided Fisher p about 0.008), and no plan arm ever scored more than 1 of 10. The single-step
  arm was run in a separate batch from IC, and it had its own first-attempt luck, so the size of the gap is
  uncertain.
- **Scoping the context did nothing.** The Implementer's failures at step 1 or 2 were not the
  scenario-in-context pattern I expected from two hand-read cases. The Planner's own instruction already
  put the discount rule, the threshold and the rounding into the data-class step ("The constructor calculates
  the total… applying a 10% discount if the net total is 500.00 or more"). A keyword check of failing
  against passing steps shows no link between behaviour in the instruction and failure (4 of 10 failing and
  9 of 15 passing in the old arm). The check is rough.
- **The Implementer varies widely run to run.** One P3 plan scored 6 of 6 steps in IA and passed 2 of 6 in
  IC, on the same plan and code. Single runs of a plan say little.

### Predictions that did not hold

| Prediction | Result |
|---|---|
| Dropping `test_classes` removes test-only steps (0/10) | 5/10 runs still had them (P2) |
| Banning `Test` in names removes the fabricated test classes | the model named them after the scenarios instead (P3) |
| The instruction cap stops the loops | it truncated them: 7 of 56 instructions at the cap (P3) |
| Scoped context cuts out-of-scope tests and raises passes | no effect: 0/10 vs 0/10 (IC) |
| With the retry fix both plan arms improve on the earlier 1/10 and 0/10 | 0/10 and 0/10 (IC) |

### Process notes

- A stale example binary ran 20 no-op Implementer runs after a repository rename (each exited at once with
  "not a git repository"). Rebuild the example binary after any rename and read the first run's output
  before reporting a batch as running.
- The model server was down once (started with nohup, not a service); restarting it needs the owner's
  browser SSH check, so a batch cannot always be started unattended.
- Two earlier counts in the working notes ("runs with mangled test class names", 8/10 and 5/10) came from a
  script that over-counted by one; the figures above (7/10 and 6/10) are recomputed from the artifacts.

### Open questions

- Where does planning start to pay? This story is small enough for one step. A larger story (several
  aggregates, cross-cutting rules) run as one step against a plan, n=10 each, would show whether there is
  a threshold. The same experiment says whether the compounding per-step reliability is the planner's
  problem or just the cost of any multi-step run.
- Would a coarse plan (at most three steps) keep the structure without the compounding?
- The first-attempt prompt still tells the model to find files when there is nothing to find (2 of 10
  first attempts in SA, and 1 of 10 in SB, looped on `glob` in the empty repository). The single-step 4/10 needs more runs
  (n=30, same binary) before it is quoted.
- The schema and context-scoping experiments in `canopy-planner/src/draft.rs` and `prompt.rs` are not
  committed. P3's schema raised validity without raising the Implementer pass rate, so whether any of it
  should be kept is undecided.
