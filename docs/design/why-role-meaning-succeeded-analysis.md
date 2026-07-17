# Why Role Meaning Succeeded Where Persona-Policy Facts Failed

Status: evidence analysis only. No mechanism, workflow, or UX proposed. Role Meaning is treated as
the reference case — a fact type already shown, via the Role Meaning Value Experiment, to survive
the pipeline — and compared directly against the broader persona-policy facts from the Human
Insight Process Experiment (Phases 2–3), which mostly did not. The goal is only to name the
properties that differed between the two, not to improve either.

Date: 2026-07-17

---

## Shape of Input

**Role Meaning**: a closed-set classification — exactly one of `internal` / `external` /
`affiliated` / `unresolved` — each option itself a short, complete, self-contained statement ("an
employee or operator of our own business, acting on its behalf"). The entire fact *is* the answer;
there is nothing to extract or paraphrase from it.

**Persona-policy facts**: open-ended free-text prose describing a business rule, of unbounded shape
and length — e.g. compliance's fact ran three clauses deep ("must be honored within a legally
mandated minimum window regardless of the item's condition, using the original payment method for
any refund"). Consuming this requires the model to decide which part, if any, answers which
checklist question — a strictly harder task than reproducing a closed-set value verbatim.

This maps directly onto the citation mechanism's own mechanics
(`unresolved-decisions-become-explicit-decision-points`): a `resolved` classification requires an
`evidence` field naming or quoting the exact source. A closed-set value is trivially quotable in
full; free-text prose has to be selectively excerpted, and every observed citation failure in the
persona-policy runs is consistent with the model failing that selection step, not failing to notice
the fact existed.

## Scope of Input

**Role Meaning**: one question, one axis, one classification, one artifact. Nothing else was
bundled into the same fact.

**Persona-policy facts**: multiple claims bundled under one shared ADR title
("Return Eligibility and Verification Policy") — e.g. growth_retention's fact asserted *both* an
exchange-offer trigger *and* a mandatory reason-capture requirement in the same statement;
compliance's bundled a timing rule, a condition-independence rule, and a payment-method rule
together. Phase 2's own analysis already found this correlates with less reliable citation, holding
content specificity roughly constant — the broader-scope facts were cited far less consistently
than the single-question role fact, even when they contained genuinely concrete language.

## Consumer

**Role Meaning**: the experiment targeted one specific, already-identified existing consumer — the
`authorization` checklist area — and verified, via a controlled comparison, that the fact reaches
exactly that consumer.

**Persona-policy facts**: no single targeted consumer. A general "eligibility and verification"
statement is plausibly relevant to several of the six checklist areas at once (uniqueness,
authorization, consistency, idempotency), with no signal in the fact itself about which area should
consume which part of it. This ambiguity is directly visible in the results: `customer_experience`'s
and `compliance`'s resolved policies cited the generic user story, or — in one case — the wrong ADR
entirely (the domain-event ADR, not the supplied policy fact), consistent with the model guessing
at relevance rather than following an unambiguous path from fact to consumer.

## Storage

**Role Meaning**: designed to occupy a storage shape that already matches the fact's own shape —
`Role::Described { name, description }`, a categorical field built for exactly this kind of
classification, already present in `canopy-core`, simply unpopulated by the one code path that
matters.

**Persona-policy facts**: stored as a generic ADR (`title`/`decision`/`reason`/`alternatives`) — a
shape built for architecture decisions, with no field corresponding to any of the six checklist
areas the fact might address. The storage shape itself carries no signal about which downstream
consumer should read which part.

## Traceability

**Role Meaning**: direct, verified, verbatim citation in 2 of 3 tested conditions — the `evidence`
field literally reproduces the injected fact's text.

**Persona-policy facts**: traceability was mixed to poor across all five personas. The one clean
exception, `risk_averse`, succeeded specifically because its fact happened to name one concrete,
quotable artifact ("the original order/purchase confirmation number, verified against our
records") — the same property that made Role Meaning's fact work, appearing once, incidentally,
inside an otherwise broader-shaped fact. Every other persona's fact — including ones with seemingly
clear content — was cited only via the generic story or not at all.

## Regeneration Stability

**Persona-policy facts**: directly measured to be unstable under regeneration. `compliance`'s fact
failed to leave any trace across two entirely separate generation runs (Phase 2's standalone
comparison and Phase 3's real regeneration). The structural divergence Phase 2 originally credited
to persona content (customer-identity vs. product-identity scoping) inverted under Phase 3's
regeneration of nominally identical input — direct evidence that apparent success in one run is not
proof of a durable, repeatable effect.

**Role Meaning**: **not tested for regeneration stability in the same way** — the Value Experiment
ran each of its four conditions once, in parallel, not repeated across multiple regenerations of the
same condition. This is a real asymmetry in what's known, not a proven advantage: Role Meaning's
result is a clean single-pass success; whether it would hold up across repeated regeneration the way
the persona facts were shown *not* to has never actually been checked. Stated honestly rather than
assumed favorably.

---

## What Properties Made Role Meaning Succeed

Five properties emerge from the comparison above, each grounded in a specific, named contrast, not
inferred generally:

1. **The fact is a closed-set value, not free text** — the entire content is quotable as-is, with
   nothing to extract or select.
2. **The fact addresses exactly one question** — no bundling of multiple claims under one shared
   statement.
3. **The fact is aimed at one identifiable consumer**, not left ambiguously relevant to several.
4. **The fact's storage shape matches its own shape** — a categorical answer stored in a categorical
   field, not squeezed into a generic decision-record shape built for something else.
5. **The fact contains at least one concrete, nameable artifact or mechanism** — this is the one
   property that also explains `risk_averse`'s partial success *within* an otherwise broad,
   multi-claim fact, meaning it operates somewhat independently of properties 1–4 rather than being
   fully subsumed by them.

**What remains genuinely unknown, stated as an open gap rather than folded into the above**:
whether a fact satisfying all five properties is also stable under repeated regeneration. The
persona facts were shown to be unstable; Role Meaning's success was never tested against that same
condition. The minimum-property list above describes what correlates with a fact being *consumed at
all*, in a single pass — not, on the evidence actually gathered, what guarantees it stays consumed
across repeated runs.
