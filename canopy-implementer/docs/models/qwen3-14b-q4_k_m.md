# Qwen3-14B, Q4_K_M (llama-server)

Results are only comparable under the same server command. Change the command here first, keep the
old one as a history line naming the runs that used it.

## Server command

Model id served: `Qwen/Qwen3-14B-GGUF:Q4_K_M` (as reported by `/v1/models`). Cached at
`~/.cache/huggingface/hub/models--Qwen--Qwen3-14B-GGUF` (8.4 GB). llama-server `0.4.1-dev`
(commit a60f9ae), Apple Silicon, 32 GB RAM.

```
/Users/ketil/code/ketilaa/llama.cpp-build/build/bin/llama-server \
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
- **unknown**: behavior of schema-constrained output with thinking enabled.
- **unknown**: runaway decode on tool-call turns (java-harness saw this on another model/build);
  not relevant until tools are used.
