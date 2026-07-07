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

Skills are defined in `canopy-llm/src/lib.rs` as `TechStackSkill` structs with four fields:
`file_layout`, `namespace_rules`, `layer_order`, `notes`.

Each skill has two render modes:

| Method | Used by | Contains |
|---|---|---|
| `render_for_planning()` | `plan_skill_for_technology()` → plan prompt | `file_layout` + `layer_order` only |
| `render()` | `skill_for_technology()` → step prompt | all four fields |

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
