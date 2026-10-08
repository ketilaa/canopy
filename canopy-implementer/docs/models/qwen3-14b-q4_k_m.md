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
- **confirmed** (probe 2026-10-08, before the loop was built): tool calls are well formed on this build with
  thinking off, `tool_choice` left at the default, and the final answer as a `submit_files` tool call carrying
  complete files. No HTTP error from truncated arguments has been seen yet (cap is `-n 2048`).
- **unknown**: behavior of schema-constrained output with thinking enabled.
- **unknown**: runaway decode on tool-call turns (java-harness saw this on another model/build);
  not relevant until tools are used.
