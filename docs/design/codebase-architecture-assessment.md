# Codebase Architecture Assessment

Status: observation only — no refactoring performed, no redesign proposed. Answers "what
technical debt do we actually have," so future cleanup can be evidence-driven rather than
intuition-driven — the same "observe first, understand first, then improve later" discipline the
Contract Readiness Assessment applied to the product architecture, applied here to the codebase
itself.

Date: 2026-07-15

Reviewed: every crate's `Cargo.toml` and `src/` tree, `git log` history across the whole
repository (excluding stale `.claude/worktrees/` copies), and targeted reads of the highest-churn
and largest files in `canopy-llm` and `canopy-cli`.

---

## Executive Summary

The dependency structure is clean and intentional: two independent trees (Canopy and Roots) joined
at exactly one documented point, with zero leaks found beyond it. Responsibility boundaries are
mostly well kept — no prompt-construction in `canopy-cli`, no CLI/interactive concerns in
`canopy-llm`, no business logic in `canopy-storage` — with one real, narrow exception in
`canopy-cli/src/commands/behaviors.rs`. The duplication found is real but small in scope: a
mechanical string operation (fence-stripping) implemented five different ways, a formatting helper
(ADR-summary rendering) copy-pasted eight times, a review-loop control-flow shape reimplemented
three times, and a "mechanical baseline + bounded LLM review" pattern implemented twice. None of
this is large-scale disorder — it reads as normal accretion from six weeks of fast, evidence-driven
iteration, not architectural drift.

The clearest risk in the codebase isn't duplication — it's concentration. `canopy-llm/src/
prompts/step.rs` and `canopy-cli/src/commands/implement/execute.rs`/`fix_loop.rs` are both
high-churn (27, 24, and 20 commits respectively) and high-complexity (a 384-line, ~30-branch,
explicitly order-sensitive function in `step.rs`; a ~495-line function in `execute.rs` that owns
generation, TDD control flow, and reporting together; a 281-line function in `fix_loop.rs` mixing
five distinct concerns). `canopy-cli` overall has only 19 unit tests for ~5,441 lines, and the two
files just named are validated mainly through live dogfooding, not automated tests — the worst
combination (high churn, high complexity, low test coverage) sits in exactly the part of the
codebase most likely to need to change next, given the project's current priorities (composition,
pre-behavior planning). Nothing here is broken today; the finding is about where change is riskiest
if it comes, not about a defect that exists now.

## Current Architecture

| Crate | Responsibility | Depends on | Consumed by | Lines | Tests |
|---|---|---|---|---|---|
| `canopy-core` | Data types shared across the whole pipeline (structs/enums, serde) | — | `canopy-storage`, `canopy-llm`, `canopy-cli` | 1,232 | 15 |
| `canopy-storage` | Save/load wrappers around `.canopy/` | `canopy-core` | `canopy-llm` (dev-dep, examples only), `canopy-cli` | 370 | 7 |
| `canopy-llm` | LLM client, prompt-building, tech-stack skills | `canopy-core`, `canopy-storage` (dev-dep) | `canopy-cli` | 9,303 | 75 |
| `canopy-cli` | CLI commands (clap), interactive prompts (dialoguer), orchestration | `canopy-core`, `canopy-llm`, `canopy-storage`, `roots-parser` (one narrow exception) | end user | 5,441 | 19 |
| `roots-core` | Graph types (Workspace/Project/Module/File/Symbol) | — | `roots-parser`, `roots-storage`, `roots-context`, `roots-cli` | 357 | 13 |
| `roots-parser` | Language parsers (Java, Kotlin, TypeScript, Rust) populating the graph | `roots-core` | `roots-cli`, `canopy-cli` (one function) | 1,657 | 14 unit + 28 integration |
| `roots-storage` | SQLite-backed graph persistence | `roots-core` | `roots-context`, `roots-cli` | 629 | 0 unit + 23 integration |
| `roots-context` | Context-packet assembly, impact analysis | `roots-storage` | `roots-cli` | 376 | 10 |
| `roots-cli` | `roots` CLI (index, query, discover, impact) | `roots-core`, `roots-parser`, `roots-storage`, `roots-context` | end user, `canopy-cli` (as an external process) | 962 | 7 |

`canopy-explore` appears in `git log` history (52 commits) but no longer exists — it was renamed
to `canopy-llm` at commit `6bed3f6`. Its history is real `canopy-llm` history, split by the rename;
worth knowing when reading raw commit counts for either name.

## Dependency Structure

```
canopy-core  (foundation, zero internal deps)
    ^
    |-- canopy-storage
    |       ^
    |       |-- canopy-llm  (dev-dep only, for examples)
    |
    |-- canopy-llm
    |-- canopy-cli  <-- canopy-storage, canopy-llm
                    <-- roots-parser  (one narrow, documented exception)

roots-core  (foundation, zero internal deps)
    ^
    |-- roots-parser
    |-- roots-storage
    |       ^
    |       |-- roots-context
    |
    |-- roots-cli  <-- roots-parser, roots-storage, roots-context
```

Two structurally independent trees, joined at exactly one point: `canopy-cli` depends on
`roots-parser` directly for `find_test_call_shape` (parsing in-memory test content before it
exists on disk, so there's no live Roots index to query yet — CLAUDE.md's own documented
exception). Confirmed clean beyond that: `canopy-cli`'s `Cargo.toml` has no `roots-storage`/
`roots-context` entries at all, and every other Roots interaction goes through
`std::process::Command::new("roots")`, not an in-process call. `canopy-llm` has zero imports of
any `roots-*` crate. `canopy-core` has zero internal dependencies at all — a genuine foundation
crate, not just a nominal one.

## Responsibility Boundaries

Checked directly, not assumed:

- **No prompt-construction in `canopy-cli`.** Every LLM interaction across the CLI's command
  files routes through a `canopy_llm::` function (12 call sites); zero direct
  `client.complete*()` calls exist in `canopy-cli/src/`.
- **No CLI/interactive concerns in `canopy-llm`.** No `dialoguer` or `clap` dependency in
  `canopy-llm/Cargo.toml`; no interactive `Input`/`Select`/`Confirm` usage anywhere in its source.
  `eprintln!` usage exists but is either gated behind an explicit debug flag
  (`canopy-llm/src/client.rs`) or is a fallback/malformed-data warning, not user-facing
  interaction.
- **No business logic in `canopy-storage`.** Its 370 lines are error types, path helpers, and
  typed `save_X`/`load_X` wrappers. The only non-trivial logic is a directory-scan-plus-sort
  (`list_adrs`) and a string-normalization helper (`intent_slug`) — both pure utility, not
  decision-making.
- **One real boundary crossing**: `canopy-cli/src/commands/behaviors.rs` directly mutates
  `Decision` struct fields (`d.status`, `d.resolution`) and iterates `BehaviorList`/
  `ClusteringResult`/`ContractSet` internals by field access, rather than delegating to a
  `canopy-llm`/`canopy-core` accessor. This is presentation-plus-human-gating logic, not business
  logic leaking in — but it is direct domain-type manipulation inside the CLI layer, the one place
  this project's stated crate boundaries aren't fully held. `execute.rs`/`plan.rs` are clean of any
  `Contract`/`Behavior` reference.
- **Mixed-concern functions exist, described neutrally.** `execute_steps` (`execute.rs`) and
  `load_or_generate_plan` (`plan.rs`) each combine deciding *what* to do, calling the LLM, running
  a human gate, and persisting the result, all within one function body — not a violation of crate
  boundaries (everything here legitimately belongs to `canopy-cli`'s orchestration role), just a
  concentration of several distinct steps in one place. See "Architectural Hot Spots" below.

## Duplication Analysis

Five real, meaningful instances found; artifact load/save was checked and found clean.

1. **Fence-stripping logic exists in five forms.** One canonical helper
   (`yaml_util::strip_code_fence`, used correctly 6 times), one independent near-duplicate
   (`repair::strip_code_fences`, used twice), and three manual inline copies of the same
   trim-chain (`spec.rs` twice, `plan.rs` once) — in a file (`spec.rs`) that already imports and
   correctly uses the canonical helper elsewhere.
2. **ADR-summary bullet-list rendering is copy-pasted eight times** across four files (`spec.rs`
   five times, `behaviors.rs`, `dependencies.rs`, `plan.rs`), with minor formatting variance
   between copies (one variant drops the "None yet." fallback) and no shared helper anywhere.
3. **The Accept/Modify/Reject review-loop control flow is independently written three times**
   (`intent.rs`, `spec.rs`, `behaviors.rs`) — all three share the same "iterate → `select_required`
   → branch on the chosen index → mutate and persist" skeleton, using the one shared UI primitive
   (`select_required`) but never factoring the surrounding loop itself into a shared function.
4. **The "mechanical baseline + one bounded LLM review pass" workflow is implemented twice**
   (`contracts.rs`'s `review_dependencies`, `clustering.rs`'s `review_clustering`) with an
   identical control-flow skeleton (guard on empty input → LLM call → strip fence → parse →
   apply/build findings) and no shared abstraction between them.
5. **No meaningful duplication in artifact load/save.** Each artifact (`services.yaml`,
   `stories.yaml`, `domain_registry.yaml`, `roles.yaml`) has exactly one save-owning call site per
   distinct lifecycle event; where two commands both save the same file (e.g. `domain_registry.yaml`
   from both `init` and `intent`), it's two genuinely different events, not two implementations of
   the same operation.

## Architectural Hot Spots

Ranked by the combination of size, churn, and structural complexity — not size alone:

- **`canopy-llm/src/prompts/step.rs`** (957 lines, 27 commits — the highest churn of any prompt
  file): one domain (implementation-step/test-stub prompting) exploded across many special cases
  (TS vs. Java, component vs. non-component, worked-example vs. hand-written skeleton).
  `unit_test_stub_prompt_ts` alone is 384 lines with roughly 30 branches, explicitly commented as
  order-sensitive by its own author.
- **`canopy-cli/src/commands/implement/execute.rs`** (613 lines, 24 commits): `execute_steps` is
  a single ~495-line, `pub(crate)`, one-call-site function that interleaves plan/constraint
  loading, stale-artifact cleanup, Red-phase generation, a sanity check, Green-phase generation,
  plan persistence, and a final cross-service regression pass.
- **`canopy-cli/src/fix_loop.rs`** (508 lines, 20 commits): `run_fix_loop_inner` (281 lines) mixes
  build-output parsing, a `javax`→`jakarta` auto-migration, LLM-driven repair dispatch,
  noop/stagnation detection, and telemetry-string building in one function.
- **`canopy-llm/src/prompts/spec.rs`** (1,204 lines, the single largest file in the project): five
  genuinely separate concerns bundled in one file — architectural-question prompting,
  entity-schema/policy prompting, scenario generation, orchestration, and OpenAPI generation.
  Largest function is `entity_schema_prompt` at 211 lines.
- **`canopy-llm/src/skills/tech_stack.rs`** (786 lines, 18 commits): large because of *data
  volume* (per-stack prose skill text), not algorithmic complexity — a structurally different
  kind of "large" than the four above, and correspondingly lower-risk to touch.
- **`canopy-core/src/lib.rs`** (984 lines): wide, not deep — roughly 55 unrelated struct/enum
  definitions spanning every pipeline stage, flattened into one module with no submodules. Fan-in
  confirms centrality (`Adr` used in 14 files, `ServicesRegistry` in 12, `UserStory` in 11) — any
  change here has a wide blast radius by construction, independent of whether the file itself is
  ever split.
- **`canopy-llm/src/lib.rs`/`canopy-cli/src/main.rs`**: both have very high raw commit counts (79
  and 66 respectively — `canopy-llm/src/lib.rs`'s inherited `canopy-explore` history adds another
  52 on top), but both are currently tiny (31 and 21 lines) — pure re-export/entry-point files that
  get one line added per new public item across the project's whole life. High churn here is a
  mechanical signal, not a complexity one; worth distinguishing explicitly from the hot spots
  above, which are high-churn *and* high-complexity.

## Evolution Pressure Analysis

File-touch counts across commits in the last 7 days, by top-level directory: `canopy-llm/` 178,
`canopy-cli/` 121, `docs/` 119, `canopy-core/` 15, `canopy-storage/` 7, and every `roots-*` crate
combined: 8. Development pressure right now is overwhelmingly concentrated in `canopy-llm` and
`canopy-cli` — exactly where the contract-driven implementation investigation (Stages 1–6),
the pre-behavior planning work, and this very assessment are happening. The Roots side of the
repository is essentially stable/mature by comparison.

Within that pressure, the files most likely to keep changing, based on what's driving current
work: `step.rs` (already the highest-churn prompt file, and every new tech-stack/layer special
case tends to land there), `execute.rs`/`fix_loop.rs` (the TDD/fix-loop orchestration that every
contract-driven implementation stage exercises), and `spec.rs` (service discovery/tech
recommendation — the exact subject of the newly-designed reproducibility sweep). These four are
also the four hot spots named above — the same files carrying the most structural concentration
are the ones under the most active pressure, which is the combination worth attention before it
becomes expensive to change, not because anything is currently broken.

## Technical Debt Inventory

### Low-risk cleanup

| Item | Evidence | Impact | Urgency |
|---|---|---|---|
| Fence-stripping duplicated 5 ways | `yaml_util::strip_code_fence` (6 correct uses) vs. `repair::strip_code_fences` (2 uses) vs. 3 manual inline copies in `spec.rs`/`plan.rs` | Low individually; a future edge-case fix would need applying in up to 5 places, risking silent divergence | Low now; rises the moment the fence-stripping logic itself needs to change |
| ADR-summary rendering duplicated 8 times | 4 files, 8 call sites, minor formatting drift already present between copies | Low individually; same "synchronized edit" risk as above | Low now; rises if the rendering rule (escaping, sort order, truncation) ever needs to change |

### Medium-priority structural improvements

| Item | Evidence | Impact | Urgency |
|---|---|---|---|
| Review-loop (Accept/Modify/Reject) implemented 3 times | `intent.rs`, `spec.rs`, `behaviors.rs` — same skeleton, no shared function | Medium — this is core human-gate logic, directly relevant to the Roadmap Reassessment's own Human-Insight Inventory idea | Medium, and specifically *rising* now: instrumenting review outcomes for that inventory is far easier against one shared function than three |
| "Mechanical baseline + bounded review" duplicated twice | `contracts.rs`'s `review_dependencies` vs. `clustering.rs`'s `review_clustering` | Medium — a real, reusable pattern now used twice, not yet extracted | Medium; per this project's own "reproduced across independent problems" bar, worth watching for a third occurrence before extracting, not urgent to extract speculatively |
| `behaviors.rs`'s direct `Decision`/`Contract`/`Behavior` manipulation | Specific field mutations and internal iteration cited above | Medium — the one clear deviation from an otherwise clean crate boundary | Low-medium; not causing a bug today |

### Long-term architectural concerns

| Item | Evidence | Impact | Urgency |
|---|---|---|---|
| `execute_steps`/`run_fix_loop_inner` concentrate multiple distinct concerns in one function each | ~495 and 281 lines respectively, both interleaving generation/control-flow/persistence/reporting | High if these files need to change — and per Evolution Pressure, they're among the most likely to | Currently low (nothing broken); this is the single item most likely to become expensive later if churn continues unaddressed |
| `canopy-core/src/lib.rs`'s breadth (55 types, no submodules) | High fan-in (`Adr` in 14 files, `ServicesRegistry` in 12, `UserStory` in 11) | High blast radius on any change, by construction | Low currently (comparatively low recent churn there) |
| `step.rs`'s ~30-branch, self-described order-sensitive function | 384 lines, highest churn of any prompt file | High — an order-sensitive many-branch function is exactly the shape most likely to accumulate a bug from a future addition landing in the wrong place | Medium — nothing broken today, but under the highest active churn of any prompt file |

## Refactoring Readiness

**Stable enough to refactor confidently**: the `roots-*` crates (near-zero recent churn — 8
file-touches combined across all five in the last week — with strong test coverage: 23 integration
tests for `roots-storage`, 28 for `roots-parser`, 10 for `roots-context`, 13 unit tests for
`roots-core`); `canopy-storage` (tiny, clean, 7 tests, low churn); `canopy-core` in the sense that
it isn't itself under heavy direct-logic churn right now — though its breadth means any change
there needs care regardless of how "stable" the file's own history looks.

**Still evolving too rapidly for major cleanup**: `canopy-llm` as a whole (178 file-touches in the
last 7 days alone — the active epicenter of the Stage 5/6 and pre-behavior-planning work happening
in this very session), and specifically `canopy-cli`'s `execute.rs`/`fix_loop.rs` — the worst
combination in the codebase (high churn, high structural complexity, and the lowest test coverage
of any major crate: 19 unit tests for ~5,441 lines, with these two files validated mainly through
live dogfooding rather than automated tests). Confident structural change needs either a slowdown
in active feature work here, or a test-coverage investment first — neither of which this
assessment is recommending, only naming as the precondition either would need.

## Incremental Improvement Opportunities

Small, additive, reversible — none require touching the higher-risk hot spots (`execute.rs`,
`fix_loop.rs`, `step.rs`, `canopy-core/src/lib.rs`):

1. Replace `repair::strip_code_fences` and the three manual inline copies with calls to the
   already-existing, already-correct `yaml_util::strip_code_fence`. Mechanical, no behavior
   change, and the existing test suite would catch any regression immediately.
2. Extract the 8-times-duplicated ADR-summary bullet-list renderer into one shared helper.
   Same shape as #1.
3. **A naturally-motivated opportunity, not a speculative one**: when the Roadmap Reassessment's
   Human-Insight Inventory work actually starts, that's the moment to factor the three independent
   review-loop implementations into one shared function *while* instrumenting it for the
   inventory's own accept/modify/reject counting — one change serving two purposes that would
   otherwise be done separately.
4. If a third "mechanical baseline + bounded review" workflow is ever needed, that's the trigger
   (per this project's own evidence standard) to extract `contracts.rs`'s/`clustering.rs`'s shared
   skeleton — not before.

None of these are proposed as work to do now; they're named as low-cost, low-risk options to reach
for opportunistically the next time any of the affected files is already being touched for another
reason.
