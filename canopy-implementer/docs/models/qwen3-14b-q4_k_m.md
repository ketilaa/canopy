# Qwen3-14B, Q4_K_M (llama-server)

Results are only comparable under the same server command. Change the command here first, keep the
old one as a history line naming the runs that used it.

## Server command

Model id served: `Qwen/Qwen3-14B-GGUF:Q4_K_M` (as reported by `/v1/models`). Cached at
`~/.cache/huggingface/hub/models--Qwen--Qwen3-14B-GGUF` (8.4 GB). llama-server `0.4.1-dev`
(commit a60f9ae), Apple Silicon, 32 GB RAM.

```
llama-server \
  -hf Qwen/Qwen3-14B-GGUF:Q4_K_M --offline \
  -c 8192 -np 1 -ctk q8_0 -ctv q8_0 -fa on \
  --jinja -n 2048 \
  --temp 0.6 --top-p 0.95 --top-k 20 --min-p 0 \
  --port 8080
```

- Sampling is Qwen's own: temperature, top-k and top-p from the model's `generation_config.json`
  (0.6 / 20 / 0.95); `min-p 0` from memory of the model card. Do not use greedy decoding.
- The harness also sends sampling explicitly on every request, so results never depend on how the
  server was launched.
- No `--repeat-penalty`: java-harness found 1.15 corrupted identifier spellings.
- Thinking is disabled per request with `chat_template_kwargs: {"enable_thinking": false}`, not by a
  server flag, so the same server can be A/B-tested with thinking later.

## Observed

- **confirmed** (smoke test 2026-10-08): `response_format` json_schema is enforced with thinking
  disabled. A one-class-plus-test answer came back as valid `{files:[{path,content}]}`, no
  `reasoning_content`, `finish_reason: stop`; 74 prompt + 185 completion tokens in 15.6 s
  (11.8 tokens/s generation, 80 tokens/s prompt eval).
- **confirmed** (batch `2026-10-08T14-01-21`, harness `095d93a`, repo tag `baseline-00-scaffold`, 10 runs of the
  Greeter step: one class, one test, a blank-name rule): 10/10 passed on the first attempt and passed the
  hidden acceptance test; 0 retries, 0 failures at any stage. One model call per run (272 prompt tokens,
  282 or 337 completion tokens), 24-30 s of model time at 10-11 tokens/s, builds 1.8-2.3 s each, step total
  28-34 s. Prompt cache: 271 of 272 prompt tokens cached after the first run.
- **confirmed** (same batch): at temperature 0.6 the outputs were nearly deterministic: only 2 distinct
  patches in 10 runs (7 and 3), differing by one extra `null`-name test. For a task this small a single run
  is almost as informative as ten, and the sweep says nothing about the retry path. The task is too easy to
  discriminate: the Greeter step does not exercise the recovery logic.
- **confirmed** (batch `2026-10-08T14-14-55`, task `trim` from tag `candidate-01-greeter`, 10 runs; the model is shown
  the current `Greeter.java` and `GreeterTest.java` in the prompt and returns both files complete): 10/10 scored
  (hidden acceptance passed, both original tests kept, one test added), all on the first attempt, and all 10 patches
  byte-identical: a one-line change (`name.trim()`) plus one new test. 590 prompt tokens, 398 completion tokens,
  34 s model time, 38 s per step. No tools were needed: whole-file output plus the current content in the prompt was
  enough for a change to an existing file of this size.
- **confirmed** (2026-10-08, while serving the batches above): the server process has 9.4 GB resident (`ps` RSS), but
  macOS `footprint` and Activity Monitor report only 964 MB, because the memory-mapped weights (8.4 GB file) are
  file-backed and not counted as dirty memory. About 700 MB of that footprint is the KV cache, allocated up front
  for `-c 8192` with q8_0 (about 87 KB per token), so memory does not grow with prompt length inside the window.
- **suspected**: the speed and the clean runs reflect small prompts (272 to 590 tokens of 8192) and no thinking, not
  an easy model. Generation speed (10-11 tokens/s) is set by output length. Quality at 4-8k tokens of context is
  untested; java-harness saw failures near the context limit.
- **confirmed** (batch `2026-10-08T17-58-29`, harness `fb77dfb` plus the runner change, task `nav` from tag
  `candidate-01-greeter`, 10 runs, tools on): the model is given only "The Greeter should trim the name before
  returning the greeting in the greet method." with no file list and no hints, plus glob, read_file and
  submit_files. 10/10 scored (hidden acceptance passed, both original tests kept, a test added), all on the first
  attempt. Every run navigated the same way: `glob src/**/*.java`, `read_file` on the production class,
  `read_file` on the test class, `submit_files`; 4 tool calls, 0 wasted, 0 refused, 0 errors. Each run: 4 model
  calls, 2993 prompt and about 471 completion tokens, 44 s of model time (35 s of it the submission turn, which
  carries both files), 48 s per step. 3 distinct patches (the added test body varies); all are `name.trim()`
  plus one new test.
- **confirmed** (same batch): the harness gates never fired in a real run. The model read the existing test file
  without being forced, because the system prompt says to keep existing tests and to include a test for every
  behaviour. So read-before-mutate and the test-for-every-change gate are verified by scripted tests, not yet by
  the live model. To see them work live the prompt rules would have to be weakened on purpose.
- **confirmed** (four pressure tasks, 5 runs each, harness `45608f4`, tools on, thinking off; batches
  `2026-10-08T18-43-46` naming, `T19-01-12` vat, `T19-14-40` pasted, `T19-19-14` hi; single-run samples earlier the
  same day agreed). Scored passes: **pasted 5/5, hi 4/5, naming 1/5, vat 0/5.** The two Greeter-only tasks are easy
  (4 to 5 tool calls, about 50 s). The two repos with indirection or a ripple are where it breaks, and the failures
  fall into a few distinct kinds, counted per run:
  - *Budget exhaustion by dithering* (vat 3 of 5 runs, all 3 attempts each; naming 1 of 5): the model reads every
    file once, then re-reads some, and spends the 12 tool calls without writing. It also calls `submit_files`
    with `{"files": []}` (up to 4 times in one attempt). The retry text for a budget failure said "read the files
    again before you change them", which is the opposite of what such a model needs.
  - *Change in the wrong place, repeated* (naming 3 of 5): the trim goes into `Greeter` (`formatter.display(name.trim())`,
    which also breaks the null-name test) while its own new test targets `NameFormatter.display`, which it never
    changes. The failing test and values were reported exactly, three attempts in a row, and the answer was
    repeated (2 distinct answers in 3 attempts).
  - *Gate message that backfired* (hi 1 of 5): told "You changed Greeter but submitted no changed test", the model
    dropped the Greeter change and submitted only the test, 3 attempts running (1 distinct answer). A test-only
    submission is allowed by the gates, so it was built and failed on the unchanged class.
  - *Own tests weaker than the request* (vat 2 of 5): the loop passed (its own tests passed) but the hidden
    acceptance failed: the total still excluded VAT (expected 5000, was 4000); it changed the invoice but not
    the calculator that the total and the charge come from.
- **confirmed** (rerun after the three message fixes and the subject hint, harness `15bf566`, clean tree; batches
  `2026-10-08T19-42-23` naming, `T19-57-40` vat, `T20-06-38` hi; same tasks, 5 runs each). Scored passes before to
  after: **hi 4/5 to 5/5, naming 1/5 to 0/5, vat 0/5 to 0/5.** What each change did:
  - *No-test message now names the file to keep* (hi): the one failing run had been exactly this backfire (the
    old message shown 3 times, the same test-only answer 3 times). hi is now 5/5, but **the rewritten message was
    not shown to the model in any of the 5 rerun runs** (the gate never fired), so this is not evidence that the new
    wording works; it only shows the failure did not recur in 5 runs where its trigger did not occur either.
  - *Subject hint* (naming): it fired and was acted on, and that produced a new failure. In 3 of 5 runs the loop
    "passed" on attempt 2, and **all 5 naming runs ended with an empty patch**: told the test exercises a class
    it did not change ("either change it so the test passes, or correct the test"), the model reverted its
    `Greeter` change, deleted its own trim test, and resubmitted files identical to disk. A submission that
    changes nothing is accepted by every gate and the build passes, so the step counts as done. In run 4 the hint
    was present on all 3 attempts and ignored. Whether the "or correct the test" wording invited the revert is
    **suspected**, not shown.
  - *Budget retry text and empty-submission text* (vat): no improvement. Attempt 1 is identical in all 5 runs
    (glob, 8 reads, 4 re-reads, budget). Attempts 2 and 3 are now degenerate: the first action is
    `submit_files({"files": []})` before reading anything, repeated 12 times despite refusals, 36 tool calls and
    24 refusals per run, identical in all 5 runs. Before the fix those attempts re-read files instead; neither
    shape writes anything.
  - *Observation*: the model avoids producing the long submission: after reading everything it emits cheap calls
    (re-reads, empty submits). Both vat and naming fail to a no-op or a stall, not to a wrong answer.
- **confirmed** (hand-written plans run by the mini orchestrator, harness `7ccae36`, 5 runs each; batches
  `2026-10-09T07-08-05` naming-planned and `T07-15-00` vat-planned; the vat-planned batch records `dirty=true`
  only because untracked plan files for a different task existed in the crate while it ran; they are not
  compiled into the runner and cannot have changed behaviour). Scored passes: **naming-planned 5/5**
  (against 1/5 and 0/5 as a single step; 2 steps, both first attempt, about 80 s, 7 tool calls, every run
  identical) and **vat-planned 0/5** (4 of 20 possible steps passed: runs 1 and 5 passed steps 1 and 2 and failed
  step 3; runs 2 to 4 failed step 1). Decomposition fixed the task whose difficulty was *where* to put a change;
  it did not fix the task whose difficulty is *numbers and unseen APIs*. The vat-planned failures:
  - *Wrong expected values in the model's own tests* (3 of 5 runs: steps 3 in runs 1 and 5, step 1 in run 2).
    The production code was correct (5000 and 22500, matching a reference solution) but the tests expected
    4400 and 20700, or 0 for a VAT of 1000. The feedback said `expected <4400> but was <5000>` and the model
    resubmitted the same wrong numbers three times. It cannot do the arithmetic, and it did not use the actual
    value it was given.
  - *Invented API and paths* (2 of 5 runs, step 1): a `PriceCalculator.java` written under a `demo` package that
    does not exist in this repository, then a call to `order.getItems()` on a record whose accessor is `items()`.
    The gate requires that a file being changed was read; nothing requires that the types a new line depends on
    were read.
  - *Observation*: an expected value in a test is only as good as whoever computed it; when the model computes
    it, the test can pass or fail for reasons unrelated to the requirement. Concrete examples belong in the
    plan step (or the Gherkin), supplied by whoever wrote the spec.
- **confirmed** (same batches): the read-before-mutate gate fired live: on a retry in a fresh conversation the model
  skipped reading and tried to submit, and was refused with the right reason. The no-test gate also fired (hi). The
  empty-submission refusal is the gate that fires most.
- **confirmed** (probe 2026-10-08, before the loop was built): tool calls are well formed on this build with
  thinking off, `tool_choice` left at the default, and the final answer as a `submit_files` tool call carrying
  complete files. No HTTP error from truncated arguments has been seen yet (cap is `-n 2048`).
- **unknown**: behavior of schema-constrained output with thinking enabled.
- **unknown**: runaway decode on tool-call turns (java-harness saw this on another model/build);
  not relevant until tools are used.
