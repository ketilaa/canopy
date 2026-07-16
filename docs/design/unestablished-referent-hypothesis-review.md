# Does "Unestablished Referent" Name a New Concept?

Status: evaluation only. The previous roadmap update
(`docs/design/roadmap-reassessment.md`, "Update 2026-07-16") proposed a shared pattern — an
"unestablished referent" — across four gaps found by the Product-Owner Perspective Experiment.
That proposal was offered as a synthesis, not tested against counter-evidence. This document runs
it through the discipline explicitly requested: Observation → Hypothesis → Evidence review →
Counter-evidence → only then decide. **Conclusion, stated up front so the correction is visible
rather than buried**: it does not survive as a new concept. Three of the four gaps are better
explained as a coverage limitation in the *existing* Decision Point mechanism; the fourth already
has a home. This corrects, rather than silently retracts, the prior update — matching this
project's own disclosure discipline (see the Contract Composition Assessment's Section 8 for
precedent).

Date: 2026-07-16

---

## 1. Observation

Stated as bare fact, no interpretation, re-checked directly against the artifacts:

- `stories/manufacturer-001/spec.yaml`, scenario 05: registration is rejected when "a manufacturer
  with the name 'X' already exists" — the only stated criterion for treating two records as the
  same manufacturer is name-string equality.
- `roles.yaml` contains exactly one line: `manufacturer representative`. No other artifact in this
  project defines what that label denotes.
- Every one of the 12 scenarios' `given` clauses includes "The manufacturer representative is
  authenticated." The generated `stories/manufacturer-001/openapi.yaml` has no `security` scheme on
  `POST /manufacturers`.
- The story's own `so_that` field: "so that products can reference them in the system."
  `domain_registry.yaml` has never contained a `Product` entity anywhere in this project's history.
- For this story: `completeness.yaml` → `gaps: []`. `decisions.yaml` → `decisions: []`.
  `decision-audit.yaml` → `findings: []`.

These five bullets are the entire observational basis. Everything past this point is inference.

## 2. Hypothesis

The previous update's claim: these four gaps (duplicate-name, role semantics, authorization,
missing `Product` relationship) are instances of one shared class — an *unestablished referent* —
where generated language uses a term or relationship as though it already denotes a specific,
agreed real-world concept, when that concept was never separately confirmed against the project's
own vocabulary. The claim further asserted this class is distinct from, and sits "one level behind,"
`unresolved-decisions-become-explicit-decision-points` (which covers a model recognizing a question
and fabricating an answer) — because in three of the four cases nothing suggests the model ever
recognized a question at all.

## 3. Evidence review

What supports treating this as one real, shared phenomenon:

- All four share an observable structural trait: no entry in `domain_registry.yaml`/`roles.yaml`/
  an ADR/a Decision Point defines the term in question. That's directly checkable, not asserted —
  confirmed again in §1 above.
- Two of the four were reached independently, via different personas reasoning in different ways:
  duplicate-name (domain-expert PO's data-modeling angle; customer-outcome PO's usability angle)
  and the `Product` relationship (product-portfolio PO's portfolio reasoning, converging with
  `domain-boundary-hypothesis-assessment.md`'s separately-derived, more abstract reasoning about
  naming-convention variance). Independent convergence is real signal, not assumed —
  `docs/retrospectives/2026-07-12.md`'s own "Learned" section states this project's standing view
  that independent convergence across isolated readers is stronger evidence than one reader's
  single observation.
- The pattern is *consistent with* two already-validated principles without being *identical* to
  either: `structure-emerges-from-behavior` (deferring roles/boundaries to concrete behavior
  produced better results than upfront abstract elicitation — relevant to the role-semantics and
  `Product`-relationship gaps) and `unresolved-decisions-become-explicit-decision-points` (models
  silently interpret ambiguous business questions — relevant to duplicate-name).

## 4. Counter-evidence

This is the section the prior update skipped, and where the hypothesis mostly stops holding up.

**Sample-size and method limitations, stated plainly:** this is one story, reviewed once, by five
*simulated* personas authored in a single pass by one evaluator — not five independent real
Product Owners. The "independent convergence" claimed in §3 is convergence across personas I wrote,
reasoning through lenses I constructed, in the same sitting. That is meaningfully weaker evidence
than, say, the reproducibility sweep's five genuinely separate LLM calls against the actual target
system. It is not worthless — a structured multi-lens re-reading can still surface real
differentiated framings a single holistic read would miss — but it cannot bear the same evidentiary
weight the phrase "independent convergence" implies without this caveat attached.

**Do the four instances actually belong to one class, or does the grouping only hold under a
label loose enough to fit anything?** Examined individually against the *existing* Decision Point
heuristic (`docs/design/behavior-first-planning.md`, Stage 2: "if answering the question would
change a validation rule, a persistence rule, an API contract, an event contract, or a test
expectation, it's a Decision Point"):

- **Duplicate-name** clearly changes a validation/persistence rule. It is squarely inside the
  existing heuristic's stated scope — the model *did* make a specific choice (name-only equality)
  and never flagged the alternative. This is not a new class; it is close to a textbook instance of
  `unresolved-decisions-become-explicit-decision-points` that Stage 2 simply didn't catch for this
  story. Filing it under a new "unestablished referent" umbrella double-counts evidence that
  already belongs to an existing, validated principle.
- **Authorization** would change an API contract (a `security` scheme) directly — also inside the
  existing heuristic's stated scope in principle.
- **Role semantics** is prior to, and arguably inseparable from, authorization — resolving "who is
  a manufacturer representative" is a precondition for resolving "what are they authorized to do,"
  not a separately-motivated third gap. Counting these as two distinct instances of a new class,
  rather than one gap examined from two angles, likely overstates how many independent findings
  this experiment actually produced.
- **The `Product` relationship** is different in kind from the other three, and this is the one
  place the counter-evidence review confirms something real: Stage 2's design operates *per story*,
  on behaviors already extracted from that story's own spec. Nothing in its design ever claimed to
  check a story's stated purpose against entities named in *other* stories, or entities implied but
  never scoped in. This gap is structurally outside what the existing mechanism was ever built to
  catch — but it already has a durable home: `docs/open-questions/domain-boundary-explicitness.md`,
  filed the day before this experiment ran, for exactly this shape of cross-story concern.

**A more mundane, better-evidenced explanation for why Stage 0/2 missed the within-story
gaps.** `exhaustive-enumeration-over-holistic-review` (high confidence, validated) already states
that this project's own models reliably miss items under open-ended review that they catch under
explicit, item-by-item enumeration — and that this exact fix already worked for Stage 0's original
constraint audit (4 of 9 gaps found holistically → all 9 found via explicit field×constraint
traversal). Neither Stage 0's completeness prompt nor Stage 2's decision-extraction prompt currently
enumerates "does every named actor/role have a stated definition," "does every uniqueness/equality
claim have an explicit criterion," or "does every precondition word imply an authorization rule that
was actually decided" as explicit checklist items — they ask about gaps and business policy more
holistically. Under this project's own already-validated principle, that is a sufficient, more
parsimonious explanation for why duplicate-name/role-semantics/authorization slipped through this
one story's Stage 0/2 runs, with no need to posit a new failure category to explain it.

## 5. Decision

**"Unestablished referent" does not hold up as a new concept.** Restated by instance:

- **Duplicate-name**: not new — an instance of the existing, validated
  `unresolved-decisions-become-explicit-decision-points` principle that this one story's Stage 2
  run happened to miss.
- **Role semantics / authorization**: best read as one gap, not two, and most likely explained by
  `exhaustive-enumeration-over-holistic-review`'s already-validated finding — Stage 0/2's own
  checklists don't currently enumerate role-definition or authorization-implication as items to
  check, not evidence of an undiscovered class of model failure.
- **`Product` relationship**: genuinely outside the existing per-story mechanism's design scope, but
  already tracked as `domain-boundary-explicitness.md` — not a reason to mint a new concept, since a
  home for it already exists.

**What actually survives this review, worth keeping**: not a new concept, but a concrete,
falsifiable hypothesis about an *existing* mechanism's checklist completeness — does explicitly
adding "is every named actor/role defined," "is every uniqueness/equality claim's criterion
explicit," and "does every authorization-shaped precondition have a corresponding resolved rule" to
Stage 0's or Stage 2's own enumeration close this gap, the same way explicit field×constraint
enumeration closed Stage 0's original constraint-audit gap? This is a testable, narrow, tier-1/2
question (per CLAUDE.md's escalation order — a prompt fix to an existing mechanism, not a new
mechanism) — not proposed or scoped further here, since this document's own charter is evaluation,
not redesign.

**Correction to the prior update**: `docs/design/roadmap-reassessment.md`'s 2026-07-16 update
should be read alongside this document — its four-gap "shared pattern" framing is superseded by the
finding above. The experiment's underlying observations (§1) stand unchanged and remain useful
evidence; the *synthesis* drawn from them does not.
