# product-010 Reassessed With a Confirmed Business Fact: Catalog Browsing Is Public

Status: reassessment, not a new investigation. A genuine domain fact — catalog browsing is
intended to be public, no authentication or authorization required — was supplied and used to
re-derive product-010's diagnosis from scratch, not to defend the prior one. Where prior
conclusions (`docs/design/product-010-story-readiness-failure-diagnosis.md`,
`docs/design/story-readiness-failure-severity-classification.md`,
`docs/design/story-readiness-reduction-to-fundamental-blockers.md`) hold up under this fact, that's
stated with the reasoning redone; where they don't, that's stated plainly.

Date: 2026-07-19

New fact used throughout: **browsing the catalog requires no authentication or authorization.**
`product-010` is blocked only by the catalog not existing yet, never by an access-control
requirement.

---

# Reinterpreting product-010

Re-reading the actual persisted artifacts against this fact, not against the prior diagnosis:

- `out_of_scope` excludes "Customer authentication and authorization" — **this is now known to be
  correct.** Building authentication is genuinely not this story's job, because browsing doesn't
  need it.
- `resolved_policies`' `authorization` entry resolves as *"the story does not explicitly mention
  any authorization requirements for browsing a catalog"* — the **conclusion** ("not required") is
  now known to be correct too. The reasoning that produced it (treating silence as permission) is
  unchanged and still unsound — more on this below.
- The accepted scenarios and contract still include `product-010-05` ("Customer receives an error
  message if they are not authenticated") and a `401 Unauthorized` behavior — and, more sharply than
  previously stated, **scenarios 01–04 each open their `given` clause with "The customer is
  authenticated..."** Every one of the five scenarios assumes an authentication concept exists,
  not just the one 401 case.

With the business fact in hand, this last item is simply wrong. Not "presupposes a capability that
happens not to exist yet" — wrong, full stop, because nothing about this behavior should ever have
depended on authentication. The earlier diagnosis treated `out_of_scope` and the scenario set as
two claims of *unknown* relative correctness that merely disagreed. That was the limit of what
artifact-level analysis alone could establish. With ground truth supplied, one side is now known
correct (`out_of_scope`) and the other is known wrong (the scenario/contract set) — the
contradiction hasn't changed, but which side is the defect is no longer ambiguous.

---

# Impact On A

**A is not weakened — it's confirmed more sharply than before, and its design is validated by
exactly this reassessment.** The contradiction between `out_of_scope` and the scenario/contract set
was real regardless of ground truth, and remains real now that ground truth is known. What changes
is only that we now know which side to fix (the scenarios), not whether a contradiction exists.

This is worth stating as a positive design point, not just a restated conclusion: **Mechanism A
was deliberately built to flag a disagreement without needing to adjudicate which side is right** —
a deterministic audit, not a compensating rewrite, per this project's own established distinction.
It doesn't need to know the business is "public browsing" to be useful; it only needs to notice
that `out_of_scope` and the scenario set disagree, and hand that disagreement to a human who *can*
supply the missing business fact — exactly the role you just filled. That's the mechanism working
as designed, not a lucky coincidence.

A second, sharper contradiction is now visible that the original diagnosis didn't emphasize:
**`resolved_policies` and the scenario set also disagree with each other, not just `out_of_scope`
and the scenario set.** `resolved_policies` resolves authorization as not required; the scenarios
proceed to assume authentication anyway, in the same `canopy spec` generation sequence
(`scenario_coverage_matrix` consumes `resolved_policies` as an input fact, per
`canopy-llm/src/prompts/spec.rs:973-975` — the scenarios were generated *after*, and *from*, a
resolution that already said authorization wasn't needed). This is a second, independent instance
of the same underlying shape A targets — not a new class, but evidence the contradiction is deeper
and more mechanical than "one loosely-worded field disagreeing with another." Not proposing a
mechanism change here per your standing instruction on this thread, only naming what the new fact
makes visible.

---

# Impact On B

**B is not weakened either — and the new fact sharpens exactly what B is for, rather than
undermining it.** The original framing risked reading as "B matters because the resolution was
wrong." That was never quite right, and the new fact makes the more precise version explicit:
**B matters because the resolution's justification was unsound, independent of whether its
conclusion happens to be correct.** "The story does not explicitly mention any authorization
requirements" is exactly as invalid a basis for "resolved: not required" whether or not "not
required" happens to be true — a resolution reached by treating silence as permission is a coin
flip that this specific story happened to land right-side-up. If the true business rule had been
the opposite (say, `product-010` were a private wholesale catalog requiring authentication), the
identical reasoning ("the story doesn't mention it, so it's not required") would have produced an
identically-shaped, equally unflagged, but now *wrong* resolution — with nothing in the artifact
distinguishing the lucky case from the unlucky one.

This is, if anything, a stronger argument for Mechanism B than the original framing had. A
mechanism that only fires when the resolution turns out wrong would be useless — you'd need to
already know the answer to check it. A mechanism that fires on the *unsoundness of the reasoning
itself*, regardless of outcome, is the only kind that can help before a human like you supplies the
missing business fact. That is exactly what the corrected, resolution-side design (from the
critical review) does: it flags `resolution` text that reports an absence as if it settled the
question, not text that reaches a wrong conclusion. Nothing about the redesign needs revising in
light of this new fact — it already didn't depend on knowing which conclusion was correct.

---

# Impact On C

**This is where the earlier diagnosis materially breaks, and it should not be preserved.**
`product-010` should be **removed** as a supporting instance of "capability/entity presupposed but
never established." The category's whole premise — a story legitimately, normally writing ahead of
a capability that doesn't exist *yet* but genuinely will be needed (matching
`structure-emerges-from-behavior`) — never applied here. There is no forthcoming authentication
capability this story is getting ahead of; the requirement was never legitimate in the first place.
"Presupposes an unbuilt capability" and "presupposes an invented, incorrect requirement" look
identical from inside the artifacts alone (both show a behavior depending on something that
doesn't exist elsewhere in the project) but are different in kind, and only a real business fact —
not any mechanical check — can tell them apart. That is the sharpest single lesson this
reassessment adds: **no artifact-level audit, however well designed, can distinguish a legitimate
forward reference from an invented one; that distinction is irreducibly a domain-knowledge
question.**

C as a *class* is not eliminated — `manufacturer-001`'s `Product`-relationship gap (`so_that`
names "products" with no `Product` entity ever established) is untouched by this reassessment and
remains a genuine, unresolved instance of the legitimate shape (nothing has supplied a business
fact resolving whether a `Product` entity is coming or whether the reference is spurious). What
this reassessment does is sharpen the class's own definition: C properly names *legitimate*
forward references only. A presupposed capability that turns out to be invented, once checked
against real business intent, was never a C-instance — it's a symptom of A/B (a contradiction or an
unsupported resolution), misread as C for lack of the fact that would have told them apart. This
also resolves a tension the reduction document left open (`product-010`'s severe C-instance always
co-occurred with A and B, while `manufacturer-001`'s clean C-instance never produced a demonstrated
defect) — that pattern now has an explanation: the "severe" instance was never really C at all.

---

# Does Mechanism B Still Have A Confirmed Target?

**Yes — the same one, and arguably a better-understood one than before.** `product-010`'s
`authorization` resolution remains the confirmed instance: its `resolution` text ("the story does
not explicitly mention any authorization requirements") is unchanged, still reads as a report of
absence rather than a citation of a positive fact, and still fails the corrected,
resolution-side check design regardless of the new business fact. The new fact doesn't change
whether the mechanism should fire here — it changes *why* it matters that it does. This is not "B
remains valid but `product-010` is no longer the best example" (one of the offered outcomes) —
`product-010` is, if anything, a *sharper* example now, since it demonstrates the mechanism catching
unsound reasoning that happened to reach the right answer, which is a harder and more convincing
case for the mechanism's value than catching reasoning that happened to reach a wrong one.

Selected outcome, stated against the options you offered: **A and B both remain independently
strong — with C specifically weakened as an explanation of `product-010` (not eliminated as a
class), and a sharper, second contradiction (`resolved_policies` vs. scenarios) now visible within
A's own territory.** Not "another interpretation" beyond what was offered, but a more precise
version of "A confirmed, B weakened" and "A and B both remain strong" combined incorrectly if taken
as mutually exclusive — the accurate synthesis is that neither A nor B weakens, only C's
applicability to this one story does.

---

# Recommended Next Step

No mechanism change to A is proposed here, consistent with this reassessment's own scope — the
second contradiction (`resolved_policies` vs. scenarios) is recorded as a sharper instance of the
same class A already targets, not a new requirement for the already-implemented Checklist 4.
Mechanism B's corrected, resolution-side design (from the critical review) remains the recommended
next implementation, unchanged by this reassessment — nothing here argues for revising its target
or its evidence base, only for stating more precisely why it matters. The one concrete update
worth making to the existing record: `docs/design/product-010-story-readiness-failure-diagnosis.md`
§2.3 and `docs/design/story-readiness-reduction-to-fundamental-blockers.md`'s C-independence
argument should both note, the next time either is touched, that `product-010`'s C-classification
has been superseded by this document — not silently, and not by rewriting the original reasoning,
which was sound given what was known at the time it was written.
