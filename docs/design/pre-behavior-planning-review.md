# Purpose

This document is a purely descriptive architectural map of everything Canopy does **before**
behavior-based planning begins — from the first artifact a user creates through the moment the
first `Behavior` struct is persisted to disk. It contains no proposals, no critique, and no
recommendations. Its only job is to let a technically strong third-party engineer, who has never
seen Canopy before, understand exactly what happens today: which decisions are made, which
recommendations are made, where model judgment sits, where deterministic logic sits, where a human
reviews or approves something, and where two runs could plausibly diverge.

This document exists to ground `docs/open-questions/pre-behavior-planning-review.md` — a deferred
investigation into service-discovery/technology-recommendation reproducibility — in a factual
baseline. This document does not answer that investigation's questions (e.g. "is stack
recommendation actually reproducible?"); it only describes the mechanism precisely enough that a
future investigation can be scoped without re-reading the source from scratch.

Every claim below is grounded in the current code, current prompts, and current CLI commands, cited
by file and line number where practical. Nothing here describes an intended future state.

# Pipeline Overview

The pre-behavior pipeline, in the order a user actually runs it:

1. **`canopy init`** — one freeform question, a short architecture/deployment wizard, and two
   LLM-suggested (human-curated) bootstrap lists. Produces `idea.yaml`, zero or more
   `decisions/adr-*.yaml`, `domain_registry.yaml`, `roles.yaml`.
2. **`canopy intent "<statement>"`** — repeatable, once per behavioral statement. An LLM call
   decomposes the statement into candidate user stories; a human accepts/edits/rejects each one;
   accepted stories drive an automatic roles-registry update and a second LLM call that extracts
   domain entities/events. Produces/updates `stories.yaml`, `roles.yaml`, `domain_registry.yaml`.
3. **`canopy spec <story-id>`** — requires the story to have `status: accepted`. One LLM call
   proposes a batch of architecture decisions (service ownership, UI, tech stack, infrastructure);
   a human accepts, edits, or rejects each one, one at a time. Once every proposal is resolved, two
   more LLM calls generate BDD scenarios (grounded in the now-final architecture) and an OpenAPI
   spec. Produces/updates `decisions/adr-*.yaml`, `services.yaml`, `stories/<id>/spec.yaml`,
   `stories/<id>/openapi.yaml`.
4. **`canopy behaviors <story-id>`** — requires the story's `spec.yaml` to exist. Runs a Stage 0
   completeness check (one LLM call) against the spec's own scenarios/entity schema/open
   questions; if it finds a blocking gap, execution stops here. Otherwise, one yes/no human
   confirmation gates entry into behavior extraction. Produces `stories/<id>/completeness.yaml`,
   then — on confirmation — `stories/<id>/behaviors.yaml` and `behavior-coverage.yaml`, the first
   of which holds the very first `Behavior`.

Everything from step 4 onward (Stage 1 behavior extraction proper, decision points, clustering,
contracts) is out of scope for this document — it begins exactly where this document ends.

# Entry Point

`canopy init` (`canopy-cli/src/commands/init.rs`) asks exactly one free-text question: **"What are
you building?"** (init.rs:18). The answer is stored as `Idea { description }`
(`canopy-core/src/lib.rs:13-15`) and saved to `.canopy/idea.yaml` via `save_idea`
(`canopy-storage/src/lib.rs:65`).

**Project name** is never asked — it is computed mechanically by `project_name()`
(`canopy-cli/src/util.rs:77-96`): shell out to `git remote get-url origin`, strip `.git`, take the
final `/`-segment; on any failure, fall back to the current directory's folder name, then to the
literal string `"project"`. This value is only printed to the console (init.rs:118) — it is not
persisted to any file.

*Note on documentation accuracy:* CLAUDE.md's own artifacts table lists `vision.yaml` as a project
artifact, while a separate line in the same file states "no vision generated." A repo-wide search
across `canopy-cli`, `canopy-core`, `canopy-storage`, and `canopy-llm` finds no code path that
writes a `vision.yaml` file anywhere. As implemented today, no vision document is ever generated or
persisted.

After the idea prompt, `init` also runs:
- A short wizard of `dialoguer::Select` choices covering architecture style and deployment style,
  plus — conditionally, only if the architecture-style choice contains the word
  "event-driven" — an event broker choice and a topic-naming-convention choice. Each resulting
  choice is saved as its own ADR via `save_adr(n, slug, adr)` to
  `.canopy/decisions/adr-00N-slug.yaml` (init.rs:24-64). (This pass did not confirm whether the
  wizard's own selectable options are hard-coded in `init.rs` or generated some other way — only
  that the human's choice among presented options is what gets persisted.)
- Two LLM calls, `suggest_domain_entities` and `suggest_roles`
  (`canopy-llm/src/prompts/intent.rs:175-226`), each producing a JSON array of suggestions. Each
  list is shown via a `dialoguer::MultiSelect` with every item pre-checked (`bootstrap_select`,
  `canopy-cli/src/ui.rs:37-44`) — the human can uncheck any suggestion before it's kept, and may
  add a description. Kept items are saved as `DomainRegistry`/`RolesRegistry` to
  `domain_registry.yaml`/`roles.yaml` (init.rs:71-116).

So: domain entities and roles at `init` time are **LLM-generated and human-curated** via an
opt-out multi-select, even though no separate narrative "vision" document exists.

## `canopy intent "<statement>"`

(`canopy-cli/src/commands/intent.rs`)

1. The behavioral statement comes from the CLI argument, or is prompted for as "Behavioral intent"
   if omitted (intent.rs:15-18).
2. Context fed to the LLM is `idea.yaml`'s `description` (intent.rs:20-22; falls back to "No
   context available." if `idea.yaml` doesn't exist).
3. LLM call `generate_stories_from_intent` (intent.rs:29-31), prompt built in
   `canopy-llm/src/prompts/intent.rs:5-80`. The prompt asks the model to decompose the intent into
   the "minimal set of user stories," each with `id, as_a, want, so_that, depends_on, status`,
   with explicit rules on ID prefixing, avoiding CRUD verbs, avoiding generic domain nouns, one
   action per story, and dependency reasoning. New stories are always emitted with `status: draft`
   (prompts/intent.rs:64).
4. Any story whose `id` already exists is filtered out before review (intent.rs:34-39).
5. **Human gate, per new story**: prints the `as_a`/`want`/`so_that` fields, then a
   `dialoguer::Select` — "Accept this story?" — with options `["Accept", "Accept with edit",
   "Reject"]`, default index 0 = Accept (intent.rs:58, via `select_required`,
   `canopy-cli/src/ui.rs:13-15`). "Accept with edit" re-prompts `want`/`so_that` pre-filled with
   the current text via `input_text_with_initial` (intent.rs:66-69). The story's `status` is then
   set to `Accepted` or `Rejected` accordingly (intent.rs:62-76).
6. All curated stories — accepted **and** rejected — are appended to the existing story set and
   the full `UserStories` list is rewritten to `stories.yaml` (intent.rs:82-88); rejected stories
   are kept on disk, just tagged.
7. **Roles update, accepted stories only**: for each accepted story, if its `as_a` value doesn't
   already match a known role (case-insensitive), a new `Role::Simple` is appended and the whole
   registry rewritten to `roles.yaml` (intent.rs:90-98). **No human gate at this specific step.**
8. **Domain extraction, accepted stories only**: a second, separate LLM call,
   `extract_domain_from_stories` (intent.rs:107; prompt `domain_extraction_prompt`,
   `canopy-llm/src/prompts/intent.rs:97-152`), asks for `entities:`/`events:` lists using DDD
   naming rules (PascalCase entities; past-tense events named `{Entity}Created/Updated` or a named
   business operation). New, non-duplicate items are appended and the whole registry rewritten to
   `domain_registry.yaml` (intent.rs:109-125). **No human gate at this specific step either** —
   unlike `init`'s bootstrap suggestions, which go through a MultiSelect, `intent`'s domain
   extraction is folded straight into the registry.

`intent` is designed to be run once per behavioral statement; each run loads the existing
`stories.yaml`/`roles.yaml`/`domain_registry.yaml`, merges new items in memory, and rewrites the
full file — cumulative in effect, not a literal file append.

**`canopy stories`** and **`canopy domain`** (`canopy-cli/src/commands/stories.rs`,
`canopy-cli/src/commands/domain.rs`) are both read-only display commands. Neither offers in-tool
editing; each explicitly tells the user to hand-edit the underlying YAML file directly
(stories.rs:34, domain.rs:29).

**Story status gating**: `StoryStatus` is `Draft` (default) / `Accepted` / `Rejected`
(`canopy-core/src/lib.rs:653-658`). `canopy spec` hard-gates on this: if
`story.status != StoryStatus::Accepted`, it errors immediately and does nothing else
(`canopy-cli/src/commands/spec.rs:85-91`).

# Service Discovery

Service discovery has no dedicated step or command — it happens inside `canopy spec`'s first LLM
call, `identify_architectural_questions` (`canopy-cli/src/commands/spec.rs:102`), which in a
single prompt covers all four proposal categories at once (structural, UI, tech stack,
infrastructure — see `canopy-llm/src/prompts/spec.rs:127-256`). Whether a story's entity belongs to
an already-known service, or needs a brand-new one, is decided by the same prompt category
("Structural questions") that also handles data ownership, event design, and API-boundary
questions.

**What the model can see at this point** (`canopy-llm/src/prompts/spec.rs:132-181`):
- `Existing Architecture Decisions` — every prior ADR in the project, title + decision text only.
- `Known Services and Responsibilities` — every entry already in `services.yaml`, each shown with
  its `[technology]` (if decided) and joined responsibilities.
- A mechanically pre-computed `Domain Event Status` line stating whether a domain-event ADR for
  this story's entity+operation already exists.

This means the model sees other stories' **already-decided** services and technologies — the spec
pass for one story is not isolated from the rest of the project's accumulated decisions.

**Explicit skip instruction** in the prompt: "Service ownership: skip if the specific service that
should own THIS story's domain is already in Known Services" (prompts/spec.rs:184). The prompt
also states naming rules verbatim: service/frontend/infrastructure names are kebab-case only, never
suffixed with "Service", "DB", or "Database" (prompts/spec.rs:212-222).

**Persistence.** `services.yaml` holds a `ServicesRegistry { services: Vec<ServiceEntry> }`
(`canopy-core/src/lib.rs:684-700}`); `ServiceEntry` has `name`, `responsibilities: Vec<String>`,
`technology: Option<String>`, `component_type: Option<String>` (`"frontend" | "service" |
"infrastructure"`). It is updated mechanically by `ServicesRegistry::apply_adr_proposal`
(`canopy-core/src/adr_merge.rs:6-96`) once a proposal is accepted or modified — this function does
not itself choose anything; it writes what the (possibly human-edited) proposal said, and will not
overwrite an already-decided `technology` unless the incoming proposal supplies one explicitly
(adr_merge.rs:77-84). An infrastructure entry's `name` is derived mechanically from the first word
of its technology, lowercased — not from any service name (adr_merge.rs:13-17).

# Technology Recommendation

Technology recommendation happens inside the same `identify_architectural_questions` call as
service discovery — category 3 of the prompt ("Tech stack questions — for every new backend
service introduced... MANDATORY, never omit," prompts/spec.rs:201-204) for backend services, and
category 2 for a new frontend. There is no separate "recommend a stack" command or prompt.

**Where this happens:** `canopy-llm/src/prompts/spec.rs:127-256` (the single architectural
questions prompt); the recommendation is carried as a `ProposedAdr` and, once accepted/modified,
written into `ServiceEntry.technology` by `apply_adr_proposal` (adr_merge.rs:6-96).

**Information available at recommendation time:** the same `Known Services and Responsibilities`
block described above — i.e. already-decided technologies for other services in the same project
— plus the domain registry and all prior ADRs (all three are passed into this call,
prompts/spec.rs:102). There is no explicit "is this really a new kind of service, or should it
reuse an existing stack" branch in the prompt; the model is simply told to skip the tech-stack
question if the target service already has a decided technology (prompts/spec.rs:187).

**Information not yet available at recommendation time:** nothing from `stories/<id>/spec.yaml`
(entity schema, BDD scenarios) — that file doesn't exist yet; it's generated later in the same
`spec` run, after every ADR (including this one) is resolved. Nothing from `openapi.yaml` either,
for the same reason.

**What records the recommendation:** the exact prompt text says "Suggest the most pragmatic and
common choice, but a human will decide before accepting" (prompts/spec.rs:203-204) — i.e. it is
explicitly framed to the model as a recommendation, not a final decision. The human-facing gate is
described fully in "Review And Approval Flow" below. The artifact that records the final decision
is `ServiceEntry.technology` in `services.yaml`, alongside the ADR itself in
`decisions/adr-*.yaml`.

**Fallback path.** After the proposal loop finishes, any service or frontend still missing a
`technology` value is handled separately: the human is asked to type one directly
(`input_text_required`, spec.rs:225), and a synthetic ADR titled `"Tech stack for {name}"` is saved
recording that manual choice (spec.rs:235-244). This is a distinct path from the LLM-proposed one
above — it only fires when the proposal loop didn't produce (or the human rejected) a tech-stack
proposal for a service that still needs one.

This section describes the mechanism only; it does not evaluate whether the recommendation is
good, consistent, or reproducible.

# Review And Approval Flow

Every point, in pipeline order, where a human can review, edit, approve, reject, or redirect
something before behavior extraction begins:

| # | Stage | Mechanism | Options / default | What it governs |
|---|---|---|---|---|
| 1 | `init` | `dialoguer::Select` wizard (2–4 selects) | Presented list; human picks one per select | Architecture style, deployment style, and — conditionally — event broker and topic-naming convention |
| 2 | `init` | `dialoguer::MultiSelect`, all pre-checked (`bootstrap_select`) | Uncheck to exclude | Which LLM-suggested domain entities are kept in `domain_registry.yaml` |
| 3 | `init` | `dialoguer::MultiSelect`, all pre-checked | Uncheck to exclude | Which LLM-suggested roles are kept in `roles.yaml` |
| 4 | `intent` | `dialoguer::Select` (`select_required`), per new story | `["Accept", "Accept with edit", "Reject"]`, default 0 = Accept | Whether a story becomes `Accepted`/`Rejected`; "Accept with edit" lets the human directly rewrite `want`/`so_that` |
| 5 | `intent` | *(none)* | — | Roles-registry update for accepted stories is automatic, no gate |
| 6 | `intent` | *(none)* | — | Domain-registry update (entity/event extraction) for accepted stories is automatic, no gate |
| 7 | `spec` | `dialoguer::Select` (`select_required`), per proposed ADR | `["Accept", "Modify decision text", "Reject"]`, default 0 = Accept | Whether an architecture/UI/tech-stack/infrastructure proposal is recorded as-is, edited, or discarded |
| 8 | `spec` | `dialoguer::Input` (`input_text_with_initial`), only reached via "Modify" | Pre-filled with the proposal's current decision text | Direct human edit of the decision text — **not** a request to the model to regenerate |
| 9 | `spec` | `dialoguer::Input` (`input_text_with_initial`), only if the modified proposal named a service | Pre-filled with current service name | Renaming a service; propagated to later proposals in the same batch that reference the old name |
| 10 | `spec` | `dialoguer::Input` (`input_text_required`) fallback | Required text | Manual technology entry for any service/frontend the proposal loop left without one |
| 11 | `behaviors` | `confirm_default` (yes/no) | Default not confirmed in this pass; a single confirm, not a list | Whether to proceed into behavior extraction, shown only if Stage 0 found no blocking gap |
| — | `stories` / `domain` | Manual file editing (out-of-band) | — | Both commands are read-only and explicitly tell the user to hand-edit `stories.yaml` / `domain_registry.yaml` directly |

Row 11's early-exit is not itself a human review — if Stage 0's own completeness check finds a
**blocking** gap, execution stops before this confirm is ever shown (`canopy-cli/src/commands/
behaviors.rs:76-82`).

# Artifact Inventory

Every artifact that can exist on disk before the first behavior is persisted, in the order it
first appears:

| Artifact | Purpose | Author | Persisted at | Downstream consumers |
|---|---|---|---|---|
| `idea.yaml` | Raw project description | Human (typed) | `init` | `intent`'s context block |
| `decisions/adr-00N-slug.yaml` (init wizard) | Architecture style, deployment style, event broker, topic convention | Human (selected from presented options) | `init` | `spec`'s "Existing Architecture Decisions" context; later domain-event behavior extraction (topic-convention ADR specifically) |
| `domain_registry.yaml` | Known domain entities/events | Model-suggested (`init` bootstrap, human-curated via MultiSelect) + model-extracted (`intent`, no gate) | `init`, updated by `intent` | `spec`'s architectural-questions context (services/tech recommendation) |
| `roles.yaml` | Known user roles | Model-suggested (`init` bootstrap, human-curated) + auto-derived from accepted stories (`intent`, no gate) | `init`, updated by `intent` | Not traced further in this pass |
| `stories.yaml` | User stories with status | Model-generated (`generate_stories_from_intent`), human-gated per story | `intent` | `spec`'s hard status gate; `behaviors`' hard status gate |
| `decisions/adr-*.yaml` (spec proposals) | Structural/UI/tech-stack/infrastructure decisions | Model-proposed, human accept/modify/reject | `spec` | `spec`'s own later scenario/OpenAPI calls (via `existing_adrs`); `behaviors`' Stage 0 completeness prompt; later domain-event behavior extraction |
| `services.yaml` | Known services, their technology and responsibilities | Model-proposed (tech/ownership), human-gated; mechanically written by `apply_adr_proposal` | `spec` | `spec`'s own context for later stories; `scaffold` (skips `component_type == infrastructure`) |
| `stories/<id>/spec.yaml` | Entity schema, BDD scenarios, open questions | Model-generated (`entity_schema_prompt` then `scenario_generation_prompt`/fallback); generated automatically once all ADRs are resolved, no separate human gate observed at this step | `spec`, after its ADR loop | `spec`'s own OpenAPI call; `behaviors`' Stage 0 completeness prompt; behavior extraction itself (mechanical validation/construction behaviors derive directly from `entity_schema`) |
| `stories/<id>/openapi.yaml` | API shape for the story | Model-generated, automatic after `spec.yaml` | `spec` | Not consumed by anything in the pre-behavior pipeline itself; downstream of this document's scope |
| `stories/<id>/completeness.yaml` | Stage 0 gap-check result | Model-generated (`identify_specification_gaps`) | `behaviors`, before the human confirm | Gates whether the human confirm (and thus behavior extraction) is even reached |

The first artifact that belongs to the *next* stage — `stories/<id>/behaviors.yaml` (together with
`behavior-coverage.yaml`, written by the same call) — marks the end of this document's scope; see
"End State Before Behavior Extraction."

# Decision Classification

For each significant output above, classified using the six categories requested. Definitions used
consistently below:
- **Mechanical Fact** — computed deterministically by code; no model call, no human choice.
- **Model Discovery** — an LLM call surfaces candidates not verbatim-specified by a human.
- **Recommendation** — a model-proposed answer explicitly framed as needing human approval before
  it takes effect.
- **Explicit Decision** — the human's approve/edit action itself, and the artifact it produces.
- **Human Decision** — content that originates entirely from typed human input, not from curating
  a model's output.
- **Implicit Decision** — an outcome that ships without any dedicated review step, by the
  pipeline's own structure rather than an oversight being asserted here.

| Output | Classification | Why |
|---|---|---|
| Project name | Mechanical Fact | Derived from `git remote get-url origin` or the folder name; no model call, no human prompt (`util.rs:77-96`) |
| `idea.yaml` description | Human Decision | Freeform typed answer to "What are you building?" |
| Init wizard choices (architecture/deployment/broker/topic style) | Explicit Decision | Human selects one option per `Select`; recorded verbatim as an ADR |
| Bootstrap domain entities/roles (pre-MultiSelect) | Model Discovery | LLM-suggested list, not asked for by name |
| Bootstrap domain entities/roles (post-MultiSelect, as saved) | Explicit Decision | The kept subset is a human-curated (opt-out) result |
| Candidate user stories (pre-review) | Model Discovery | LLM decomposition of the intent statement |
| Story `status` (Accepted/Rejected) | Explicit Decision | Direct result of the per-story `Select` gate |
| "Accept with edit" story text | Human Decision | Direct human rewrite, not model output |
| `intent`'s roles-registry update | Implicit Decision | New role rows are added automatically for accepted stories, with no dedicated review step for that specific addition |
| `intent`'s domain-registry update | Implicit Decision | Same — a separate LLM call's output is merged automatically, unlike `init`'s bootstrap equivalent |
| Architecture/UI/infrastructure ADR proposals (pre-review) | Recommendation | Explicitly framed in-prompt as proposals a human will accept/modify/reject |
| Technology-stack proposals (pre-review) | Recommendation | Same mechanism; prompt states "a human will decide before accepting" verbatim |
| Accepted/modified ADRs (as saved) | Explicit Decision | Direct result of the per-proposal `Select` gate, possibly with a human-edited decision text |
| Fallback manual technology entry | Human Decision | Directly typed by the human, not model-suggested at all |
| `ServiceEntry.technology`/`component_type` (final, saved) | Explicit Decision | Mechanically written by `apply_adr_proposal`, but its *content* is whatever the just-resolved Explicit Decision (ADR) specified |
| Entity schema / BDD scenarios (`spec.yaml`) | Model Discovery | LLM-generated from the story + now-resolved architecture; no per-item human gate observed at this step |
| OpenAPI spec | Model Discovery | LLM-generated from `spec.yaml` + services/ADRs; no human gate observed |
| Stage 0 completeness result (gaps found or not) | Model Discovery | LLM call (`identify_specification_gaps`) checking the spec against itself |
| Decision to proceed into behavior extraction | Explicit Decision | The single yes/no confirm gate, reached only if Stage 0 found no blocking gap |

# Sources Of Variability

Every point in this pipeline where two runs, given the same starting input, could plausibly
produce different output — identified, not judged:

- **Every LLM call in this pipeline is a variability source structurally**: `call_anthropic`
  (`canopy-llm/src/client.rs:155-160`) sends only `model`, `max_tokens`, and `messages` — no
  `temperature` field and no visible seed/sampling control anywhere in `LlmClient`. This applies
  uniformly to every model call listed below.
- **Story decomposition** (`generate_stories_from_intent`) — the same intent statement could
  decompose into a different set or wording of candidate stories across runs.
- **Bootstrap suggestions** (`suggest_domain_entities`, `suggest_roles`) — the suggested list
  itself (before human curation) could vary.
- **Domain extraction from accepted stories** (`extract_domain_from_stories`) — a separate LLM
  call from bootstrap suggestion; its output could vary independently.
- **Architectural questions / ADR proposals** (`identify_architectural_questions`) — which
  questions get raised at all, their exact wording, and specifically which service or technology
  gets proposed, can vary run to run for the same story. This is the specific behavior named in
  `docs/open-questions/pre-behavior-planning-review.md`.
- **Order-dependent prompt content**: `existing_adrs` and `services.services` are rendered in
  their stored `Vec` order (`prompts/spec.rs:136-140, 145-154`) — the order they were appended in
  `decisions/`/`services.yaml`, i.e. the order stories happened to be processed in, not a sorted
  order. Processing stories in a different sequence changes what context appears, and in what
  order, inside a later story's `spec` prompt — a structural variability source independent of
  model sampling.
- **Entity schema and BDD scenario generation** (`entity_schema_prompt`,
  `scenario_generation_prompt`/`fallback_scenario_prompt`) — LLM-generated, could vary.
- **OpenAPI generation** (`generate_story_openapi`) — LLM-generated, could vary.
- **Stage 0 completeness check** (`identify_specification_gaps`) — the gap list itself could vary
  between runs on identical input, which has a structural downstream effect: whether a "blocking"
  gap is found at all determines whether behavior extraction proceeds or halts for that run.
- **Human review outcomes** are themselves a source of run-to-run difference in the literal sense
  the pipeline permits — the same proposal shown to two different reviewers (or the same reviewer
  at two different times) can be Accepted, Modified differently, or Rejected. This is a property of
  where the human gates sit, not of the model.
- **No clustering-like operation exists in this part of the pipeline.** Clustering (grouping
  behaviors by subject/kind) is part of the behavior-first pipeline's own later stage, entirely
  outside this document's scope — there is nothing clustering-shaped to report as a variability
  source before behavior extraction begins.

# End State Before Behavior Extraction

By the moment the first `Behavior` is persisted, the following can exist on disk for a story (see
"Artifact Inventory" for the full table): `idea.yaml`, one or more `decisions/adr-*.yaml`,
`domain_registry.yaml`, `roles.yaml`, `stories.yaml`, `services.yaml`, `stories/<id>/spec.yaml`,
`stories/<id>/openapi.yaml`, and `stories/<id>/completeness.yaml`. No `vision.yaml` exists under
any code path found in this pass.

**Neither service discovery nor technology recommendation is re-examined at this stage.** Grepping
`canopy-cli/src/commands/behaviors.rs`, `canopy-llm/src/prompts/behaviors.rs`, and the
`extract_behaviors` function itself finds zero references to `ServicesRegistry`, `DomainRegistry`,
`load_services_registry`, or `load_domain_registry` — everything about services and technology was
decided during `spec` and is not revisited here.

**The first `Behavior` constructed in the whole pipeline is mechanical, not model-generated.**
Inside `extract_behaviors` (`canopy-llm/src/prompts/behaviors.rs:581-627`), when
`spec.entity_schema` is present, three mechanical functions run in this fixed order before any LLM
call is made:
1. `mechanical_validation_behaviors` — one behavior per (field, constraint) pair defined in the
   entity schema.
2. `mechanical_construction_behaviors` — one behavior per system-generated field.
3. `mechanical_event_behaviors` — for each ADR whose `decision` text parses as `"<EventName> on
   topic <topic>"` (via `parse_event_adr`) and whose event name is prefixed with the entity name:
   `eventId`/`occurredAt`/`<entity>Id` shape behaviors, plus one publication behavior.

Only after every mechanical behavior above has been appended does the pipeline make its one bounded
LLM call (`scenario_behavior_prompt`) to produce scenario-derived behaviors — restricted by design
to `persistence | orchestration | http-request | http-response | error-translation` kinds; it
cannot produce a validation, construction, event-shape, or publication behavior.

The moment `save_behaviors` writes `stories/<id>/behaviors.yaml` and `behavior-coverage.yaml`
(`canopy-storage/src/lib.rs:173-176`) is the literal boundary this document describes up to — the
first artifact of behavior-based planning.

# Glossary

- **ADR (Architecture Decision Record)** — a recorded decision (`title, decision, reason,
  alternatives`), saved to `decisions/adr-NNN-slug.yaml`, whether it originated from `init`'s
  wizard or `spec`'s proposal loop.
- **`Behavior`** — a single, atomic, testable statement about the system, tagged with a `kind`
  (e.g. Validation, Construction, EventShape, Publication, Persistence, Orchestration,
  HttpRequest, HttpResponse, ErrorTranslation). The unit this document's scope ends at.
- **Bootstrap suggestions** — the LLM-suggested domain entities/roles offered during `init`,
  shown via an opt-out `MultiSelect` before being saved.
- **`DomainRegistry`** — the accumulated list of known domain entities and events
  (`domain_registry.yaml`), fed by both `init`'s bootstrap and `intent`'s per-story extraction.
- **Entity schema** — the field-level shape of a story's core entity (name, constraints, which
  fields are system-generated), stored in `stories/<id>/spec.yaml`; the direct source of every
  mechanical validation/construction behavior.
- **`ProposedAdr`** — the in-memory, not-yet-persisted shape of a candidate architecture decision,
  produced by `identify_architectural_questions`, before the human review gate resolves it into a
  saved ADR (or discards it).
- **`ServiceEntry` / `ServicesRegistry`** — the per-service record (`name`, `responsibilities`,
  `technology`, `component_type`) and its containing list, persisted to `services.yaml`.
- **Stage 0 (Specification Completeness)** — the gap-check LLM call (`identify_specification_gaps`)
  that runs at the start of `canopy behaviors`, before any behavior is generated; its result is
  `stories/<id>/completeness.yaml`.
- **`StoryStatus`** — `Draft` (default) / `Accepted` / `Rejected`; gates whether `spec` and
  `behaviors` will act on a given story at all.
- **`UserStory` / `UserStories`** — a single behavioral requirement (`id, as_a, want, so_that,
  depends_on, status`) and the full accumulated list, persisted to `stories.yaml`.
