# CLAUDE.md

## Project: Canopy

Canopy is an AI software engineering system.

Canopy is NOT a code completion tool.
Canopy is NOT a chat interface over a repository.
Canopy is NOT a big-bang architecture generator.

Canopy is an incremental planning and implementation engine. It enforces discipline:
behavior is specified before code is written, and architecture decisions are made
story by story — never all at once.

---

## Core Design Insight

Everything emerges. Nothing is decided upfront.

| Artifact | Emerges from |
|---|---|
| Vision | `init` |
| User roles | `intent` (from `as_a` fields) |
| Domain entities and events | `intent` (automatic extraction) |
| User stories | `intent` (one behavioral statement at a time) |
| Services and responsibilities | `spec` (ADR proposals) |
| Technology stack per service | `spec` (ADR proposals) |
| Databases and event infrastructure | `spec` (infrastructure ADR proposals) |
| BDD acceptance criteria | `spec` (after ADRs are resolved) |
| Project scaffold | `scaffold` (reads services registry) |

There is no step that generates architecture, domain model, or component structure upfront.

---

## Workflow

**Canopy is a REPL, not a subcommand CLI.**
Run `canopy` (no arguments) to start the interactive session. Commands are typed at the
`canopy>` prompt inside the running process. The only shell-level flag is `--llm-debug`.

```
$ canopy              ← starts the REPL; the shell sees no subcommands
canopy> ...           ← all commands are typed here
```

There is no `canopy <command>` shell invocation. Attempting it will produce an "unexpected
argument" error. Do not guess at shell-level subcommands that don't appear in `--help`.

Available REPL commands:

```
canopy> init
  └─ one question: "What are you building?"
  └─ saves: idea.yaml
  └─ project name derived from git remote or folder name — no vision generated

canopy> intent "<behavioral statement>"   (repeat per requirement)
  └─ LLM derives user stories
  └─ human curates: status → accepted | rejected
  └─ auto-extracts: domain entities and events → domain_registry.yaml
  └─ saves: stories.yaml, roles.yaml, domain_registry.yaml

canopy> stories      → display backlog
canopy> domain       → display accumulated domain vocabulary (edit freely)

canopy> spec <story-id>   (story must be accepted)
  └─ LLM proposes ADRs: structural, UI, tech stack, infrastructure
  └─ human gates each: Accept / Modify / Reject
  └─ accepted ADRs → decisions/adr-NNN-slug.yaml
  └─ services and tech stack accumulate → services.yaml
  └─ generates BDD scenarios grounded in resolved architecture → stories/<id>/spec.yaml
  └─ generates OpenAPI spec → stories/<id>/openapi.yaml

canopy> scaffold [--dir <path>]
  └─ reads services.yaml (skips infrastructure components)
  └─ requires at least one service with a decided technology
  └─ runs scaffold commands: Spring Boot, Angular, React, Node.js, etc.

canopy> implement <story-id>
  └─ detects actual package from scaffolded *Application.java (no guessing)
  └─ generates implementation plan: one LLM call per service → merged and sorted
  └─ human confirms plan before execution
  └─ executes step by step, reindexes after each file
  └─ runs test/fix loop per service after all steps complete (up to 5 iterations)
  └─ saves: stories/<id>/plan.yaml (generates stories/<id>/openapi.yaml too, if `spec` hasn't already)

canopy> dependencies  → display the global dependency decision log
```

**There is no `reset` command.** To reset a plan, edit `.canopy/stories/<id>/plan.yaml`
directly and set all `status: done` entries back to `status: pending`.

---

## ADR Proposal Categories

`canopy spec` asks the LLM to surface four categories of architectural questions:

1. **Structural** — service ownership, data responsibility, event design, API boundaries
2. **UI** — if the story has a human actor, what frontend delivers this capability?
3. **Tech stack** — for each new service or frontend, what technology?
4. **Infrastructure** — persistent storage (database per data-owning service) and event broker

Naming rules enforced in prompts: kebab-case only (`product-registry`, `catalog-service`).
Infrastructure entries (`component_type: infrastructure`) are tracked in services.yaml
but skipped by `canopy scaffold` — they belong in docker-compose or equivalent.

---

## Artifacts

```
.canopy/
  idea.yaml                        raw idea description
  vision.yaml                      project, problem, goals
  stories.yaml                     story backlog with status
  roles.yaml                       accumulated user roles (as_a values)
  domain_registry.yaml             entities and events (edit freely)
  services.yaml                    services + tech stack + responsibilities
  scaffold.yaml                    generated scaffold plan
  decisions/
    adr-NNN-slug.yaml              accepted architecture decisions
  stories/
    <story-id>/
      spec.yaml                    BDD scenarios for that story
      plan.yaml                    implementation steps with status (resume-safe)
      openapi.yaml                 OpenAPI spec snapshot used during implementation
```

---

## Codebase Structure

### Canopy (planning and implementation engine)

```
canopy-core/       data types (structs, enums, serde)
canopy-llm/        LLM client, prompts, and generation functions
canopy-storage/    save/load wrappers around .canopy/
canopy-cli/        CLI commands (clap), interactive prompts (dialoguer)
```

When adding a new capability: type in core → storage helpers → llm prompt/function → cli command.

**Test file placement is language-specific, by design.** TypeScript/JavaScript tests are
co-located next to their source (`src/services/Widget.ts` ↔ `src/services/Widget.test.ts`) —
the JS/TS ecosystem convention. Java tests use the Maven-style mirrored tree
(`src/main/java/...` ↔ `src/test/java/...`). These are deliberately NOT unified: each stack
follows its own ecosystem's convention, not the other's. `detect_layer()`, `is_test_file()`,
and `derive_test_file_path()` all branch on this split — don't "simplify" one language's
file-placement logic to match the other's.

### Roots (repository intelligence engine)

Roots indexes a repository into a structured graph and answers queries about it.
Canopy uses Roots in repository mode to get context packets instead of reading raw files.

```
roots-core/        graph types: Workspace, Project, Module, File, Symbol, Relationship
roots-parser/      language parsers that populate the graph (Java, Kotlin, TypeScript)
roots-context/     context packet assembly, impact analysis, fact extraction
roots-storage/     SQLite-backed graph persistence
roots-cli/         `roots` CLI: index, query, discover, impact
```

The graph hierarchy: Workspace → Project → Module → File → Symbol.

Roots is the authoritative source of truth in repository mode.
When Roots is available, prefer it over `canopy-storage` for symbol and relationship queries.

**Canopy only calls Roots as an external command — never as a linked library.** `canopy-cli`
must not depend on `roots-storage` or `roots-context` in `Cargo.toml`, and must not call
`Store::open`/`query_exact`/`feature_context` or any other roots-storage/roots-context API
in-process. Every query goes through `std::process::Command::new("roots")` (init, index, symbol,
dump, context, ...), capturing stdout and parsing its JSON — the same pattern `ensure_indexed`/
`reindex` already used for init/index before this rule was written down. Reason: canopy linking
Roots' internal storage crate directly coupled canopy to Roots' SQLite schema and query API —
a schema change inside Roots could silently break canopy with no interface boundary to catch it.
The `roots` CLI's stable JSON output *is* that boundary; treat it the same as any other external
tool canopy shells out to, not as an internal module. One narrow exception: `find_test_call_shape`
(in `canopy-cli/src/roots.rs`) uses `roots_parser::find_subject_calls` directly — this parses a
test file's in-memory content that hasn't been written to disk or indexed yet (it runs *before*
the stub file exists), so there's no live Roots index or CLI command to shell out to in the first
place; it's a plain tree-sitter parsing utility, not a query against Roots' running state. That's
still a direct `roots-parser` dependency, but it's parsing, not talking to Roots-the-system —
don't read this exception as license to add other direct roots-storage/roots-context calls.

**How Roots integrates with `canopy implement`:**
- `build_sibling_section` calls `get_ts_module_surface` (compact export surfaces) for each step's `depends_on` files
- Falls back to full file content only when Roots is unavailable or the file isn't indexed yet
- `reindex()` runs after each step write to keep the index current

The compact surface (exported interfaces, classes, function signatures) is the primary context
mechanism for implementation steps — not skill rules. When generated code ignores an existing
symbol (e.g. calls `createProduct` should exist but the model generates its own UUID instead),
the fix is to verify Roots is indexed and the surface reaches the prompt, not to add a rule.

---

## LLM Providers

Canopy supports two providers: `anthropic` and `ollama`. The `ollama` provider uses the
OpenAI-compatible API (`/v1/chat/completions`) and works with any server that speaks that protocol —
Ollama, llama.cpp server, or any OpenAI-compatible endpoint.

Provider and model are configured per-agent in `.canopy/config.yaml`:

```yaml
default:
  provider: ollama
  model: qwen2.5-coder:14b
  base_url: http://localhost:8080
agents:
  intent:
    provider: ollama
    model: qwen2.5:14b
    base_url: http://localhost:8080
  architect:
    provider: ollama
    model: qwen2.5:14b
    base_url: http://localhost:8080
  planner:
    provider: ollama
    model: qwen2.5-coder:14b
    base_url: http://localhost:8080
  developer:
    provider: ollama
    model: qwen2.5-coder:14b
    base_url: http://localhost:8080
```

`for_agent()` returns the agent-specific config or the default — no merging. Each agent that
needs a non-default `base_url` must declare it explicitly.

### llama.cpp server (llama-server)

Preferred local backend. Exposes OpenAI-compatible API on `http://localhost:8080`.

```
llama-server \
  --hf-repo Qwen/Qwen2.5-Coder-14B-Instruct-GGUF \
  --hf-file qwen2.5-coder-14b-instruct-q4_k_m.gguf \
  -c 16384 \
  -ctk q8_0 \
  -ctv q8_0
```

| Flag | Purpose |
|---|---|
| `-c 16384` | Context window — 16K covers all planning and most implementation prompts |
| `-ctk q8_0` | KV cache quantization for keys — saves VRAM, better quality than Q4 |
| `-ctv q8_0` | KV cache quantization for values |

Note: `-ctk`/`-ctv` use a single dash. `--ctk`/`--ctv` (double dash) is invalid.

The model name in config.yaml is informational — llama-server ignores it and uses whatever
model is loaded. Verify connectivity: `curl http://localhost:8080/v1/models`

LLM debug log (requires `--llm-debug` flag or env): `<project>/.canopy/logs/llm-debug.log`
Tail it: `tail -f <project>/.canopy/logs/llm-debug.log`

---

## Tech-Stack Skills

`canopy implement` injects a **skill** into each per-service plan prompt based on the service's
technology. A skill is a concise, authoritative rules block that tells the LLM the exact
conventions for that tech stack — package layout, file paths, naming, forbidden patterns.

Skills are defined in `canopy-llm/src/skills/tech_stack.rs` as `TechStackSkill` structs.
`file_layout`, `layer_order`, and `notes` apply regardless of layer; import/naming rules use
one of two shapes — legacy `namespace_rules` (shown in full regardless of layer) or the
layer-partitioned `common_rules` (every layer) + `layer_rules` (keyed by `detect_layer()`'s
output, e.g. `"model"`, `"route"`, `"infrastructure"` — only the file's own layer's entry is
injected). New skills should use the partitioned shape; it's what makes per-layer scoping
(see Prompt House Style below) possible.

Each skill has three render modes:

| Method | Used by | Contains |
|---|---|---|
| `render_for_planning()` | `plan_skill_for_technology()` → plan prompt | `file_layout` + `layer_order` only |
| `render_for_layer(layer)` | `skill_for_technology()` → step/fix prompts | `file_layout` + `layer_order` + only that layer's rules |
| `render_all_layers()` | contexts not tied to one file (e.g. dependency proposals) | every layer's rules concatenated |

The split keeps planning prompts lean (~300 tokens vs ~1,500) so the planner model focuses on
file enumeration and dependency graph — not import syntax or zod chain rules.

The matcher functions share the same technology detection logic:

| Skill | Matched by |
|---|---|
| Spring Boot 3 (Jakarta EE) | "spring", "quarkus", "micronaut", "java", "kotlin" |
| React + TypeScript (Vite) | "react", "vite" |
| Angular | "angular" |
| Node.js / Express | "node", "express", "nest" |

**What a skill encodes:**
- Exact file paths and source roots (computed from the detected scaffold package)
- Sub-package names and layer ordering
- Forbidden patterns (e.g. `javax.*`, `../../` imports)
- Required dependencies (e.g. `spring-boot-starter-validation`)
- Strict scope: only files directly required by the story — no speculative abstractions

**What a skill must NOT encode:**
- Specific bug workarounds observed in one generation run ("NEVER import from 'crypto'", "NEVER call publishEvent")
- Implementation details that Roots already provides through symbol surfaces
- Rules that only apply to one domain entity or method name

When the impulse is to add "NEVER use X" for a specific import or method, ask first:
is this a structural principle (belongs in the skill) or a bug report (belongs in the fix loop)?
Skills that grow beyond their structural core dilute attention on small models — each new rule
crowds out the ones that matter. Audit and trim instead of appending.

**Scope discipline.** Each skill explicitly lists what must NOT be added unless a story requires it.
For Spring Boot: no extra Application classes, no sub-package for the entry point.
For React/Vite: no custom hooks, page components, route files, Redux slices, or utility modules
unless the story's acceptance criteria call for them. Architecture emerges story by story.

**Adding a new skill:** implement a builder function returning `TechStackSkill`, add a match arm
in both `skill_for_technology` and `plan_skill_for_technology`, and document it in the table above.

**Prefer structural placeholders over example entities.** Default to bracketed placeholders —
`<entity>`, `<domain object>`, `<aggregate>`, `<event>` — rather than any concrete noun, even the
project's own established `Widget`/`createWidget` convention. Live-verified reason: the reference
model treats an example noun as a candidate answer, not just an illustration — a prompt showing
"register an account" produced literal `account`-vocabulary output for a story about registering
a manufacturer, and a later fix that swapped in `Widget` repeated three times in one example set
was flagged for recreating the same anchoring risk with a different noun. Any concrete noun is a
potential output token; a bracketed placeholder can't be copied literally because it isn't a real
word. Field names: prefer `<fieldName>` over `name`/`optionalField` for the same reason, unless
the concrete name is itself demonstrating a naming *convention* (see below).
Only use a concrete noun (`Widget`/`createWidget`, never a project-specific one like `Product`)
when the structure being taught genuinely requires an actual word — e.g. demonstrating a
camelCase-vs-PascalCase transformation, or a multi-field validation shape a bracket alone can't
convey. When in doubt, remove the noun. The pattern is established in the Models section of
`node_express_skill()` — that file predates this refinement and is a lower-priority candidate for
a future pass, not a template to copy into new work.

---

## Prompt House Style

Every string sent to the model — skills and prompts alike, across `canopy-llm/src/prompts/*.rs`
and `canopy-llm/src/skills/*.rs` — follows the same rules:

- **ALWAYS/NEVER, not paragraphs.** State a rule as `ALWAYS <imperative>.` / `NEVER <imperative>.`,
  not a multi-sentence explanation mixing the rule with its rationale. Short example fragments
  (1-4 lines of code) are encouraged — this is a "no restated rationale" rule, not a
  "no examples" rule.
- **No duplicate injection.** The same rule must not reach the model twice in one call, even
  worded differently — e.g. a rule stated once via a tech-stack skill's layer section and again
  in a separate IMPORTANT-list bullet in the same prompt. Trace every section a changed string
  feeds into before assuming it's the only copy.
- **Proximity.** A rule sits next to the content it governs. An instruction far from the thing
  it constrains is a known failure mode for the local reference model ("lost in the middle") —
  even a correct, well-worded rule gets ignored if it's positioned far from its subject.
- **Layer-scoping correctness.** A `layer_rules` entry must be keyed to the layer that actually
  needs it. A rule filed under the wrong layer key (e.g. a *route*-file rule accidentally placed
  under `"app"`) is correctly worded but never reaches the file it's meant to constrain — this
  bug produces no error, just silent non-compliance, since layer-scoped rendering only sends the
  matching layer's rules.
- **Prefer exhaustive enumeration over holistic review.** When a prompt asks the model to review
  a known, boundable set of items (fields, constraints, scenarios, behaviors, dependencies,
  clusters) for gaps, ALWAYS have it iterate the set explicitly rather than judge it holistically
  — a small model reliably misses items under open-ended review that it catches under explicit
  enumeration. Only fall back to holistic phrasing when no bounded set exists to enumerate.
  BAD:  "Review the specification and identify missing constraints."
  GOOD: "For each field: for each constraint: determine whether coverage exists."
  GOOD: "For each scenario: determine whether at least one behavior was produced."
  Confirmed live: Stage 0's original constraint audit ("review for gaps") found 4 of 9 real gaps;
  rewriting it as an explicit field × constraint traversal found all 9. The same shift fixed
  Stage 1 behavior extraction, dependency review, and clustering review.
- **When a model ignores a correct instruction, fix the prompt, not the code.** Verify with
  `llm-debug.log` that the instruction actually reached the prompt as intended before concluding
  it's a compliance problem rather than a missing-instruction problem. If it's genuinely a
  compliance problem, the fix is to shorten and reposition the instruction — never a Rust-side
  filter, override, or post-processing step that rewrites, corrects, or replaces the model's own
  output, and not by default adding more prose either. A longer WRONG/CORRECT example is a last
  resort after a short instruction has been tried and shown (with real evidence) to still fail —
  not the first move. This is distinct from a **deterministic audit** — Rust code that compares an
  already-generated artifact against an independently-known fact (existing domain vocabulary, a
  coverage list, an ownership record) and fails loudly on disagreement without touching the
  artifact's content (Entity Continuity, Event Continuity, coverage/contract/dependency checks).
  Audits enforce an invariant; compensation papers over a compliance gap. The test: does the Rust
  code change what the model produced, or only reject/accept it as-is?

Use the `canopy-prompt-reviewer` subagent (`.claude/agents/canopy-prompt-reviewer.md`) to check
prompt/skill changes against these rules before installing.

---

## Principles

**Intent before coding.** No implementation without an accepted story and a resolved spec.

**Minimise context.** Pass the smallest useful input to the LLM — facts, summaries, symbols.
Never dump entire files. The prompt is the design.

**Generate diffs, not files.** Prefer targeted edits over full file regeneration.

**Explain decisions.** ADRs are first-class outputs. Reasoning is not a comment — it is the record.

**Model quality is secondary. Context quality is primary.**
Canopy succeeds when a small model can make large changes because the system provides excellent context.

**Fix loop scope.** The fix loop handles compile errors and test failures — broken imports, missing
methods, type mismatches. Stylistic issues that don't break the build are not fix loop targets and
should not drive prompt changes. If something repeatedly causes the build to fail, that is a skill
gap; add a structural principle. If it's just inconsistent but working, leave it.

**DDD aggregate lifecycle.** Three responsibilities that must never be mixed in a plan step:
- **Factory** (model file): constructs a new aggregate instance, assigns `id` and `createdAt`
- **Repository**: receives a fully-constructed aggregate and persists it unchanged — never assigns ids or timestamps
- **Application service**: calls the factory to construct, then the repository to persist

A plan step description should name its layer responsibility using the verb that fits:
`Defines` (model), `Constructs` (factory), `Persists` (repository), `Orchestrates` (service),
`Handles` (route), `Translates` (middleware). An ambiguous verb ("implements", "manages") is a
signal the step is conflating responsibilities.

**Escalation order when the model fails at something: tool, then prompt, then gated code —
never skip a rung.** Every fix this project makes to a recurring model mistake falls into one of
three tiers, tried in this order:

1. **Is this a tool call, or a stated fact?** If the answer is mechanically computable and the
   model needs to *decide when to ask* (the space of things it might need is too large to
   pre-inject everything — "where is symbol X" for any X), offer a tool: `find_symbol`
   (`canopy-cli/src/roots.rs`, `canopy-llm::find_symbol_tool_spec`) resolves a missing import by
   looking it up instead of guessing, and the fix loop shows each call live in the console,
   collapsing to the looked-up result. If the value is always needed and cheap to compute ahead
   of time instead, inject it as a stated fact rather than a tool — `find_test_call_shape`'s
   `observed_call` fact (Roots-parsed test call shape) and the `available_packages` fact
   (`read_available_packages`, replacing a static "don't import moment/uuid" blocklist with the
   project's real `package.json` dependencies) are both this shape: always relevant, no judgment
   call about *whether* to look them up. Prefer this tier whenever the failure is "the model
   doesn't know a fact," not "the model doesn't know the convention."
2. **If it's not a lookup — it's a judgment or convention the model needs to be taught — fix the
   prompt.** Fix the discovery or skill prompt to make the requirement clearer; do not add Rust
   safety nets (path injectors, output filters, post-generation reordering). The prompt is the
   design; the model should get it right because the context is good, not because Rust patches
   the output. See Prompt House Style above for how to fix a prompt the model isn't complying
   with.
3. **Only once tiers 1 and 2 are genuinely exhausted — a real compliance limitation, not a
   missing-lookup or missing-instruction problem — propose gated code.** Reach for code
   enforcement only when the problem is structurally impossible to express as a tool or a prompt
   (e.g. numbering step IDs after a merge across services), and even then, propose it and stop
   for human approval before implementing — see "Diagnosing Dogfooding Runs" below.

Don't jump straight to tier 3 because a code fix is easier to write than a good prompt, and don't
reach for tier 2 when the actual gap is tier 1 (a prompt teaching path arithmetic or an import-
type rule is noise once a tool can just answer the question directly — see the `find_symbol`
tool's evolution in `canopy-llm/src/skills/tech_stack.rs` for a concrete before/after).

**TDD Red phase checks compilation, not runtime — that gap is structural, not an oversight.**
`tsc --noEmit` (or `javac`) can't catch a test that compiles fine but crashes at Jest/JUnit
*runtime* for a non-domain reason (e.g. `jest.spyOn` on an empty mock object). Green phase
deliberately protects the test file from further edits, so a runtime-only test bug that survives
Red becomes unfixable once Green begins — the test is right there, but nothing is allowed to
touch it. This is why Red phase also runs the test once and checks the failure is the stub's own
expected rejection (e.g. `'not implemented'`); anything else routes to a bounded fix attempt on
the test file while it's still editable (`run_red_test_sanity_check` in `canopy-cli/src/fix_loop.rs`).
Don't remove this check thinking the compile check alone is redundant with it — they catch
different failure classes.

**A clean test PASS at Red phase means the stub lied, not that the test is broken.** The model
is sometimes asked for a stub (`throw new Error('not implemented')` in every method) and hands
back a full implementation instead, ignoring the stub-only instruction. `run_red_test_sanity_check`
detects this via `test_file_passed_cleanly` (`canopy-cli/src/build_output.rs`) and returns
`RedSanityOutcome::AlreadyImplemented` rather than treating the PASS as "an error to fix." The
caller (`canopy-cli/src/commands/implement/execute.rs`) skips Green phase entirely in that case —
Red's own compile check plus this sanity check have already proven the file compiles and passes,
so there's nothing left for Green to verify, and regenerating from scratch would only risk
replacing a working answer with a fresh gamble (confirmed: this is exactly how an
exactOptionalPropertyTypes violation got introduced into an implementation that was otherwise
already correct — Green's unconditional regeneration is what broke it, not the over-eager stub).
Before the PASS-detection fix, a PASS fell through to the generic "fix the test file" loop with
the literal string `PASS <file>` as its "errors," and with nothing real to fix, the model invented
unrelated changes across attempts until it drifted the
test into a completely different, unrelated shape. If a fix loop's output looks like it hallucinated
an unrelated domain, check whether the input it was fed was ever a real error in the first place.

---

## Running a Dogfooding Session Non-Interactively

Canopy's REPL reads commands via `rustyline`, which checks whether stdin is a real terminal and
falls back to a plain `BufRead::read_line` loop when it isn't — so a command can be piped in
without needing raw-terminal support:

```
cd <dogfooding-project-root>
printf 'implement <story-id>\nexit\n' | canopy --llm-debug > run.log 2>&1
```

`--llm-debug` is a REPL-startup flag (`canopy --llm-debug`), not an argument to `implement`
itself — the REPL re-adds it to every command typed inside the session. It only controls whether
LLM request/response payloads are also printed to the console; the debug log file at
`<project>/.canopy/logs/llm-debug.log` is written on every run regardless of the flag. Passing it
is what makes redirected console output (`run.log` above) useful for following a run live, since
it mirrors the same LLM I/O the log file gets, interleaved with the progress lines — no need to
tail a second file while the run is active. The log path is relative to CWD, so `cd` into the
dogfooding project root first or the log lands somewhere unexpected.

**This only runs fully unattended when resuming a story that already has a `plan.yaml`.**
`implement <story-id>` loads an existing `.canopy/stories/<id>/plan.yaml` if one exists and skips
straight to step execution — the only `dialoguer` confirmation prompt in the whole `implement`
flow ("Execute this plan?") sits in the *fresh-plan-generation* branch, never reached on resume.
Once past that point (or when there was never a plan to confirm), the Red/Green TDD loop, the fix
loop, and the final cross-service regression pass run with zero further prompts. If a story has
no `plan.yaml` yet, piping an answer like `y\n` will NOT satisfy that confirmation — `dialoguer`'s
`Confirm` reads raw key events from an actual terminal, not a stdin line, and errors immediately
when stdin isn't a tty. Every call site in canopy wraps that error with `.unwrap_or(false)` /
`.unwrap_or(default)`, so a non-interactive fresh run doesn't hang — it just silently declines and
leaves the plan saved but not executed. Practical implication: run `implement` once interactively
to get a story past its first plan confirmation, then all later resumes can be scripted.

Since this can run for many minutes across several steps, launch it as a background process and
follow along rather than blocking on it:

```
tail -f run.log                                    # console mirror, if --llm-debug was passed
tail -f <project>/.canopy/logs/llm-debug.log        # always written, even without --llm-debug
```

Grep-able markers to watch for while a run is live (each is a plain string, not a progress-bar
artifact, so they survive redirection to a file):
- `[N/M] <file> — TDD 🔴` / `TDD 🟢` — which step, and which TDD phase, is currently running.
- `generating test` / `generating stub` / `implementing` — which of the three LLM calls per step
  is in flight.
- `Test file to create :` / `Implementation file :` — inside an `--llm-debug` payload dump,
  confirms which file's prompt you're looking at (useful once the log has many calls in it).
- `No fixable errors found — manual fix needed.` — the fix loop exhausted its iterations; the
  step is stuck and needs the kind of diagnosis described below, not another automatic retry.
- `removing stale artifact (leftover from an interrupted run)` — a prior run was killed or
  crashed mid-step; canopy is clearing a half-written file before regenerating it.

---

## Driving an Interactive Dogfooding Session (`intent` → `spec` → `behaviors`)

The section above covers `implement` resuming a saved plan, where every remaining prompt is a
`confirm_default` that degrades gracefully without a real terminal. `intent`, `spec`, and
`behaviors` are different: they gate on `select_required` (story acceptance, ADR accept/modify/
reject, Stage 2 decision resolution) — this reads raw key events via `dialoguer::Select`, which
hard-errors with "not a terminal" on a plain pipe. Piping answers with `printf | canopy` does not
work for these commands past the first `select_required` prompt.

**Use `expect` (or another pty-allocating driver), not a plain pipe.** `expect`'s `spawn` gives
the child process a real pty, so `Select`/`Confirm`/`Input` all render and respond correctly.
Minimal pattern for one command with N `select_required` gates, all accepting the sensible
default:

```tcl
#!/usr/bin/expect -f
set timeout 240
log_file -a /path/to/session.raw.log
cd <dogfooding-project-root>
spawn canopy --llm-debug
expect "canopy>"
send "behaviors <story-id>\r"
expect {
    -re {Accept this ADR\?} { send "\r"; exp_continue }
    -re {continue to behavior extraction\?} { send "\r"; exp_continue }
    -re {Clustering looks correct} { send "\r"; exp_continue }
    "canopy>" { }
    timeout { puts "TIMEOUT-MARKER" }
}
send "exit\r"
expect eof
```

**`select_required`'s `default` parameter is almost always index 0, and Enter alone accepts the
default** — every gate in `intent.rs`/`spec.rs`/`behaviors.rs` (story accept/reject, ADR accept/
modify/reject, testing-framework choice, decision resolution's option list) passes `default: 0`,
and index 0 is consistently the "Accept" / recommended option. This means a single generic
`-re {some known prompt text} { send "\r"; exp_continue }` clause per gate is enough to drive an
entire multi-gate command without needing arrow-key navigation, *provided* accepting every
default is actually the reviewer's intent — for anything else (rejecting a story, picking a
non-default ADR option, choosing a specific decision resolution), send the arrow-key escape
sequence(s) (`\x1b[B` down, `\x1b[A` up) before `\r`, or resolve it by hand-editing the saved
artifact afterward instead (see below).

**`intent "<multi-word statement>"` typed inline does not work — the REPL's line parser has no
shell-quote awareness.** `cli.rs` tokenizes a typed line with plain `split_whitespace()`, so
`intent "As a foo, I want..."` splits into many separate arguments (the leading `"` stays glued
to the first word), and clap rejects everything after the first token as an unexpected argument.
Use the command's own interactive fallback instead: send `intent` bare, wait for the "Behavioral
intent" `Input` prompt, and send the full statement as a separate line — spaces and punctuation
are preserved correctly since `dialoguer::Input` reads a raw line, not REPL-tokenized args.

**Human-in-the-loop corrections belong in the saved YAML, not in cleverer keystroke scripting.**
When a generated artifact needs a real correction (a bad `want` field, an unresolved open
question, a missing constraint-coverage scenario), it's far more robust to accept the LLM's
output as generated, then edit `stories.yaml`/`spec.yaml`/`domain_registry.yaml`/`roles.yaml`
directly with the fix, than to fight `dialoguer`'s "Accept with edit" text-replacement UX (which
starts the cursor at the end of pre-filled `with_initial_text`, so replacing it needs a kill-line
keystroke first) through a scripted pty session. Re-run the affected command afterward if the
correction changes what a later stage needs to see.

**`select_required` genuinely blocks a run when nothing sensible can be defaulted to.** If a
stage's own completeness check reports a blocking gap (Stage 0's `has_blocking_gaps()`, for
example), there is no way to "answer past it" via a gate — the command returns before any further
prompt appears. The only ways through are: fix the underlying artifact (add the missing scenario,
resolve the open question) and re-run, or — for throwaway live-verification only, never for a
real run — temporarily short-circuit the check in code (`&& false` on the gate) and revert before
committing, the same pattern already used throughout this project's own Stage 0–4 development.

---

## Diagnosing Dogfooding Runs

Canopy is dogfooded against separate throwaway projects driven entirely through the `canopy`
REPL — these are personal to whoever is running the session; never name one in a commit message
or anything else shared from this repo, refer to it generically ("a dogfooding project") instead.
When a dogfooding run misbehaves, the primary source of truth is that project's own
`.canopy/logs/llm-debug.log` — every prompt and every raw LLM response, in call order, with a
`[YYYY-MM-DDThh:mm:ssZ]` timestamp on every line.

**Find the right slice of the log before reading it.** The log accumulates across every run ever
made against that project, often spanning days and hundreds of thousands of lines. Grepping a
skill/error string in isolation will surface matches from unrelated earlier runs. Anchor first:
check the relevant `.canopy/stories/<id>/plan.yaml`'s mtime (or the log file's own tail) to find
roughly when the run in question happened, then locate the actual boundary by searching for the
step's own marker text (`Test file to create : <path>`, `Implementation file : <path>`) near that
timestamp rather than trusting line-count proximity alone.

**Rule out a recent prompt/skill change before blaming it.** "This got worse after our last fix"
is a hypothesis, not a finding. Two checks settle it quickly: (1) could the changed text even
reach the affected prompt? — `render_for_layer`/`render_for_planning` scope skill text to one
layer, so an edit to the `"repository"` block cannot appear in a `"model"`-layer prompt; grep the
actual prompt dump in the log for a distinctive phrase from your change to confirm one way or the
other. (2) `git log -S '<distinctive phrase>'` (or `git blame` on the surrounding lines) to find
when the suspect text was actually introduced, then compare that commit's timestamp to the failing
run's log timestamps — a run that predates the change cannot have been caused by it.

**A hallucination is a symptom, not the defect.** When a fix loop drifts into content that bears
no relation to the story (wrong field names, an unrelated entity shape), don't fix the drifted
output — walk backward through the log to the first iteration in that loop and read exactly what
was fed to the model as "the error." If it wasn't a real error (e.g. a bare `PASS <file>` line
handed to a "fix these errors" prompt), the defect is in the harness's outcome classification, not
in prompt wording — fix the Rust-side check that misclassified the outcome (see the TDD Red-phase
note above for a concrete example), not the prompt.

**Compliance gap vs. harness gap.** If the model ignores an instruction that's demonstrably
present in the actual prompt (confirmed via the log), that's a prompt-wording problem — see
Prompt House Style. If the harness's own control flow has no branch for an outcome the model
legitimately produced (a stub that over-delivers, a test that passes when it "shouldn't"), that's
a Rust-side classification gap — fixing it in code is correct, not a violation of "fix prompts,
not code," because the thing being fixed is deterministic control flow, not LLM output.

**Always dig deep on the prompt before reaching for anything else — and gate the next move on
the human.** Before proposing (let alone implementing) a code-level workaround, confirm from the
log: was the rule actually in the prompt, worded correctly, and reasonably positioned? Read the
model's own `##CANOPY_DEVIATIONS##` self-report too, but don't trust it uncritically — it can
confidently say "None" on the exact same response that violated a rule and ignored a formatting
instruction, so a clean self-report is not proof of compliance. Only once that dig is done and
genuinely inconclusive (the rule was right there, correctly placed, and the model still ignored
it — a real compliance limitation, not a missing/misplaced instruction) does a structural or
code-level fix become the right next move. Even then: propose it and stop — get the human's
explicit go-ahead before implementing, don't fold "diagnosed the root cause" and "here's the code
fix, already applied" into the same turn. What counts as sufficient digging: grep the actual
prompt dump for the rule's exact text (confirms presence and position), check whether it's a
generic/default TS pattern the rule overrides (weak in-context rule vs. strong training prior is
a very different failure mode than "instruction never arrived"), and read the self-reported
deviations for that specific call before concluding anything.

---

## Commit Discipline

This project overrides the general default of never committing without being asked. On this
repo specifically: **commit at natural checkpoints without waiting to be asked**, as long as
every condition below holds.

ALWAYS commit, unprompted, when ALL of these are true:
- `cargo build --workspace` and `cargo test --workspace` are both green.
- A distinct unit of work just finished — a bug fixed and verified, one file's pass in a
  multi-file task, a feature landed — not a mid-thought pause.
- The change isn't still under active discussion (e.g. the user is weighing whether the
  approach is even right).

NEVER commit when:
- The build is red, or tests haven't been run since the last edit.
- Work is deliberately left mid-file/incomplete (e.g. a multi-step plan not yet finished).
- The user is actively iterating on the same change within the current exchange.

When doing multi-step work (auditing N files, fixing a chain of bugs), track "commit <unit>"
as its own task alongside the work items — via TaskCreate/TaskUpdate if a task list is already
in use — so finishing the list surfaces the commit instead of silently skipping it.

Write real messages: a short imperative subject line, then 1-3 sentences on *why* — the git log
must tell the story of this project, not just list touched files.

This authorization is scoped to routine checkpoint commits. It does not extend to force-push,
amending published commits, or anything else the general git safety rules already gate —
those still require an explicit ask.

**Backing safety net, not a substitute for judgment.** `.claude/hooks/checkpoint-reminder.sh`
(wired into `.claude/settings.json` — the shared, project-wide, checked-in config, not the
personal and gitignored `.claude/settings.local.json`) fires on session Stop if source changed
and nothing is staged. It is purely informational: no git command, no install, no nested agent
call — it only surfaces a reminder, hash-gated so it fires once per distinct diff and can never
block a session from ending. If it appears, a checkpoint was probably missed; commit before
continuing.

An earlier version of this automated the commit itself (auto-stage, invoke a nested `claude -p`
to draft/gate the message, commit, reinstall) — Claude Code's own safety classifier rejected it
as a self-modification risk: unsupervised commit-and-deploy at session end, decided by a
background agent nobody was watching. Don't reintroduce that shape. The reminder-only design
above is the deliberate replacement — supervised judgment stays with whichever session is
active; the hook only makes sure that judgment gets exercised instead of silently skipped.

---

## Knowledge Capture Cadence

`docs/{retrospectives,blog-drafts,principles,reports,narratives,open-questions}/` is not a
one-time export — it's meant to stay current as the project evolves, across whichever developer or
session is doing the work. Six distinct artifact types, each answering a different question:

| Directory | Answers | Scope |
|---|---|---|
| `retrospectives/` | What happened during a period? | One cross-cutting day, or a project-level milestone |
| `principles/` | What reusable lesson generalizes across many events? | Cross-cutting, evidence-graded, includes counter-evidence |
| `blog-drafts/` | What's one story worth telling? | A single bounded incident (or milestone) with a full belief→evidence→change arc and a genuine prediction at stake |
| `reports/` | What happened across sessions against one story? | One story, extended chronologically |
| `narratives/` | How did our thinking on one thread evolve across months? | Weeks-to-months, connects multiple retrospectives/principles/blog posts into one arc — see `docs/narrative-analysis.md` for how these get compared and ordered |
| `open-questions/` | What important questions have we discovered but deliberately deferred? | One architectural/design question, tracked from discovery through eventual resolution — preservation, not project-management ceremony |

Don't force a finding into the wrong type. A recurring pattern across many small commits is a
principle, not a blog post (a blog post needs one dramatizable scene to open on, which a diffuse
pattern usually doesn't have). A single-incident story that's genuinely bounded is a blog post, not
a narrative. A thread that only makes sense read across several retrospectives and principles
together — "how did X evolve" — is a narrative, and narratives require a dedicated top-down pass
(pick one thread, trace it end to end) rather than emerging automatically from the bottom-up
retrospective/principle/blog-post cadence below.

**Trigger-based, not calendar-based.** None of these six get created because time passed — each
has its own trigger, checked when relevant work happens, not on a schedule. Historical evidence for
this shape: the historical-reconstruction sweep found retrospectives are common, principles less
so, blog-worthy investigations rarer still, and narratives rarest of all — a fixed cadence would
either starve the rare types waiting for a schedule slot, or pad the common ones with nothing to
say. `docs/reports/`'s own trigger ("at the close of a sweep") already worked this way before the
other five were generalized to match — proof this shape holds up in practice, not just in theory.

- **Retrospectives**: a day's work is cross-cutting (spans multiple stories or areas) or crosses a
  project-level milestone (first production wiring, first schema change, first cross-story
  generalization). A story-scoped reproducibility sweep belongs in that story's `reports/` file
  instead (see below) — don't double-capture the same content in both places.
- **Principles**: a new, evidence-backed recurring pattern appears; existing evidence for a
  principle is strengthened or weakened by new work; or a principle's own stated confidence-
  limiting caveat (e.g. "medium because only one story has been tested") is resolved or
  contradicted by new evidence. Extending a principle's evidence list without changing its
  confidence rating is a valid, common outcome — don't inflate confidence just because more
  evidence accumulated in the same shape as before.
- **Blog drafts**: a complete investigation closes, OR a major evidence-backed intermediate
  milestone is reached — AND, either way, a genuine prediction was at stake that could have gone
  the other way, not a foregone conclusion. Follow the style in that directory's existing drafts
  (evidence-first, prediction-driven, concrete numbers — see the drafts themselves for the bar).
- **Reports**: at the close of a reproducibility sweep or dogfooding session against a specific
  story — the original trigger-based artifact type; unchanged here. Don't leave sweep results only
  in a scratchpad or a chat transcript. Scratchpad files are session-scoped and can be lost;
  `docs/reports/` is the durable home. Prefer one file per story, extended chronologically as new
  sessions run, over one file per sweep (see `docs/reports/manufacturer-001.md` for the shape).
- **Narratives**: a long-running thread's own previously-stated open question or "what's left"
  gets resolved by recent work; a major project belief changes; or enough new
  retrospectives/principles have accumulated that an existing narrative's "Current View" or "Open
  Questions" section looks stale. Check this by re-reading the narrative's own stated open items,
  not by vibe. When a narrative *is* updated, update only the sections the trigger actually
  touched — a resolved open question does not require re-reviewing the whole thread from scratch;
  a full top-down regeneration pass (review retrospectives, principles, blog drafts, commit
  history, and design-doc/CLAUDE.md history together) is its own separate, much rarer trigger
  (enough accumulated drift that "Current View" reads as stale end-to-end), not the default
  response to one closed item.
- **Open questions**: an important architectural or design question is discovered through real
  evidence, is not the current priority, and is expected to remain relevant across sessions —
  losing it (e.g. to a memory wipe) would genuinely hurt. Litmus test: would a future session need
  this from the repo alone, with no memory system at all? If yes, it belongs here, not only in
  personal memory. Update an existing entry (don't create a duplicate) when new evidence changes
  its status, evidence, or exit criteria. Distinguish from the other five types: it graduates to
  `docs/design/` once work actually starts (mark `status: active`, link the new design doc — the
  entry stays as the stable placeholder, it isn't replaced); it graduates to a principle or
  narrative once resolved with a generalizable lesson (the entry's `Resolution` section links out
  rather than restating); it stays out of `reports/` entirely, since a report chronicles one
  story's session history and an open question is a cross-cutting architectural gap, not a story.
  Not project-management ceremony — no priority field, no reordering ritual; just preservation.

**This is a habit to apply proactively whenever a natural trigger fires** — not a step that
requires the user to ask each time, and not a scheduled job either. Skip it honestly when no
trigger has actually fired; forcing an artifact out of routine work defeats the purpose (see "Blog
Post Candidates" and "Engineering Principles" evidence bars below).

**Why this shape, not a bigger pipeline:** an earlier proposal considered automating draft
generation and publishing end to end. Deliberately scoped down to draft-only, human-gated
(publish/discard stays a human call), because the value being tested is whether the raw material
is actually worth writing up — building a publishing pipeline before that's established would be
solving a problem that might not exist. Revisit only after the draft-only step has produced
consistently useful material over real time, not as the default next step.
