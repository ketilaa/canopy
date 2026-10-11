# Routing the repository contract to the request that needs it

Status: first increment built 2026-10-11 (`393d1cb`); the signals beyond names are not built.

## Why

Every request used to get the same block of repository facts: the Planner's prompt and every implementation
step. That is noise for each of them, and noise primes: "entity" and "repository" in a plan led the model to
JPA, and each concept mentioned in a context is a candidate for the output. As a contract grows, one block
for everybody stops working. `CLAUDE.md` already says to pass the smallest useful input and to scope
rules by layer; this applies it to the contract. Principle:
`docs/principles/hints-belong-in-repository-metadata.md`.

## Format (`.canopy/contract.yaml`)

```yaml
layers:                       # how a class is recognised
  - {name: controller, suffix: Controller, annotation: RestController}
rules:                        # what a request is told
  - id: spring-controller
    applies_to: {consumer: implementer, layers: [controller]}   # consumer: planner | implementer | both (default)
    content: |                #                                  # layers omitted: any layer
      Use @RestController.
      Use constructor injection.
```

`test` is a reserved layer. Each entry may carry `source: adr-NNN`, for people and the audit, never shown to
the model. The first format (`stereotypes`, `conventions`) is still read, as layers and rules that apply
everywhere.

## What each request is given

| Request | Derived facts | Contract |
|---|---|---|
| Planner | all | the stereotype line, and rules for `planner` or `both` |
| An implementation step | all | the stereotype of each of the step's layers, and the `implementer` or `both` rules for those layers or for every layer |

Derived facts (build, dependencies, the namespace, absences, existing classes) are not scoped yet; only the
contract is. A step whose layers cannot be worked out gets every rule for its consumer: noise over a missing
rule.

## How a step's layers are found

`Contract::layers_of(class, source)`, a ladder of signals, each added to a set:

1. **Name**: the class name (minus a trailing `Test`) ends with a layer's suffix. Works for classes not yet written,
   which is all the Planner has. A test class is also in `test`.
2. **Annotation**: a class that already exists carries the layer's annotation. This catches the class whose
   name does not say (`WidgetManager` with `@Service`). Used for `modify` steps and for classes read earlier.
3. **Not built:** the Roots graph (what a class implements or extends, who calls it), and the build graph
   (module or package). They plug into `layers_of` without a format change.

## Seen in a run

`routing.json` next to each generated plan lists, per step, the layers found and the rule ids the step was given;
`step_started` carries the context the step was actually sent. Together they answer "was the relevant rule
present when the step failed?", with the usual caveat that present is not consumed.

## Known weak points

- A planned class with an unconventional name has no layer until it exists: it falls back to all rules.
- Rules are scoped by a layer set, not by a kind of work (create or modify, test or production). The `test`
  layer covers part of that.
- An entry in `applies_to.layers` only restricts steps whose layers were found; the planner is not scoped by
  layer at all (it has no step).
