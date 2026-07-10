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
  └─ saves: stories/<id>/plan.yaml, stories/<id>/contract.yaml

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
      contract.yaml                OAS contract snapshot used during implementation
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
`canopy-cli` calls into `roots-context` to get context packets rather than reading files directly.
When Roots is available, prefer `roots-context` over `canopy-storage` for symbol and relationship queries.

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

**Generic placeholders in skill examples.** All code examples in skills and prompts use `Widget` /
`createWidget` as the canonical stand-in — never domain-specific names like `Product` /
`createProduct`. Field names: `name`, `optionalField`, `name-value`, `other-field-value`. The
pattern is established in the Models section of `node_express_skill()` — follow it everywhere.
Domain-specific names in skill examples leak the current project's vocabulary into the LLM context
and cause the model to mirror those names back incorrectly on other projects.

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
- **When a model ignores a correct instruction, fix the prompt, not the code.** Verify with
  `llm-debug.log` that the instruction actually reached the prompt as intended before concluding
  it's a compliance problem rather than a missing-instruction problem. If it's genuinely a
  compliance problem, the fix is to shorten and reposition the instruction — never a Rust-side
  filter, override, or post-processing step to compensate, and not by default adding more prose
  either. A longer WRONG/CORRECT example is a last resort after a short instruction has been
  tried and shown (with real evidence) to still fail — not the first move.

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

**Fix LLM output through prompts, not code.** When the LLM produces wrong paths, missing files,
bad structure, or incorrect patterns — fix the discovery or skill prompt to make the requirement
clearer. Do not add Rust safety nets (path injectors, output filters, post-generation reordering).
The prompt is the design; the model should get it right because the context is good, not because
Rust patches the output. Only reach for code enforcement when the problem is structurally
impossible to express in a prompt (e.g. numbering step IDs after a merge across services).
See Prompt House Style above for how to fix a prompt the model isn't complying with.

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
detects this via `test_file_passed_cleanly` (`canopy-cli/src/build_output.rs`) and accepts it
immediately rather than treating the PASS as "an error to fix" — Green phase regenerates the impl
file from scratch regardless, so nothing is lost. Before this fix, a PASS fell through to the
generic "fix the test file" loop with the literal string `PASS <file>` as its "errors," and with
nothing real to fix, the model invented unrelated changes across attempts until it drifted the
test into a completely different, unrelated shape. If a fix loop's output looks like it hallucinated
an unrelated domain, check whether the input it was fed was ever a real error in the first place.

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
