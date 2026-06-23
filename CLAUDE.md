# CLAUDE.md

## Project: Canopy

Canopy is an AI software engineering system.

Canopy is NOT a code completion tool.

Canopy is NOT a chat interface over a repository.

Canopy is a planning and implementation engine that operates on structured repository knowledge produced by Roots.

---

## Core Principles

### 1. Structure Before Tokens

Canopy must prefer structured repository knowledge over raw source code.

Bad:

* Read entire files
* Dump files into LLM context
* Ask model to infer architecture

Good:

* Query Roots
* Obtain context packet
* Operate on symbols, relationships, modules and architecture facts

---

### 2. Intent Before Coding

Every implementation begins with intent and specification.

Required workflow:

Intent (behavioral statement authored by human)
→ Stories (LLM derives; human curates → accepted | rejected)
→ Spec (accepted story → ADR gating → BDD scenarios as acceptance criteria)
→ Implementation (story-scoped)
→ Validation (scenarios as test oracle)

Architecture decisions emerge story by story — never big-bang up front.
Each `canopy spec <story-id>` run proposes ADRs and blocks until resolved.
Accepted ADRs accumulate in `.canopy/decisions/` and feed the services registry.

Direct feature-to-code generation must be avoided.

---

### 3. Graph First

The repository is represented as:

Workspace
→ Project
→ Module
→ File
→ Symbol

The graph is the primary source of truth.

Source code is a projection of the graph.

---

### 4. Minimize Context

Canopy should request the smallest useful context packet.

Prefer:

* facts
* symbols
* relationships
* summaries

Avoid:

* entire files
* repository dumps
* broad retrieval

---

### 5. Generate Diffs, Not Files

Preferred output:

* patches
* edits
* symbol modifications

Avoid regenerating complete files when a targeted change is possible.

---

### 6. Explain Decisions

Every plan should explain:

* why a change is needed
* what is affected
* what risks exist

Reasoning artifacts are first-class outputs.

---

## Modes

### Greenfield Mode

No repository exists.

Greenfield flow:

1. `canopy explore` — captures idea and generates vision only
2. `canopy stories` — generates initial backlog from vision
3. `canopy intent "<statement>"` — adds stories from behavioral requirements (repeat)
4. Edit `.canopy/stories.yaml` — set `status: accepted | rejected`
5. `canopy spec <story-id>` — ADR gating per story; tech stack ADRs decided here
6. `canopy scaffold` — runs after enough ADRs are accepted; reads services registry,
   not an upfront component architecture

Architecture, domain, and tech stack emerge story by story through `canopy spec`.
There is no big-bang upfront architecture phase.

### Repository Mode

Repository exists.

Canopy uses Roots for:

* graph queries
* impact analysis
* context generation

before modification.

---

## Tool Usage

Preferred tools:

1. roots.get_context_packet
2. roots.impact
3. roots.trace
4. roots.graph
5. canopy.plan
6. canopy.generate_patch
7. canopy.validate

Avoid direct filesystem traversal when equivalent Roots tools exist.

---

## Success Criteria

Canopy succeeds when a small model can make large changes because the system provides excellent context.

Model quality is secondary.

Context quality is primary.

