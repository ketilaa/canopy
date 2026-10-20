---
title: What the Planner is given, what only a human can supply, and what the model can do
status: open
origin: the S6a read and the S6c regression (2026-10-20): the failing plans fail on facts absent from the input
date_discovered: 2026-10-20
related_principles: [read-the-prompt-for-contradictions-first.md]
related_narratives: []
related_reports: []
related_design_docs: [planner-first-increment.md, implementer-first-course.md]
---

# Question

What is the right input for the Planner, which parts can the model contribute, and which need a human, given that Canopy's flow
(`intent`, `spec`) does not produce the class-level API the best Planner results depend on?

# Why It Matters

The layered Planner reaches 20 to 24 of 50 hidden passes with a hand-written `api.txt` (types, signatures, exception types,
responsibilities) beside a Gherkin feature. Canopy's `spec` produces services, ADRs, BDD and OpenAPI, not that. Until the Planner is
scored on an input the product can produce, its numbers do not say what the product will do. And the failures that remain are
plan cases whose expected values the model invented.

# Evidence So Far

- Tickets, gaps in the feature that a team of humans would have to ask about: (1) whether 90 minutes is inclusive (the hidden test
  requires 11:30 valid; the feature has 10:30 and 11:31 only), (2) what `validate` changes (S6a run 8 overwrote `issuedAt`),
  (3) which rejection wins when several apply, (4) cancel edge cases (run 8 invented "Ticket is already cancelled"),
  (5) persistence on success and none on failure, (6) time is always passed in (run 4 used `now()`), (7) non-goals (duplicate issue,
  null or blank ids). The last feature scenario also mixes validate and cancel for an unknown ticket.
- S6a: 3 of 5 failing plans carry a self-contradicting or nondeterministic case; no existing check sees it ("dropped cases: 0").
- Order 7 to 9, shipping 3 to 5, tickets and stock 10 of 10 on hand plans; loans 0 of 10 on hand plans (an Implementer limit).
- Shipping rose to 10 of 10 with a value for every named case and a test sentence that did not contradict the rule (B1 to B6).

# Hypothesis and designed experiment (E1, not run)

A human-written, executable example table (a transition table for `Ticket`; concrete rows per unit; decisions and non-goals for the
seven gaps), checked by a reference implementation plus six mutants, is the artifact that carries what only a human knows.
Arms, n=10 each, hidden passes: A2 the plan rendered from the table and `api.txt` without a model (prediction at least 8; falsified at 6 or fewer);
A1 the layered Planner taking cases and exceptions verbatim from the table, the model writing rules prose (at least 7; falsified at 5 or fewer);
A3 the A2 table without the rows for gaps 1 to 7 (at least 2 below A2). References: S6a 5, the hand plan 10.
Caveat: the maintainer who wrote the gap list had read the hidden test, so E1 tests whether a table that answers the right questions
helps, not whether a team would ask them. E2 (cheap): give the model the feature and `api.txt`, ask for the ambiguities, score its
recall against the seven gaps above.

# Exit Criteria

Resolved when the Planner has been scored on an input the Canopy flow can produce (with a stated human step), and the split between
model, harness and human is measured, not argued.
