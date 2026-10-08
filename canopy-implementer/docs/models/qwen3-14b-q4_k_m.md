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
- **unknown**: behavior of schema-constrained output with thinking enabled.
- **unknown**: runaway decode on tool-call turns (java-harness saw this on another model/build);
  not relevant until tools are used.
