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
| Vision | `canopy explore` |
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
canopy explore
  └─ one question: "What are you building?"
  └─ saves: idea.yaml, vision.yaml

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

canopy implement <story-id>   (future)
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
```

---

## Codebase Structure

```
canopy-core/       data types (structs, enums, serde)
canopy-explore/    LLM prompt functions and generation logic
canopy-storage/    save/load wrappers around .canopy/
canopy-cli/        CLI commands (clap), interactive prompts (dialoguer)
```

When adding a new capability: type in core → storage helpers → explore prompt/function → cli command.

---

## Principles

**Intent before coding.** No implementation without an accepted story and a resolved spec.

**Minimise context.** Pass the smallest useful input to the LLM — facts, summaries, symbols.
Never dump entire files. The prompt is the design.

**Generate diffs, not files.** Prefer targeted edits over full file regeneration.

**Explain decisions.** ADRs are first-class outputs. Reasoning is not a comment — it is the record.

**Model quality is secondary. Context quality is primary.**
Canopy succeeds when a small model can make large changes because the system provides excellent context.
