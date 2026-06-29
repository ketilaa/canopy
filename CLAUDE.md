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
| Vision | `canopy init` |
| User roles | `canopy intent` (from `as_a` fields) |
| Domain entities and events | `canopy intent` (automatic extraction) |
| User stories | `canopy intent` (one behavioral statement at a time) |
| Services and responsibilities | `canopy spec` (ADR proposals) |
| Technology stack per service | `canopy spec` (ADR proposals) |
| Databases and event infrastructure | `canopy spec` (infrastructure ADR proposals) |
| BDD acceptance criteria | `canopy spec` (after ADRs are resolved) |
| Project scaffold | `canopy scaffold` (reads services registry) |

There is no step that generates architecture, domain model, or component structure upfront.

---

## Workflow

```
canopy init
  └─ one question: "What are you building?"
  └─ saves: idea.yaml
  └─ project name derived from git remote or folder name — no vision generated

canopy intent "<behavioral statement>"   (repeat per requirement)
  └─ LLM derives user stories
  └─ human curates: status → accepted | rejected
  └─ auto-extracts: domain entities and events → domain_registry.yaml
  └─ saves: stories.yaml, roles.yaml, domain_registry.yaml

canopy stories      → display backlog
canopy domain       → display accumulated domain vocabulary (edit freely)

canopy spec <story-id>   (story must be accepted)
  └─ LLM proposes ADRs: structural, UI, tech stack, infrastructure
  └─ human gates each: Accept / Modify / Reject
  └─ accepted ADRs → decisions/adr-NNN-slug.yaml
  └─ services and tech stack accumulate → services.yaml
  └─ generates BDD scenarios grounded in resolved architecture → stories/<id>/spec.yaml

canopy scaffold [dir]
  └─ reads services.yaml (skips infrastructure components)
  └─ requires at least one service with a decided technology
  └─ runs scaffold commands: Spring Boot, Angular, React, Node.js, etc.

canopy implement <story-id>
  └─ detects actual package from scaffolded *Application.java (no guessing)
  └─ generates implementation plan: one LLM call per service → merged and sorted
  └─ human confirms plan before execution
  └─ executes step by step, reindexes after each file
  └─ runs test/fix loop per service after all steps complete (up to 5 iterations)
  └─ saves: stories/<id>/plan.yaml, stories/<id>/contract.yaml

canopy validate <story-id>    (future)
```

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

---

## Tech-Stack Skills

`canopy implement` injects a **skill** into each per-service plan prompt based on the service's
technology. A skill is a concise, authoritative rules block that tells the LLM the exact
conventions for that tech stack — package layout, file paths, naming, forbidden patterns.

Skills are defined in `canopy-llm/src/lib.rs`. The matcher is `skill_for_technology(tech, pkg, pkg_path, service_name)`.

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

**Scope discipline.** Each skill explicitly lists what must NOT be added unless a story requires it.
For Spring Boot: no extra Application classes, no sub-package for the entry point.
For React/Vite: no custom hooks, page components, route files, Redux slices, or utility modules
unless the story's acceptance criteria call for them. Architecture emerges story by story.

**Adding a new skill:** add a `const SKILL_*` string (or a function for dynamic context),
add a match arm in `skill_for_technology`, and document it in this table.

---

## Principles

**Intent before coding.** No implementation without an accepted story and a resolved spec.

**Minimise context.** Pass the smallest useful input to the LLM — facts, summaries, symbols.
Never dump entire files. The prompt is the design.

**Generate diffs, not files.** Prefer targeted edits over full file regeneration.

**Explain decisions.** ADRs are first-class outputs. Reasoning is not a comment — it is the record.

**Model quality is secondary. Context quality is primary.**
Canopy succeeds when a small model can make large changes because the system provides excellent context.
