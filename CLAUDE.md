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

### 2. Planning Before Coding

Every implementation begins with planning.

Required workflow:

Feature
→ Impact Analysis
→ Plan
→ Tasks
→ Implementation
→ Validation

Direct feature-to-code generation should be avoided.

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

Canopy creates:

* vision
* architecture
* domain model
* backlog
* scaffold

before code generation.

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

