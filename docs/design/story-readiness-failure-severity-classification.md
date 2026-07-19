# Should Story Readiness Failure Classes Block, Warn, or Signal Backlog Evolution?

Status: classification only. No check, mechanism, process change, or framework proposed anywhere
below, per explicit instruction. Takes `docs/design/story-readiness-failure-taxonomy.md`'s eight
candidate classes and asks a different question of each: not "how would we detect this" but "what
should a confirmed instance mean for whether a story is ready" — using only evidence already on
file.

Date: 2026-07-19

Reviewed: `docs/design/story-readiness-failure-taxonomy.md`, `docs/design/product-010-story-
readiness-failure-diagnosis.md`, `docs/design/human-insight-inventory{,-rerun}.md`,
`docs/design/roadmap-reassessment.md`, `docs/open-questions/{story-readiness-vs-backlog-evolution,
domain-boundary-explicitness}.md`, `docs/principles/{unresolved-decisions-become-explicit-decision-
points, cross-artifact-consistency-audits-prevent-drift, structure-emerges-from-behavior}.md`,
`canopy-core/src/lib.rs` (`DecisionCategory`), `canopy-llm/src/prompts/{decisions,spec}.rs`.

A distinction used throughout, established directly from the evidence rather than asserted: **Story
Readiness** (is this story's own specification internally sound) and **implementation sequencing**
(can this story safely be built *right now*, given what else exists in the project) are different
questions. Several classes below turn out to matter for the second without being evidence against
the first — that split does real work in what follows, not just terminology.

---

# Readiness Blockers

A class belongs here when a confirmed instance means the story's own specification is internally
unsound — not a judgment call, not missing context, but self-contradictory or objectively wrong as
written.

### A — Cross-artifact same-story contradiction: **Blocker, high confidence**

**Should it block?** Yes. A story whose own fields assert incompatible things cannot be
consistently implemented — there is no reading of `product-010`'s spec where `out_of_scope`
("Customer authentication and authorization" excluded) and the accepted contract (a 401-Unauthorized
behavior pair) are both honored. This isn't ambiguity needing a human judgment call; it's a direct
logical inconsistency in the artifact itself.

**Evidence for.** The counterfactual already established in the prior diagnosis: the contradiction
holds regardless of project state (even if authentication existed elsewhere, these two fields
would still disagree) — a pure property of this one story's own internal soundness, the textbook
shape "not ready" is supposed to mean. There is also direct precedent in this project's own design
for treating artifact contradiction as blocking, not advisory: Entity Continuity and Event
Continuity (`cross-artifact-consistency-audits-prevent-drift`) *refuse to save* a generated
artifact on mismatch — "nothing is saved... the caller re-runs" — for the lexical form of this same
failure shape (a generated entity name disagreeing with established vocabulary). `product-010`'s
case is the semantic analogue that principle's own Future Validation section names as untested; the
existing lexical precedent is still real evidence about how this project already treats the general
shape when it does catch it.

**Evidence against.** Only one confirmed instance exists, and it happens to be a clean, binary
contradiction (excludes X / requires X). A milder cross-artifact tension — differing degree or
wording rather than logical impossibility — might argue for Warning instead; nothing in the current
evidence base tests that gradient, since `product-010` is the only real case on file.

### B — Unresolved decision silently resolved: **Blocker when business-stakes, see split below**

*(Full treatment under Ambiguous Cases — this class does not resolve to one severity tier uniformly.)*

### G — Instruction-compliance gap: **Blocker in principle, evidentially thin for Story Readiness specifically**

**Should it block?** In principle, yes, when the violated rule is genuinely unconditional. The
domain-event existence rule (`domain-event-decision-point-criteria-comparison.md`, sub-decision a)
is stated as MANDATORY, not a judgment call — a story missing a domain event it's mandated to have
is objectively incomplete, the same way a missing required field would be.

**Evidence for.** The rule's own wording removes ambiguity: "MANDATORY whenever... creates/updates/
deletes an aggregate" is a stated fact-check, not something reasonable people could disagree about.
Measured directly: 3 of 5 reproducibility-sweep runs complied, 2 didn't — a real, quantified
compliance gap, not a hypothetical.

**Evidence against.** This class has never actually been observed failing inside a Story Readiness
gate (Stage 0–4) — its only evidence comes from a reproducibility sweep measuring spec-generation
variance, a different investigation. `product-010` itself didn't exercise this class at all. Calling
it a Story Readiness *Blocker* is a reasonable extrapolation from "this project's own stated rule
was violated," not a direct observation of Stage 0–4 catching or missing it.

---

# Readiness Warnings

A class belongs here when a confirmed instance is real signal worth a closer human look, but not,
on its own, proof the story is unsound — the story could turn out fine on inspection.

### B — Unresolved decision silently resolved: **Warning when low-stakes**

**Should it merely raise caution?** When the underlying question is genuinely low-stakes — the
pipeline's own existing `DecisionCategory` enum already distinguishes `business` (blocks
implementation) from `behavioral_ambiguity` ("a softer wording/ordering/precision call, not truly
blocking") from `technical`. The domain-event *naming* sub-decision (b) in the criteria-comparison
document is exactly this shape: "the model picks between two prompt-offered synonyms," a
low-consequence choice recorded transparently (`alternatives` even names the road not taken) rather
than a silent, consequential fabrication.

**Evidence for.** The project's own mechanism already encodes this distinction mechanically — a
`business`-classified decision and a `behavioral_ambiguity`-classified one are not treated the same
way even where the Decision Point mechanism does fire, which is direct evidence this project's own
design intent is severity-graded, not uniform, for this class.

**Evidence against.** Whether a given silently-resolved decision is actually low-stakes is itself a
judgment call that has to be made *after* the fact — you cannot know a decision was low-stakes
without first noticing it was made at all, which is exactly what this class already fails to
surface reliably (Stage 2 never saw `product-010`'s authorization question because it was
misclassified upstream). Treating "some instances are low-stakes" as grounds for Warning-only risks
under-weighting the cases that turn out to be `business`-shaped, like `product-010`'s own case —
see Blocker discussion below.

### C — Capability/entity presupposed but never established: **Warning, secondary to Backlog Evolution Signal**

**Should it merely raise caution?** Yes, specifically when the story's *accepted behavior* actively
exercises the missing capability (not merely alludes to it in passing) — `product-010`'s 401 check
and `manufacturer-001`'s `Product` cross-reference in `so_that` are both cases where implementing
the story as specified would produce a dangling reference to something that doesn't exist. That is
worth a human's attention before implementation proceeds, distinct from whether the story's own
fields are self-contradictory (class A's separate question).

**Evidence for.** Both real instances (`product-010` auth, `manufacturer-001` `Product`) would
produce broken or meaningless behavior if implemented verbatim right now — a real, concrete
consequence, not a hypothetical risk.

**Evidence against — the stronger argument, and why this is not a Blocker.** `structure-emerges-
from-behavior` (high confidence, validated) is this project's own founding design principle:
information should be captured as concrete behavior demands it, not solicited or fully built out
upfront. Writing a story that presupposes a capability not yet built is not a defect under that
principle — it's the expected, normal shape of incremental delivery. Making C alone a Blocker would
directly contradict a validated principle this project already committed to. The real question C
raises is one of *implementation sequencing* (can this be safely built right now) rather than
*Story Readiness* (is the specification itself sound) — and no real instance of C, absent an A- or
B-shaped defect riding alongside it, has ever been treated as making a story un-buildable in this
project's actual history.

---

# Backlog Evolution Signals

A class belongs here when a confirmed instance is not really evidence about *this story* at all —
it's evidence that something is missing from the project's backlog, a different question with a
different natural response (add a story/entity), not "reject this story."

### C — Capability/entity presupposed but never established: **primary placement**

This is the clean generalization of `entity-with-no-story` (Iteration 1's own already-validated
signal) from entities to capabilities — "a concept this project has touched but never separately
established." `product-010`'s authorization gap and `manufacturer-001`'s `Product` gap are both,
at bottom, evidence that a capability/entity belongs on the backlog and doesn't exist there yet —
the same *kind* of fact Backlog Evolution already surfaces for entities, just one level of
abstraction further out (a capability rather than a named entity), exactly as
`docs/open-questions/story-readiness-vs-backlog-evolution.md` already reasoned. This is the
primary, not secondary, characterization of what C *is*; its Warning-tier placement above is about
what it means for one specific story in the moment, not a competing classification.

**Evidence for.** Both confirmed instances are naturally read as "this project should probably have
an authentication story" / "this project should probably have a `Product` entity" — exactly the
shape of finding Iteration 1's entity-with-no-story check already produces for entities, and
exactly the roadmap's own stated next-frontier reasoning for generalizing it.

**Evidence against.** No registry-like artifact for "capabilities" exists the way `domain_registry.
yaml` exists for entities — Backlog Evolution's current, narrower entity-only scope genuinely
cannot reach this today (already established in the earlier diagnosis: "not merely didn't, could
not, even in principle"). This is evidence the *signal* is real, not evidence the *current
mechanism* already produces it.

---

# Ambiguous Cases

### B — Unresolved decision silently resolved (full treatment)

Does not resolve to a single tier. Splits cleanly along a distinction this project's own pipeline
already encodes mechanically (`DecisionCategory::Business` vs. `::BehavioralAmbiguity`):

- **When the undecided question would change a validation rule, persistence rule, API contract, or
  event contract** (the `business` shape) — Blocker-tier. `product-010`'s authorization case is
  exactly this: the silently-resolved question directly determines whether a `security` scheme
  should exist. There is also real precedent for blocking-severity enforcement of this class when
  it's caught cleanly: `bucket_policy_checklist` already fails loudly (returns an error, forces a
  re-run) on a *zero-citation* fabrication — the project's own design intent for this class,
  where it works, is fail-loud. The vacuous-citation form that let `product-010` through is a gap
  in that enforcement, not evidence the intended severity is lower.
- **When the undecided question is a `behavioral_ambiguity`-shaped wording/ordering call** —
  Warning-tier at most, per the domain-event-naming precedent above.

Placed here rather than picked into one bucket because the evidence genuinely supports both, and
which one applies is only knowable per-instance, not per-class — this is a real, not a placeholder,
ambiguity.

### D — Ambiguous referent / undefined role semantics: **thin evidence, tentatively Warning-only**

Already downgraded from a standalone class to an instance of F
(`unestablished-referent-hypothesis-review.md`'s own conclusion). Its only evidence is one
simulated-persona observation (`manufacturer-001`'s "manufacturer representative," noticed by 1 of
5 personas) that never demonstrably broke anything downstream — unlike C's instances, nothing
about this ambiguity produced a dangling reference or an internal contradiction on its own. If
treated as its own bucket at all, the evidence only supports Warning ("worth a second look"), not
Blocker — but the honest position is that the evidence base is too thin (single simulated
observation, already folded into another class) to assign confident severity, and this document
does not attempt to strengthen that evidence beyond what §D of the taxonomy already established.

### E — Dependency assumed but never modeled: **cannot classify — no data**

No confirmed real instance exists anywhere in the reviewed material (taxonomy §E). Assigning a
severity tier to a class with zero observed instances would be speculation dressed as a finding.
By loose analogy to C (a missing link is closer to a bookkeeping gap than proof of an unsound
story), a real instance would likely land nearer Backlog-Evolution/Warning than Blocker — but this
is explicitly a guess, not a classification the current evidence supports.

### F — Checklist/enumeration axis missing: **category mismatch, not a severity question**

F is a property of a *review mechanism's own scope* (does Stage 0/2's checklist enumerate the right
items), not a property of any one story's content. Asking "does a confirmed instance of F make a
story not ready" doesn't quite parse — F's confirmed instances (Stage 0's original 4/9 constraint
miss; the role-semantics gap's best current explanation) describe *why* some other class's
instance went undetected, not a defect in a story on its own terms. Its effect on readiness is
indirect: a confirmed F-shaped gap means some other class (typically B or D, per the evidence
reviewed) may be under-caught, not that F itself is blocking, warning, or backlog-signaling
anything directly.

### G — Instruction-compliance gap (evidentiary caveat)

Placed under Blockers above on principle (a stated MANDATORY rule, violated, is unambiguous), but
repeated here because the caveat matters for confidence: this class's only measurement comes from a
reproducibility sweep of spec-generation, not from any Stage 0–4 gate actually catching or missing
it in a real Story Readiness pass. Its Blocker placement is a reasoned extrapolation, not a direct
observation the way A's and C's placements are.

### H — Missing-upstream-fact / sequencing gap: **unrelated to readiness, not merely low-severity**

This is the one class where the reviewed evidence argues the artifact produced is *not a defect at
all*. The criteria-comparison document's own analysis: when no Topic Naming Convention ADR exists
yet, "name the event only" is "the textually correct behavior given that state, not an invented
answer." The 1-of-5 convention-compliance rate reflects true upstream absence, not fabrication or
inconsistency. Unlike D or E (thin evidence, could still turn out to matter) or F (indirect but
real effect), H has a direct argument *against* it mattering for readiness at all: the fact being
measured is that the pipeline behaved exactly as it should, given the state that actually existed.
Recorded here rather than omitted because the question "should this be unrelated to readiness
altogether" was explicitly asked, and for this one class the answer is a considered yes, not a
default.

---

# Where `product-010` Lands

Composing the classifications above against the one real, fully-worked incident:

- **A (Blocker) fires.** `out_of_scope` and the accepted contract directly contradict each other —
  on this classification alone, `product-010` is not ready, independent of anything else true about
  the project.
- **B (Blocker sub-case) fires.** The authorization question was silently resolved with a vacuous
  citation, and it is `business`-shaped (it would change the API contract's `security` requirement)
  — the same severity tier as A, reached by a different route (how the contradiction was allowed to
  form, rather than that it exists).
- **C (Backlog Evolution Signal, secondary Warning) fires, but does not independently block.** No
  authentication capability exists anywhere in the project — real, relevant context that raises the
  stakes of A/B's failure (a 401 check with nothing behind it is worse than a redundant one), and
  a legitimate signal that an authentication story belongs on the backlog. Per the reasoning above,
  this fact alone — absent A and B — would not have made `product-010` un-implementable; writing a
  story ahead of a capability it will eventually need is normal, not a defect.
- **D, E, F, G, H do not fire** for this story. `product-010`'s role (`customer`) was never flagged
  ambiguous (D); no dependency was omitted, one was disclaimed (E); no evidence exists either way
  since F is mechanism-level, not story-level; G and H both come from an unrelated investigation
  (spec-generation reliability, a different story) and were never exercised here.

**Net verdict, composing all of the above**: `product-010` is doubly confirmed as not ready (A and
B both independently reach Blocker tier), with C explaining why the failure is dangerous rather than
merely untidy, and no evidence that C alone would have been sufficient to reach the same verdict.
This matches, and sharpens with an explicit severity argument, the earlier diagnosis document's own
conclusion that the primary failure is Story Readiness, not Backlog Evolution.

---

# Evidence That Would Change These Classifications

- **A milder, real cross-artifact tension** (not a clean logical contradiction, but a difference of
  degree or emphasis between two fields) would test whether A's Blocker tier holds across the whole
  class or only for binary contradictions like `product-010`'s — currently untested, since only one
  instance exists.
- **A real `business`-classified Decision Point that a human actually resolved incorrectly**, as
  opposed to one that was never surfaced at all, would test whether B's Blocker tier is really about
  the silent-resolution failure mode specifically, or about business-stakes decisions in general
  regardless of how they were reached.
- **A story that presupposes a missing capability with no accompanying contradiction or
  misclassification** would directly test whether C alone can ever justify Blocker tier, or whether
  the Warning/Backlog-Evolution-only placement holds generally, not just for the two instances on
  file (both of which happen to co-occur with A- or B-shaped defects, `product-010`'s case
  explicitly, `manufacturer-001`'s `Product` gap less directly).
- **A second real instance of ambiguous role/referent semantics (D)** that demonstrably produces a
  downstream defect (not just a persona noticing it) would move D out of "too thin to classify" and
  into a real severity tier — currently its only evidence never demonstrated consequence.
- **Any real instance of E at all** would resolve the current "cannot classify" placement into an
  actual evidence-backed tier — right now there is nothing to classify.
- **A demonstrated case of F's applicability claim being tested via fix-and-remeasure** (the
  role-semantics enumeration fix `unestablished-referent-hypothesis-review.md` proposed but did not
  implement) would clarify whether F's indirect effect on other classes' detection is as large as
  currently assumed, or smaller.
- **A Story Readiness gate (Stage 0–4) actually catching or missing a G-shaped mandatory-rule
  violation in a real story**, rather than only in a reproducibility sweep, would convert G's
  Blocker placement from a reasoned extrapolation into a directly observed one.
- **A case where an H-shaped sequencing gap turned out to cause a real downstream problem** (as
  opposed to being confirmed harmless, as in the one instance on file) would reopen whether "unrelated
  to readiness" is the right classification for this class, or whether it was only true in the one
  case observed so far.
