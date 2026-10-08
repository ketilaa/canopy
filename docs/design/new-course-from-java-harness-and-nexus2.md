# A New Course: Absorbing java-harness and nexus2

Status: principle adopted 2026-10-08 (see the escalation order in CLAUDE.md); the plan below is partly
superseded. Written after reading both predecessor projects' CLAUDE.md, roadmap/backlog, dogfooding
knowledge base and experiment records.

**What changed after this was written.** The same day, the Part 1 redesign (a hand-written golden output
and an ablation of its fields) was dropped as too much at once, and work moved to the Implementer on its
own: an isolated crate, `canopy-implementer`, run against an empty Spring Boot / Maven repository, and
brought into `canopy-cli` only once it has proven itself. That crate realises the Phase 0 and Phase 2
ideas below for a single step: a typed event stream with external observers, schema-constrained output,
the harness (not the model) running `mvn`, exact restore after a failed attempt, and measured runs against
a hidden acceptance test. Part 1 and the Anthropic deprecation are still open. Findings so far are in
`canopy-implementer/docs/models/qwen3-14b-q4_k_m.md`.

## Why a new course

Canopy's current escalation order is *tool, then prompt, then gated code*, and it forbids Rust-side
handling of model output beyond audits. That order assumes a model that follows a correct, well-placed
instruction. Three independent bodies of evidence say the small local model does not, reliably:

- **Canopy itself:** three escalating prompt fixes for the repository-layer test ceiling failed
  identically (2026-07-13) and ended in a pipeline redesign, not a better prompt.
- **java-harness:** one paragraph break moved P(model keeps editing after "compiles") from 0.458 to
  0.058. Temperature 0 did not make runs reproducible (a near-tie, 0.551 vs 0.449, decided the fork).
  The roadmap's own verdict: "the problem is the method, not the mechanisms." Prompt wording tuned to
  one model's decision surface does not transfer.
- **nexus2:** the proposer/challenger pair is stable (92%) partly because the model is consistently
  wrong in the same direction (personal data classified `PSEUDONYMOUS` in 10/10 runs; the label was
  `PERSONAL`). Stable is not correct, and a second pass by the same model shares its bias.

So the working assumption changes: **the model is an unreliable component inside a deterministic
harness.** Prompts teach; they do not enforce. Enforcement belongs to the output type, the filesystem,
the compiler and the test runner.

## The revised escalation order

Replaces "tool, then prompt, then gated code" in CLAUDE.md, once agreed.

0. **Make the wrong answer unrepresentable.** Constrain output with a JSON schema (llama-server
   `response_format`; enum-typed fields for closed sets). The harness renders any downstream text
   from the fields. java-harness's Initializr lost its whole line-rule chain this way; nexus2 gives
   each facet its own schema so the enum is exactly that facet's options.
1. **Compute or look it up.** A fact that is mechanically computable is injected (always needed) or
   offered as a tool (model decides when to ask). Unchanged from today's tier 1 (`find_symbol`,
   `observed_call`, `available_packages`).
2. **Verify against ground truth, then recover deterministically.** Compile, run the test, re-check
   against the filesystem, compare to the independently-known fact. On failure: snapshot-restore the
   step, retry once with specific feedback naming what is wrong, then revert and record. Never rely
   on the model's own report ("no change needed", `##CANOPY_DEVIATIONS##`).
3. **Teach in the prompt, and only with a measured delta.** Short, positioned next to the subject.
   Any prompt change that is not backed by a paired comparison is a guess, not a fix.

What stays forbidden: Rust code that silently *rewrites the meaning* of model output (path injectors,
semantic post-processors). What is newly allowed and encouraged: rejecting, restoring, retrying with
precise feedback, and rendering text from validated fields. The existing "audit vs compensation" test
still decides: does the code change what the model produced, or only accept/reject it?

Honest caveat from the evidence: a schema fixes *shape*, not *truth*. java-harness kept its grounding
layer even after schemas, because a well-formed answer can still be invented.

## What to adopt, ranked by leverage

### From java-harness (mechanisms and method)

| # | Take | Evidence | Canopy today |
|---|---|---|---|
| 1 | **Measure before building**: typed run-event trace, automatic scorer with hidden acceptance checks, cross-run table with intervals, replay client | Roadmap: 40 improvements tuned on one request/one model; batches of 4 cannot separate 2/4 from 0/4 | Only `llm-debug.log`; no scorer, no replay |
| 2 | **Schema-constrained output** for every structured stage | Initializr, Explorer, Planner, Implementer all moved to schemas | 33 call sites, free-text YAML, `repair.rs` + `yaml_util.rs` (34 uses) |
| 3 | **Step snapshot / restore / revert**, per-edit compile check with checkpoint | run-12: a broken first edit was inherited by four steps and five repair passes | None in `canopy-cli` |
| 4 | **Ground every claim against tool evidence or the filesystem** | Fabricated existence claims reproduced live | Partial (audits, Roots) |
| 5 | **Step scope** and **same-file step merging**, "compiles" is a stop signal as its own paragraph | batch-6/7: out-of-step edits 3/4 runs; 0 after the fix | Not present |
| 6 | **Test-adequacy checks**: requested-test count by name, test-preserving edits, mutation check for vacuous tests | A scored pass can be a no-op; vacuous `assertEquals(id, id)` passes | Red-phase sanity check only |
| 7 | **Language registry seam** for stack-specific facts | One entry (Java/JUnit); core stays language-neutral | Layer detection branches per language inline |
| 8 | **Knowledge base discipline**: `models/<model>.md` starts with the exact server command; claims cite run ids; confirmed vs suspected | Results are only comparable under the same server command | Retrospectives/principles exist; no per-model files |
| 9 | **Server-health knowledge**: `tools` plus `response_format` must never be sent together; tool-grammar runaway decode on `peg-native`; a stale-cancel slot race; `repeat_penalty` override | Measured on llama.cpp b9870 (model-degeneration investigation) | Not recorded anywhere in canopy |

Two java-harness ideas to treat carefully: *final answer as a tool call* is attractive (removes the
schema/tools split) but was explicitly "to verify first"; the *invalid run status* (environment
misbehaved is not the agent failing) is cheap and should come early.

### From nexus2 (technique)

| # | Take | Evidence |
|---|---|---|
| 1 | **Typed LLM boundary**: `LlmRequest<T>` carries its response type and schema; parse failure is a first-class event | Cleanly separates raw client from structured generator |
| 2 | **Schema drift test** (schema vs type must agree) or better, **generate schemas from the types** | nexus2's own backlog lists hand-maintained schema copies as debt; in Rust, `schemars` on the serde types should remove the second copy (to verify) |
| 3 | **Disagreement as an ambiguity detector**: blind second opinion disagreed in 22% of decisions, concentrated on judgement calls (4/10) and near zero on clear ones (0/10); the judge-style challenger objected in 5% | Directly relevant to canopy's finding that the review gate gives no signal between categories |
| 4 | **Positive-control inputs**: stability without cases that *need* the non-default answer can just be the cheapest answer every time | Checklist experiment, 120 runs |
| 5 | **Split factual from design questions**; "cheapest sufficient option" is wrong for classification | `PSEUDONYMOUS` vs `PERSONAL` |
| 6 | **Idempotent consumers** (at-least-once delivery) and an architecture-rules test that enforces dependency direction | Worth an ArchUnit-equivalent for the crate graph (`canopy-cli` must not link roots-storage) |
| 7 | Idea, untested: **rationale before verdict** in the schema so the verdict follows the reasoning | Reviewer approved a distributed system for 10-200 users while its own rationale said a monolith |

Not adopted: the event-sourced runtime, event bus, projections, web UI, consensus policy. Canopy's
artifacts are human-edited YAML under `.canopy/`; an event store would fight that. Keep the *ideas*
(typed boundary, disagreement signal, escalation to a human), drop the machinery.

## Phased plan

Each phase has an exit criterion that is a measurement, not an impression.

**Phase 0 - measure (no prompt changes allowed during it).**
Typed run events written to `.canopy/runs/<id>/events.jsonl`; token/time per call; an `invalid` status
for environment failures; a replay `LlmClient` that serves recorded responses; a small benchmark of
stories with hidden acceptance checks and a scorer; a `docs/knowledge/models/<model>.md` with the exact
`llama-server` command. *Exit:* "is commit B better than A on this model" is one script with an interval.

**Phase 1 - unrepresentable.**
A provider-neutral `StructuredRequest<T>` in `canopy-llm`; schema derived from the serde type. Migrate
stages in order of measured parse/repair failures from Phase 0, not by guess; retire the matching
`repair.rs` helpers as each stage moves. Never send `tools` and a schema in the same turn. Only the
OpenAI-compatible local provider needs the schema path (see "Deprecating the Anthropic provider").
*Exit:* repair-helper hit rate near zero on migrated stages; fewer retries per stage.

**Phase 2 - recover deterministically in `implement`.**
Per-step snapshot with restore; compile after each edit with a checkpoint; step scope; revert with a
recorded `NOT DONE` that later steps see. Stack specifics behind a registry seam, starting by
consolidating the existing TS/Java split. *Exit:* no step leaves the tree non-compiling; scored pass
rate same or higher with fewer fix-loop iterations.

**Phase 3 - disagreement as signal in `spec`/`behaviors`.**
Add a blind second sample on judgement-call ADRs and behavior decisions; disagreement raises the
human gate's priority, agreement does not skip it. Build positive-control stories first. Tested
against canopy's own human-insight inventory. *Exit:* disagreement predicts human modification of
the proposal better than chance on the existing review history.

## Deprecating the Anthropic provider

Decision (2026-10-08): hosted Anthropic API usage has a significant cost, so canopy goes local-only
(llama-server or any OpenAI-compatible endpoint). Consequences for this plan:

- **Simpler Phase 1.** One structured-output path (`response_format` json_schema) instead of two.
  No need to design forced-tool-use equivalents for a provider we no longer run.
- **The measurement discipline matters more, not less.** A local model is the only model, so every
  prompt and harness change is tuned to one decision surface (java-harness's problem 1 and 5).
  Phase 0's paired runs and the per-model knowledge file are the guard; record the server command
  with every benchmark result.
- **No fallback to a stronger model.** The "stronger judge" escape hatch is gone; nexus2's finding
  that proposer and challenger share the model's bias applies in full. Disagreement signals (Phase 3)
  must come from independent samples and the human gate, not from a bigger model.

Touch points (small): `canopy-llm/src/client.rs` (`call_anthropic`, `MissingApiKey`, the
`LlmProvider::Anthropic` arms), `LlmProvider` in `canopy-core/src/lib.rs`, one mention in
`canopy-llm/src/tools.rs`, and the "LLM Providers" section of CLAUDE.md. Stored `config.yaml` files
that say `provider: anthropic` must fail with a clear message, not a serde error.

Suggested sequence: (1) mark deprecated now, with a warning at startup and in CLAUDE.md; (2) remove
once Phase 0's trace and bench exist, so the removal itself shows no regression. Removing `Anthropic`
from the enum is a breaking config change, so do it deliberately rather than as a drive-by.

## Retiring the two projects

Order matters; nothing is deleted.

1. Port knowledge, not code: create `docs/knowledge/` in canopy with the harness findings
   (`shared.md` distilled, the two model files, the model-degeneration investigation, the roadmap) and
   the nexus2 experiment record, each keeping its evidence citations and a provenance line. Distil,
   do not copy 89 KB of CLAUDE.md; most of it is Java-mechanism detail that belongs in this doc's
   adoption table.
2. Record what was deliberately *not* carried over (event store, runtime, web UI, Gradle skill, the
   disconnected `GradleSkill`, the 104-row improvements backlog except the rows the roadmap kept as
   hypotheses: #92, #62, #73, #77).
3. Tag each repository's final commit, add a README stating "superseded by canopy, see
   docs/knowledge/", and archive on the host. The archive step is outward-facing, so it is yours to
   trigger.
4. Keep java-harness's dogfooding target and its tags as the first benchmark seed if you still want a
   Java stack; otherwise leave it with the archive.

## Open decisions

- Is the principle change above what you intend, including allowing deterministic restore/retry?
- Phase 0 before Phase 1, or schema first because it is the biggest immediate gain? (Recommendation:
  Phase 0 first, kept small; its own roadmap says the same.)
- May the predecessor repos be named in canopy's committed docs for provenance? CLAUDE.md forbids
  naming personal dogfooding projects; these are predecessors, but it is your call.
- Which model and server command is the baseline for Phase 0?
- Anthropic: warn-and-deprecate first, or remove the provider outright in the first change?
