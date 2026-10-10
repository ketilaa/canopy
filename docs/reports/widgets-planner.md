# Story Report — `widgets` (Planner, plan-only sweeps)

A running record of the Planner's plan-only batches against the widgets story (a story, a Gherkin
feature, the `java-widgets@baseline-00-widgets` repository). Design and build order:
`docs/design/planner-first-increment.md`. Model: Qwen3-14B Q4_K_M, thinking off, 4096 completion
tokens, on the GPU-accelerated server. Every batch is n=10 plans, one draft call plus at most one retry,
no Implementer run. A plan counts as **valid** when it parses, passes the five verifier rules and has
every scenario covered. Batch records (harness commit, server properties, sampling) are in each batch's
`batch.json`; the run directories are git-ignored.

Caveat for everything below: one story, one stack, one model, n=10 per batch. The numbers say what
this model does on this story, not what planning in general looks like.

---

## 2026-10-10 — first four batches

First live run (n=1, before the batches): all eight steps came back `refactor`, which exempts a step from
the test rule, so production code with no test passed the verifier. The model no longer chooses the kind
(see `05ba2e6`). The Implementer then failed that plan at step 1 (0 of 8): the instruction said only "a
model class with an id and a name" and the model reached for `jakarta.persistence`.

| Batch | Harness | Change under test | Valid | First draft valid | What the rejections were |
|---|---|---|---|---|---|
| 1 | `05ba2e6` | `kind` removed from the schema | 0/10 | 0/10 | 6 runs: production steps with their tests in later steps (missing test). 4 runs: one file in both `create` and `modify`. |
| 2 | `94a0905` | prompt rule "every step with production code carries a test"; overlap error names the lists | 0/10 | 0/10 | 8 runs still missing tests (the rule was in the request, confirmed in the event log); 2 overlap. |
| 3 | `27c1166` | `tests` a required, non-empty field of every step | 0/10 | 0/10 | 10 runs: a test file listed in both `create` and `tests`. No missing tests. |
| 4 | `f6c741d` | path patterns in the schema: `create`/`modify` under `src/main`, `tests` under `src/test` | 1/10 | 0/10 | 9 runs: a file in both `create` and `modify` (see below). |

### What the batches showed

- **A plain instruction did not hold (batch 2).** The rule was in the prompt, the retry named the failing
  steps, and the model still planned production and test steps apart in 8 of 10 drafts. A schema field that
  cannot be empty removed the failure class completely (batch 3: 0 missing tests).
- **A field the model can misuse, it does (batch 3).** With `tests` added next to `create`, every draft
  listed the test file in both. Grammar-enforced path patterns removed that too (batch 4); the llama-server
  accepted `pattern` in the schema.
- **The remaining failure is state, not shape (batch 4).** The model writes later steps as "update `X`"
  and lists `X` in `create` and in `modify` together, and gives every test the action `create` even when an
  earlier step already created it. It does not track which file an earlier step made. Retrying with the
  named lists did not change that (9 of 9 identical on the second attempt).
- **The instructions are still thin.** The one valid plan (batch 4, run 10) has eight steps with no method
  signatures, no concrete test values and no wiring: the three costs the hand-written plan carries. Whether
  such a plan runs is the end-to-end question, not yet asked at n=10.
- **Cost.** A draft takes 20 to 40 s; a failed draft plus retry 40 to 80 s.

### Open: who decides create or modify

Whether a path is `create` or `modify` is mechanically known: it is `modify` if the repository has it or an
earlier step creates it, `create` otherwise. Having the model state it is where 9 of 10 batch-4 drafts
fail. Computing it would move a fact out of the model (see `docs/principles/compute-facts-mechanically.md`)
but it is Rust code that classifies model output, which the project's rules gate on a proposal and
approval. Decision pending; see the session notes.
