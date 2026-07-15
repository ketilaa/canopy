---
title: "Two of Three Runs Invented a Field We Never Asked For"
date: 2026-07-15
status: draft

learning_type:
  - design-evolution
  - failure-analysis

topics:
  - ai-assisted-code-generation
  - contract-driven-implementation
  - system-design

key_principles:
  - implementation-ownership-requires-full-file-scope-visibility

source_artifacts:
  - "docs/design/contract-driven-implementation-experiment.md — Stage 1 and Stage 2 Results"
  - "docs/principles/implementation-ownership-requires-full-file-scope-visibility.md"
  - "canopy-llm/examples/contract_driven_stage1_experiment.rs"
  - "canopy-llm/examples/contract_driven_stage2_experiment.rs"

story_ids:
  - manufacturer-001

evidence_strength: high

commits:
  - 5a8a4b4
  - 260ff44
  - e368cb1

initial_assumption: >
  A model told exactly what one contract authorizes, with an explicit instruction not to invent
  anything beyond it, would stay inside that scope — the instruction was correct, well-worded,
  and correctly positioned in the prompt.

final_understanding: >
  The model wasn't ignoring the instruction. It was filling in a gap the instruction couldn't
  close by itself: nothing told it that other, unshown contracts existed and already accounted
  for the fields it invented. The fix wasn't a stronger instruction on the same partial view —
  it was showing the model the rest of the room.
cluster: "Full-Scope Visibility"
---

# Summary

We gave a model one validation contract, an explicit "don't invent unrelated fields" instruction,
and asked it to write a Java class. Two of three runs added `@Entity`, `@Id`, and
`@GeneratedValue` anyway — persistence annotations that belonged to a *different* contract we
never showed it. The instruction was right there, correctly worded. It didn't matter. What fixed
it wasn't a stronger instruction. It was showing the model five more contracts it had never seen.

# Original Assumption

We'd just finished validating that Canopy's `Contract` type — `kind`, `entity`, `member`,
`mandatory`, `required_tests`, `dependencies` — carries enough information to drive real
implementation, one field at a time. The next test was the obvious one: give a model exactly one
contract, tell it precisely what it owns, and see if it stays inside that boundary. We wrote the
instruction carefully: "This file may eventually need to satisfy OTHER fields/contracts not shown
to you here — implement ONLY what THIS contract requires... do NOT invent unrelated fields,
methods, or class structure beyond the minimum." That felt like enough. It named the risk
directly and told the model what not to do.

# What Happened

Three runs, same contract (`ManufacturerNameValidation` — one field, two behaviors, zero
dependencies), same prompt, same model. Run 1 came back clean: a `name` field, a getter and
setter, a hand-rolled validation method. Runs 2 and 3 both added something we never asked for:

```java
@Entity
public class Manufacturer {
    @Id
    @GeneratedValue(strategy = GenerationType.IDENTITY)
    private Long id;
    // ...
```

`id` wasn't in the contract we gave it. Neither was `@Entity`. The instruction telling it not to
do exactly this was sitting a few lines above, in the same prompt, worded plainly. It didn't
matter, in two of three runs.

Our first read was that the instruction needed to be stronger — a WRONG/CORRECT example, maybe,
or a harder-edged phrasing. But we'd already learned, earlier in this same investigation, not to
trust that read without checking the actual cause first. So we asked a narrower question: what if
the model wasn't disobeying the instruction, but filling in something the instruction genuinely
couldn't supply — the fact that `id` was a real, already-decided part of *this* file, just
authorized by a contract it had never been shown?

We had the answer already sitting in real data. This same story's actual `contracts.yaml` had six
contracts, not one, all resolving to the same file — five validation contracts plus a construction
contract that explicitly owns `id`, `createdAt`, and `modifiedAt`. We'd been showing the model a
fifth of the truth and asking it to guess correctly about the other four-fifths.

We changed exactly one thing: showed the model all six contracts sharing that file, instead of
one, with everything else in the prompt held constant — same model, same skill text, same target
file, same withheld story/scenarios/ADRs. Three runs. All three came back with exactly the eight
fields those six contracts authorize. `@Entity`/`@Id`/`@GeneratedValue` appeared in every run
again — and this time they were correct, because a contract now actually said so.

# Evidence

- 2 of 3 runs invented `@Entity`/`@Id`/`@GeneratedValue` with a single contract shown, despite an
  explicit, correctly-worded, correctly-positioned instruction not to.
- The only variable changed between the failing trial and the fix was visibility: all six
  contracts sharing the file, shown at once, instead of one at a time. Nothing else in the prompt
  changed.
- 3 of 3 runs clean after the change — exactly the eight authorized fields, every time.
- A built-in negative control, in the very same fixed run: two *other* defects (Bean Validation
  annotations with no enforcement mechanism; `id` relying on `@GeneratedValue` alone, which never
  fires on a plain constructor call) persisted at the same rate regardless of how many contracts
  the model could see. That's what let us tell "this fixes ownership" apart from "this generally
  makes output better" — the visibility fix moved exactly one thing, and only that thing.
- Confirmed twice more since, under progressively stricter scrutiny: real compilation and test
  execution (not just reading the generated code), then production wiring
  (`generate_story_plan_from_contracts`, now the mechanism that actually groups contracts by
  shared file before any real generation call happens).

# Evolution of Understanding

We went in assuming a scope violation meant the instruction wasn't strong enough. What we found
instead: the model wasn't choosing to disobey a rule it could see. It was pattern-matching a
familiar shape — "this looks like a JPA entity, entities have ids" — because nothing in its
context told it that shape's missing piece was accounted for somewhere else. An instruction can
say "don't invent this" as many ways as you like; it can't supply a fact that simply isn't in the
prompt. The fact that was missing wasn't about the field itself — it was about the *existence of
other contracts* that already covered it.

This reframes what "ownership" means for a contract-driven system. A contract doesn't just say
what one file-slice contains. For a model to respect a boundary, it needs to see the *whole*
boundary — every contract that claims a stake in the same file — not just its own slice with
better-worded fences around it.

# Engineering Principle

See [[implementation-ownership-requires-full-file-scope-visibility]] for the full statement and
evidence. In short: when a file's correct shape is the union of several independent contracts,
show the model every contract that shares it before asking it to generate anything — not one at a
time, however precisely worded the scope instruction is. Full visibility of the combined
authorized scope is what stops the invention; a stronger instruction on the same partial view
isn't.

# Why It Generalizes

Any system that composes a generated artifact from multiple independently-authored units — not
just contract-driven code generation — is exposed to this. OpenAPI specs assembled from several
endpoint contracts. Configuration files built from several services' declared needs. Anywhere a
model is shown one slice of an artifact's true, multi-party scope and asked to produce the whole
artifact, a strong training prior about what that *kind* of artifact usually contains can fill the
gap the prompt left open — and no amount of "don't invent things" phrasing closes a gap that's
actually a missing fact, not a missing rule.

# Remaining Questions

Every confirmation of this finding — the original 2-of-3-versus-3-of-3 result, the real
compilation pass, the production wiring — rests on the same one entity, one file, one story. We
don't yet know whether the effect holds as the number of contracts sharing a file grows large (six
here; would twenty reintroduce a different failure mode, like the combined list becoming too much
context to hold reliably?), or whether it generalizes to a second, independent story at all. The
Contract Composition Assessment names this directly as the next test worth running before treating
this finding as settled rather than strongly suggested.
