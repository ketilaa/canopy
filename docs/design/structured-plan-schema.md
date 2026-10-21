# A structured plan schema for the Planner/Implementer, designed clean of the existing REPL pipeline

Status: proposal, 2026-10-21. Nothing here is built. Written at the maintainer's request, deliberately
without the constraint that the plan's input must come from `intent` → `spec` → `services.yaml` (the
REPL pipeline in `CLAUDE.md`'s Core Design Insight table). That pipeline produces services, ADRs, BDD
scenarios and OpenAPI — not a class-level API, not ownership, not executable examples. The schema below
is designed from what the S0–S6c experiments showed actually causes plans to fail, independent of where
the input eventually comes from. How this input would be produced (by `spec`, by a new stage, or by a
human) is explicitly out of scope here; see `docs/open-questions/planner-input-human-facts.md`.

Read first: the S6a read and the S6c regression (`canopy-implementer/docs/prompt-and-plan-changelog.md`,
S6a/S6c/A1 entries) and `docs/retrospectives/2026-10-20.md`. Every design choice below is a direct
response to a named failure in that record, not a general best practice.

## Problem

Five runs of evidence point at the same shape of failure, and none of them is "the model wrote bad
code":

1. **Contradicting checklist items in one plan.** S6a runs 3 and 5: the same setup (`save(T-1)`) appears
   in two checklist items with expected results `Optional.empty()` and `Optional.of(...)`. Both must be
   tested (the prompt says so), so the plan is unsatisfiable, and no retry can fix it — the retry shows
   the failure, but the instruction that caused it is still there.
2. **A fact the model has to invent.** S6a run 4 uses `LocalDateTime.now()` in two places and expects
   them to agree. S6a run 8 invents that `validate` changes `issuedAt`. Neither the feature nor any
   other part of the plan says what should happen, so the model picks something, and it stops a later
   step.
3. **An exception rendered into the wrong unit.** S6c: the `exceptions` field lists every exception the
   *story* has, and the harness's Throws block shows all of them to every unit, including `Loan` (which
   the plan's own Rules prose says has none). The leak is visible in the plan file itself.
4. **A rule stated in two places that can drift.** The tickets service plan's Rules prose says "the
   service performs the validation and cancellation logic," while `Ticket`'s own Rules carry the same
   logic (correctly, since `Ticket` is read-only to the service step). Nothing in the plan format says
   which one is authoritative.
5. **Noise large enough to erase the signal.** A1 (2026-10-20): re-running the same 30 plans moved the
   measured S4-to-S6c gap from 9 to 3 — most of three stories' "regression" was Implementer sampling, not
   the Planner change. Any schema redesign has to be evaluated with more than one Implementer run per
   plan, or its own effect will be indistinguishable from this noise.

Each of 1–4 is representable in the current `plan.yaml` shape (free-text Rules prose, a checklist of
numbered sentences, a `exceptions` array keyed by method name only). The schema below tries to make each
one either unrepresentable or mechanically checkable, per the escalation order in `CLAUDE.md`
("make the wrong answer unrepresentable" before "teach in the prompt").

## Design principles

1. **Every field carries provenance** (`human` / `model` / `computed`), so "which part was hand-written
   and which was model-derived" is a schema fact, not something a session has to reconstruct from a
   changelog after the fact — which is exactly what the S6c diagnosis had to do.
2. **Ownership is a field, not prose.** `owns_rules` plus `delegates_to` make "two units claim the same
   rule" a checkable condition instead of a read.
3. **Examples are structured triples (`setup`, `call`, `expect`), not sentences.** A sentence like "free
   shipping at and just below 100.00" is ambiguous on its face; a triple is diffable, and "two triples,
   same `setup`, different `expect`" is a one-line audit.
4. **Exceptions declare their owning unit.** A renderer that only shows a unit its own entries makes the
   S6c leak structurally impossible, not just caught after the run.
5. **Invariants are explicit** (`preserves: [id, issuedAt]`). "A new ticket is returned" does not by
   itself say what stays the same.
6. **Non-goals are a field.** Every invented behavior in the record (a bogus exception message, a changed
   field, a Mockito-based test where none was asked for) filled a silence the plan left open.
7. **Facts and decisions are separate from rules.** A "decision" is a question only a human could answer
   (is the boundary inclusive?); a "fact" is computed (decimal scale, classpath). Mixing them into
   undifferentiated prose is why nobody could tell, on sight, which kind of gap a failure traced back to.

## The schema

```yaml
plan:
  id: tickets
  version: 1
  story: >
    A single-journey ticket is issued, can be validated within 90 minutes of being
    issued, and can be cancelled until it is validated. (kept for the record; not shown
    to the implementing model directly — only through the fields below)

  facts:                          # computed, never asked for; a model never writes this section
    language: java
    build: maven
    package_root: no.experis.tickets
    decimal_scale: null          # e.g. 2, with rounding mode, only when the story has money
    classpath_excludes: [mockito]  # computed from the real pom.xml / package.json

  decisions:                      # questions only a human could resolve
    - id: boundary-inclusive
      question: Is the 90-minute validation window inclusive of the boundary instant?
      answer: Yes — exactly 90 minutes after issuedAt is still valid.
      resolved_by: human
    - id: validate-preserves-issued-at
      question: Does validate change issuedAt?
      answer: No — only status changes. id and issuedAt are immutable across validate and cancel.
      resolved_by: human
    - id: rejection-precedence
      question: When cancelled and expired both apply, which rejection wins?
      answer: "Checked in this order: cancelled, already-validated, expired."
      resolved_by: human
    - id: not-in-scope
      question: Are duplicate issue(), null ids, or a system-clock read in scope?
      answer: "No. at is always supplied by the caller; never read the system clock."
      resolved_by: human

  non_goals:
    - "Concurrency or thread safety of the repository."
    - "Any persistence beyond an in-memory HashMap."
    - "Reading the system clock anywhere; all times are passed in."

  units:
    - id: ticket
      kind: model                      # model | factory | repository | service | route | middleware
      file: src/main/java/no/experis/tickets/Ticket.java
      owns_rules: [validation-window, validation-precedence, cancellation-rule]
      provenance: model                 # drafted by the model
      confirmed_by: human                # confirmed by a human before the plan is used
      api:
        type: record
        name: Ticket
        components:
          - {name: id, type: String}
          - {name: issuedAt, type: LocalDateTime}
          - {name: status, type: TicketStatus}
        methods:
          - signature: "Ticket validate(LocalDateTime at)"
            preserves: [id, issuedAt]
            throws:
              - {order: 1, type: IllegalStateException, message: "Ticket is cancelled",
                 condition: "status == CANCELLED"}
              - {order: 2, type: IllegalStateException, message: "Ticket is already validated",
                 condition: "status == VALIDATED"}
              - {order: 3, type: IllegalStateException, message: "Ticket has expired",
                 condition: "at.isAfter(issuedAt.plusMinutes(90))"}
          - signature: "Ticket cancel()"
            preserves: [id, issuedAt]
            throws:
              - {order: 1, type: IllegalStateException, message: "A validated ticket cannot be cancelled",
                 condition: "status == VALIDATED"}
      examples:
        - id: valid-at-boundary
          setup: "new Ticket(\"T-1\", 2026-03-01T10:00, ISSUED)"
          call: "validate(2026-03-01T11:30)"      # exactly 90 minutes — the inclusive boundary
          expect: {status: VALIDATED}
        - id: expired-one-minute-past
          setup: "new Ticket(\"T-1\", 2026-03-01T10:00, ISSUED)"
          call: "validate(2026-03-01T11:31)"
          expect: {throws: IllegalStateException, message: "Ticket has expired"}
        - id: preserves-fields-on-validate
          setup: "new Ticket(\"T-1\", 2026-03-01T10:00, ISSUED)"
          call: "validate(2026-03-01T10:30)"
          expect: {status: VALIDATED, id: "T-1", issuedAt: "2026-03-01T10:00"}

    - id: ticket-repository
      kind: repository
      file: src/main/java/no/experis/tickets/InMemoryTicketRepository.java
      owns_rules: [save-replaces-by-id]
      read_only: [ticket]
      api:
        type: class
        name: InMemoryTicketRepository
        implements: TicketRepository
        methods:
          - signature: "Optional<Ticket> findById(String id)"
          - signature: "void save(Ticket ticket)"
      examples:
        - id: find-missing-is-empty
          setup: "new InMemoryTicketRepository()"
          call: "findById(\"T-9\")"
          expect: {empty: true}
        - id: save-then-find
          setup: "repo.save(new Ticket(\"T-1\", 2026-03-01T10:00, ISSUED))"
          call: "findById(\"T-1\")"
          expect: {present: true, status: ISSUED}

    - id: ticket-service
      kind: service
      file: src/main/java/no/experis/tickets/TicketService.java
      owns_rules: [unknown-ticket-rejection]   # NOT validation/cancellation — Ticket owns those
      depends_on: [ticket, ticket-repository]
      api:
        type: class
        name: TicketService
        constructor: "TicketService(TicketRepository)"
        methods:
          - signature: "Ticket validate(String id, LocalDateTime at)"
            throws:
              - {order: 1, type: IllegalArgumentException, message: "Unknown ticket",
                 condition: "repository.findById(id) is empty"}
            delegates_to: {unit: ticket, method: validate}
      examples:
        - id: unknown-ticket-rejected
          setup: "new TicketService(new InMemoryTicketRepository())"
          call: "validate(\"T-9\", 2026-03-01T10:00)"
          expect: {throws: IllegalArgumentException, message: "Unknown ticket"}

  verify:                              # deterministic audits the orchestrator runs before any model call
    - rule: no-duplicate-setup-conflicting-expect
    - rule: no-wall-clock-calls
    - rule: exceptions-rendered-only-for-owning-unit
    - rule: preserved-fields-not-contradicted-by-example
    - rule: every-identifier-in-an-example-is-defined-in-its-own-setup
    - rule: one-owner-per-rule-id

  steps:                              # derived deterministically from units + depends_on, never model-authored
    - id: tickets-1-ticket
      unit: ticket
      create: [src/main/java/no/experis/tickets/TicketStatus.java,
               src/main/java/no/experis/tickets/Ticket.java]
      covers: [valid-at-boundary, expired-one-minute-past, preserves-fields-on-validate]
    - id: tickets-2-repository
      unit: ticket-repository
      create: [src/main/java/no/experis/tickets/TicketRepository.java,
               src/main/java/no/experis/tickets/InMemoryTicketRepository.java]
      read_only: [src/main/java/no/experis/tickets/Ticket.java]
      covers: [find-missing-is-empty, save-then-find]
    - id: tickets-3-service
      unit: ticket-service
      create: [src/main/java/no/experis/tickets/TicketService.java]
      read_only: [ticket, ticket-repository]
      covers: [unknown-ticket-rejected]
```

## Failure-to-field map

| Failure (evidence) | Field that addresses it |
|---|---|
| S6a 3/5: contradicting checklist items | `examples` as `{setup, call, expect}` triples + `no-duplicate-setup-conflicting-expect` |
| S6a run 4: `now()` in a case | `no-wall-clock-calls`; `facts` never contains wall-clock access |
| S6a run 8: `issuedAt` silently changed | `preserves: [id, issuedAt]` |
| S6c: exceptions leaked into a unit that throws nothing | `throws` lives under its owning unit/method only; `exceptions-rendered-only-for-owning-unit` |
| Tickets rule duplicated on `Ticket` and `TicketService` | `owns_rules` + `delegates_to` + `one-owner-per-rule-id` |
| The 11:30 boundary, unstated in the feature | `decisions` |
| "Which rejection wins" | `throws[].order`, explicit per method |

## What this does not solve

- It makes contradictions and leaks *detectable*. It does not make the model draft correct `examples` or
  `throws` unaided — that is the open question in `docs/open-questions/planner-input-human-facts.md`
  (E1). The `provenance`/`confirmed_by` fields exist so that split can be measured, not argued.
- Conflict detection for `owns_rules` (two units claiming the same `rule_id`) is specified above but not
  designed in implementation terms here.
- Whether `run_step`'s prompt renderer can consume this shape without a rewrite is unknown; see the
  introduction plan.
- `A1` (2026-10-20) showed that a story-level difference of 3 of 10 is within one Implementer run's
  sampling noise. Any claim that this schema improved or hurt a story's pass rate needs the same
  guard the introduction plan below applies: several Implementer runs per plan, not one.

## Relationship to the existing pipeline

Out of scope here, by the maintainer's instruction, is *how* `facts`, `units[].api`, and `decisions` get
produced inside the Canopy REPL (whether `spec` grows a stage, whether a new `design` stage sits between
`spec` and `implement`, or whether this stays a standalone Planner input indefinitely). That question is
tracked in `docs/open-questions/implementer-and-planner-into-the-canopy-repl.md` and
`docs/open-questions/planner-input-human-facts.md`; this document does not resolve it.
