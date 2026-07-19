# Do Story Readiness Failures Reduce to Two Fundamental Blockers?

Status: reduction test only. No mechanism, check, or framework proposed anywhere below, per
explicit instruction. Tests a specific hypothesis — that Story Readiness fundamentally means (1) no
internal contradictions and (2) no business-significant decisions silently treated as resolved,
with every other confirmed class either collapsing into one of those two or existing outside the
blocker question entirely — against the evidence already assembled in `docs/design/story-readiness-
failure-taxonomy.md` and `docs/design/story-readiness-failure-severity-classification.md`.

Date: 2026-07-19

Reviewed: both documents above, `docs/design/product-010-story-readiness-failure-diagnosis.md`,
`docs/reports/manufacturer-001.md`, `docs/design/{unestablished-referent-hypothesis-review,
domain-event-decision-point-criteria-comparison}.md`, `docs/principles/{unresolved-decisions-
become-explicit-decision-points, structure-emerges-from-behavior}.md`, `canopy-core/src/lib.rs`
(`DecisionCategory`).

---

# Candidate Fundamental Readiness Blockers

**A — No internal contradictions.** A story's own generated artifacts must not assert incompatible
things. Stands as offered — this document does not attempt to reduce A further, since the
hypothesis under test takes it as one of the two atomic cases, and nothing in the reviewed evidence
suggests a more basic property underneath it.

**B — No business-significant decisions silently treated as resolved**, precisely scoped to the
subset the pipeline's own `DecisionCategory::Business` already names (changes a validation rule,
persistence rule, API contract, or event contract) — not every silently-made decision, which the
severity-classification document already established splits by stakes
(`DecisionCategory::BehavioralAmbiguity` instances land at Warning, not Blocker). The hypothesis as
stated already builds in this qualifier ("business-significant"), which resolves rather than
contradicts the earlier finding that B doesn't classify uniformly.

---

# Classes That Collapse Into Those Blockers

### D — Ambiguous referent / undefined role semantics → collapses into B

**Is this fundamentally a readiness blocker?** Not as its own category.

**Is it evidence some instance of A or B exists?** Yes, argued directly from the reviewed material.
`unestablished-referent-hypothesis-review.md`'s own conclusion treats role semantics and
authorization as "one gap, not two" and states plainly that duplicate-name handling (a closely
related instance) "is not a new class; it is close to a textbook instance of
`unresolved-decisions-become-explicit-decision-points` that Stage 2 simply didn't catch for this
story." The same document also states role semantics "is prior to, and arguably inseparable from,
authorization" — and authorization, in `product-010`, is a confirmed B instance. An undefined actor
label ("manufacturer representative") is substantively a business question — who is this actor,
what can they do — that never crystallized into a stated policy-checklist item at all; it's B one
step further upstream (a question that never reached classification), not a different kind of
question.

**Could it independently make a story not ready?** Only through B's own criterion (if the ambiguity
turned out to be business-significant, e.g., it actually changes an authorization rule) — there is
no evidence of D mattering on its own terms, separate from whatever business question it turns out
to encode.

**Evidence for.** Direct textual conclusion from the document that investigated this exact question
on purpose; the role-semantics/authorization pairing observed in the same story
(`manufacturer-001`).

**Evidence against.** The evidence base is thin either way — one simulated-persona observation,
never confirmed to have caused a real downstream defect. The reduction is plausible, not proven by
a demonstrated consequence.

### G — Instruction-compliance gap → plausibly collapses into A, but not confirmed by any document on file

**Is this fundamentally a readiness blocker?** Reasoned as one in the severity-classification
document, but on reflection here, its independence from A is questionable.

**Is it evidence some instance of A or B exists?** A case can be made that it's A, reframed: the
domain-event existence rule is not an isolated fact, it's a consequence of the Architecture Style
ADR ("event-driven") plus the story's own classified action (creation/update/deletion) — an
already-established artifact, plus a determinable fact about this story, jointly implying a domain
event must exist. A story whose generated behaviors omit one *contradicts* what its own governing
ADR requires, in the same structural shape as `product-010`'s `out_of_scope`-vs-scenario
contradiction: two already-existing facts about the project disagree with what was actually
produced. It is not evidence of B, since the criteria-comparison document is explicit that this
sub-decision "isn't being asked to judge whether one is warranted" — there is no judgment call
here at all, which is precisely what disqualifies it from B's shape (B requires a genuine business
question with no basis; G has a basis, the rule is simply not followed).

**Could it independently make a story not ready?** If the A-reduction above is accepted, then no —
it's the same blocker (A) reached through architecture-level rules instead of same-story fields.
If rejected, it would need to stand as a third, independent atomic case, which the current evidence
is too thin to support with confidence either way.

**Evidence for the reduction.** Structural: both A and this reframing of G describe an already-
settled fact (a field's own content; an ADR's own stated rule) disagreeing with what was actually
generated — the same shape at different scopes (within-story vs. story-vs-architecture).

**Evidence against.** No document on file actually states this reduction — it's this document's
own inference, not something `domain-event-decision-point-criteria-comparison.md` or any other
source concluded. That document treats the compliance gap as its own thing, explicitly distinct
from the "unresolved decision" (B) framing, but never explicitly compares it to A either. This
reduction should be read as plausible, not established.

---

# Classes That Remain Independent

### C — Capability/entity presupposed but never established → independent phenomenon, but not a blocker

**Is this fundamentally a readiness blocker?** No — already established in the severity
classification (`structure-emerges-from-behavior` argues directly against it).

**Is it evidence some instance of A or B exists?** Not necessarily, and this is the sharpest
finding in this reduction test. `product-010`'s C instance *did* co-occur with both A and B — but
`manufacturer-001`'s `Product`-relationship gap did not. There is no `out_of_scope` field
contradicting the `Product` reference (no A), and no Policy Discovery item about `Product` was
ever silently resolved (no B) — the gap arose because domain-vocabulary extraction and Stage 0/2's
per-story checklists simply never look outside the current story's own text at all, a scope
limitation, not a misfired check. `docs/open-questions/domain-boundary-explicitness.md` names this
precisely: a cross-story concern structurally outside what any current per-story mechanism was
built to catch. This is real, direct evidence that C can occur cleanly without A or B present —
it does not reduce to either.

**Could it independently make a story not ready?** No, per the severity classification's own
reasoning: writing a story ahead of a not-yet-built capability is this project's normal,
principled mode of operation, not a defect — regardless of whether C happens to co-occur with A/B
in a given instance or not.

**Evidence for treating it as independent.** The `manufacturer-001` `Product` case is a clean
instance with neither A nor B present, directly contradicting a full reduction.

**Evidence against (i.e., for reduction).** Every C instance observed so far that *did* produce a
genuinely dangerous outcome (`product-010`) also had an A/B-shaped defect riding alongside it — one
could argue C only becomes practically consequential when paired with A or B, even if it can occur
independently in principle. This is a real tension the current evidence doesn't resolve: C is
independent as a *phenomenon*, but its two observed instances differ sharply in whether they
co-occurred with a genuine blocker.

### F — Checklist/enumeration axis missing → a mechanism failure, not a story-content class at all

**Is this fundamentally a readiness blocker?** No — already established as a category mismatch in
the severity classification (F describes a *review mechanism's* scope, not a property of a story's
own content).

**Is it evidence some instance of A or B exists?** Functionally, yes, in every confirmed
application on file: F's role-semantics application explains why a B-shaped gap (via D) went
undetected. But F's original evidence base (Stage 0's 4/9 constraint-completeness miss) doesn't
map cleanly onto A or B either — a missing `max_length` constraint is neither a contradiction
between two artifacts nor a business decision silently resolved, it's closer to a schema-
completeness omission with no clean home in either category. F is best described, in the terms
this document's own framing offers, as **a mechanism failure**: its real-world consequence is
"some other class's instance goes undetected," not that F itself is a new content-level defect
sitting alongside A/B/C/D.

**Could it independently make a story not ready?** No — a checklist gap in the *reviewing*
mechanism says nothing about whether the story it failed to fully check is actually sound or
unsound; that's determined by whichever class's instance the gap let through, not by the gap
itself.

### H — Missing-upstream-fact / sequencing gap → outside the frame; not a defect

**Is this fundamentally a readiness blocker?** No, and not by a small margin — already established
in the severity classification as the one class the evidence argues is *not a failure at all*.

**Is it evidence some instance of A or B exists?** No. By construction, H is exactly the case where
there is *no* contradiction (the fallback the model produced is textually correct given the true
absence of an upstream ADR) and *no* silently-fabricated decision (nothing was invented; a
deterministic rule was correctly applied to the actual state). H sits outside the entire
collapse-vs-independent framing because there is nothing here to collapse or remain independent —
the artifact produced was correct.

**Could it independently make a story not ready?** No — this is the class the evidence most
directly rules out of the failure space altogether, not merely a low-severity member of it.

---

# Evidence For Reduction

- **The reduction holds fully for the Blocker question specifically.** Of the seven confirmed
  classes examined, none was shown to independently constitute a Blocker outside of A and B — D
  collapses into B by direct textual conclusion from the document that investigated it; G plausibly
  collapses into A by structural analogy (though unconfirmed); C, F, and H were all independently
  ruled out of Blocker status by evidence already on file (a validated principle, a category
  mismatch, and a direct "this is correct behavior" finding, respectively).
- **Both A and B have direct, independent precedent elsewhere in this project's own design for
  being treated as fail-loud, not merely advisory** — Entity/Event Continuity for A's lexical form
  (refuses to save on mismatch), and the zero-citation enforcement in `bucket_policy_checklist` for
  B's clean form (rejects ungrounded "resolved" answers). No other class has comparable existing
  enforcement precedent in this codebase.
- **The two confirmed classes with the deepest evidence base (B and C, per the taxonomy's own
  confirmed-instance count) split cleanly**: B's business-significant instances behave like A/B's
  own kind (blocking), while C's instances — even the more severe one, `product-010` — were argued,
  via a direct counterfactual, to depend on A/B for their severity rather than supplying an
  independent blocking property themselves.

# Evidence Against Reduction

- **C resists full reduction, concretely, not hypothetically.** The `manufacturer-001`
  `Product`-relationship instance is a real, observed case of a readiness-relevant finding with
  neither an A-shaped contradiction nor a B-shaped silent resolution anywhere in it. If "Story
  Readiness failures reduce to A and B" is read as a claim about the full *phenomenon* space rather
  than only the *Blocker* subset, this instance falsifies it directly — C is a third, genuinely
  independent axis, even though it doesn't rise to Blocker severity.
- **G's reduction into A is this document's own inference, not a conclusion drawn anywhere in the
  reviewed material.** `domain-event-decision-point-criteria-comparison.md` treats the compliance
  gap as analytically distinct from both A and B without ever proposing A as its explanation
  either. Treating G as "collapsed" overstates how settled this is.
- **F's original evidence (the Stage 0 constraint-completeness miss) doesn't obviously belong to
  either A or B**, which means "everything collapses into A or B" is not quite accurate even for
  the classes this document does treat as reducible — F is better described as sitting outside the
  A/B frame entirely (a mechanism-level failure) than as evidence a hidden A or B instance exists,
  except in its one applied case (role semantics, via D).
- **The sample remains small.** Every confirmed instance of every class in this taxonomy comes from
  exactly two stories (`product-010`, `manufacturer-001`). A reduction that holds across two data
  points is a real, evidence-based finding, but not yet a law — the Human-Insight Inventory rerun's
  own discipline (don't generalize past what's been observed) applies here with the same force.

---

# What Would Need To Be Observed To Falsify This Model

- **A confirmed C instance with a demonstrated defect and no accompanying A or B**, i.e., a story
  where presupposing a not-yet-built capability, on its own, produced a real problem (not merely a
  plausible risk) without any contradiction or silently-resolved decision alongside it. This would
  directly overturn the "C never independently blocks" position, not just weaken it — currently no
  such instance exists; the one severe C case (`product-010`) always co-occurs with A and B.
- **A G instance where the missing mandatory element does *not* trace back to any ADR or governing
  rule** — i.e., a compliance gap against a rule that isn't itself an established project fact,
  which would break the proposed A-reduction (there would be nothing for the omission to
  contradict) and force G to stand as an independent third case.
- **A D instance that causes a real, observed downstream defect** (not merely noticed by a
  simulated reviewer) that does *not* trace to any specific business question — which would break
  the D-into-B reduction by showing role/referent ambiguity can matter independent of any
  identifiable business decision underneath it.
- **A confirmed instance of E** (dependency assumed but never modeled) — entirely absent from the
  current evidence base, so its relationship to A/B is currently unknowable in either direction;
  its first real instance would be the first real test of whether this hypothesis needs a third
  slot.
- **A second project or a second multi-story dataset** repeating the same pattern (A/B as the only
  confirmed independent Blockers, C as independent-but-non-blocking, D/G plausibly reducible, F/H
  outside the frame) would move this from "holds across two stories in one project" to a
  meaningfully stronger finding. Nothing currently on file provides that second dataset.
