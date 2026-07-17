# Auditing the Minimal Discriminating Experiment

Status: self-audit of `docs/design/role-meaning-collection-strategy-narrowing.md`'s own proposed
experiment, against the sharpened framing that the surviving uncertainty is narrowly "can a real
user reliably provide a Role Meaning fact" — not which collection strategy wins. No mechanism
redesign, no implementation, no UI. Finds the prior proposal over-promised what a single event can
discriminate, and simplifies accordingly.

Date: 2026-07-17

---

# Question 1 — What Assumptions Does the Proposed Experiment Still Make?

Listed exhaustively, then classified:

1. That a real dogfooding session will naturally introduce a new role within a usable timeframe.
2. That a *single* naturally-occurring role-introduction event can be split into "one of two
   conditions" (skippable / non-skippable) for comparison purposes.
3. That recording the resulting answer into two storage channels simultaneously (ADR,
   `Role::Described`) is methodologically clean and doesn't itself confound anything.
4. That "unresolved," in the forced condition, is genuinely perceived by a real user as a low-cost,
   honest option rather than an obstacle being worked around.
5. That the mechanism which made Policy Discovery's citation requirement work (a checkable, named
   source) has *some* functioning equivalent for a fact type with no external source to cite —
   i.e., that "forcing" can even be given the same shape for Role Meaning that it had for policy
   resolution.
6. That `Role::Described`'s existing wiring (`stories_from_intent_prompt` rendering a role's
   description into later context) will actually be exercised — which requires not one but *two*
   natural events: the original role introduction, and a later, separate story that happens to
   reuse the same role.
7. That one observation is sufficient to say anything at all — the same n=1 caveat every
   experiment in this chain has already had to carry, inherited here without restatement.

**Already evidenced:**
- Assumption 3's core premise (that the ADR channel, once populated, is read by the authorization
  checklist and produces a citable resolution) — directly confirmed by the Role Meaning Value
  Experiment's controlled result.
- The precedent behind assumption 4's concern — that an optional, skippable prompt gets skipped —
  is directly evidenced (`init`'s bootstrap prompt, 0 of 1).

**Weakly evidenced:**
- Assumption 5 (that "enforced cost" changes behavior in general) is well evidenced — for a
  *different* fact type (Policy Discovery). Its *transfer* to Role Meaning specifically is
  untested; the mechanism's applicability, not its existence, is what's weak.
- Assumption 6's wiring half (`stories_from_intent_prompt` does render a description when present)
  is confirmed by direct code reading. Whether that rendered content is actually *used* by the
  model in a way that matters is untested — the same wiring-vs-value distinction this chain has
  already drawn elsewhere.

**Completely untested:**
- Assumption 1 (occurrence timing) — no evidence bears on this at all; it's an operational
  precondition, not a claim about the mechanism.
- Assumption 2 — this is the assumption that turns out not to hold, addressed directly in
  Question 2 below.
- Assumption 4's actual claim (that a real human, live, treats "unresolved" as genuinely low-cost)
  — nothing in this chain has ever tested a live human choosing between options in an interactive
  session for *any* fact type; every prior "forced choice" result was measured via repeated,
  automated LLM calls, not a real person at a terminal.
- Assumption 5's specific mechanical question — whether *any* enforced-cost shape is even
  definable for a sourceless classification — is not just untested, it may be structurally
  undefined; nothing in this chain has proposed what a "citation" would even mean for role
  identity.

---

# Question 2 — Does the Experiment Actually Discriminate, or Does It Conflate?

**It conflates.** Stated plainly rather than defended: the prior proposal bundles at least three
separable uncertainties into "one event," and only one of them can actually be resolved by a single
occurrence.

- **Whether a human responds at all** (the highest-value uncertainty, per this reassessment) —
  genuinely testable from one event, in one condition.
- **Whether skippable vs. forced changes the *rate* of response** — structurally cannot be tested
  by a single event, because a single occurrence is only ever in one condition. Comparing "what
  happened this one time" against "what would have happened under the other condition" is not a
  comparison; it's a hypothetical. This isn't a matter of statistical power being low — it's that
  the design as stated doesn't produce two comparable observations at all.
- **Whether the ADR channel and `Role::Described` differ in downstream consumption** — partially
  testable from the same event (both channels can be populated from one real answer, and each
  checked for consumption independently), but the `Role::Described` half additionally requires a
  *second*, separate, later natural event (a story reusing the same role) before it can be
  observed — compounding the occurrence assumption rather than resolving cleanly alongside the
  first two questions.

So: the storage-channel comparison is a legitimate "two readings of one answer" design and survives
scrutiny. The elicitation-method comparison (Direct vs. Forced) does not — it was framed as
discriminable by a single event when it structurally requires at least two.

---

# Question 3 — Can the Design Be Simplified Further?

Yes, directly following from Question 2's finding. Since the elicitation-*method* comparison cannot
be resolved by one event anyway, the honest simplification is to **stop trying to make one event do
that job**, and instead point the single available occurrence at exactly the uncertainty that *can*
be resolved by it — whether a real human answers at all — while still getting the storage-channel
reading for free, since that part of the design was never actually conflated.

**Simplified design**: on the next naturally-occurring new role, ask the question in exactly **one**
condition — the forced-but-honestly-deferrable shape, not both. This is a deliberate choice, not an
arbitrary one: the *skippable* condition's expected behavior already has a real, if imperfect,
existing reference point (`init`'s 0-of-1 precedent) that can serve as an informal baseline without
needing to be re-run fresh. Spending the one available natural occurrence on the condition with **no**
existing reference point (Forced Classification) extracts strictly more new information than
splitting it, or than re-testing the condition already partially known. Record the resulting answer
(if any) through both channels, exactly as before — this part of the design needed no change. A true,
controlled Direct-vs-Forced comparison, if ever wanted, would require its own separate pair of
events later, not a fraction of this one.

This is smaller than the original design in a specific, principled sense: it removes an axis the
original design couldn't actually deliver on, rather than trimming something that worked.

---

# Question 4 — What Would Constitute a Decisive Result?

Per outcome, what it would actually teach — not merely whether it counts as "success":

- **User answers with a definite classification.** Resolves the core surviving uncertainty
  affirmatively for the first time in this entire chain: a real human, live, can and will supply
  this fact. Every prior positive result in this chain used pre-authored, programmatically-injected
  facts; this would be the first genuine existence proof of live elicitation working at all.
- **User explicitly chooses "unresolved."** Arguably as valuable as an answer, not a lesser result:
  it demonstrates real engagement with the mechanism (the user considered the question and made a
  deliberate choice) rather than silence, and directly validates the specific design premise that
  "unresolved" reads as a legitimate, low-cost option rather than an obstacle — a distinct claim
  from whether the user has an opinion at all.
- **User ignores/skips despite the mechanism being non-skippable.** The single most consequential
  possible outcome, exactly as flagged before this audit: it would extend the existing 0-of-1
  non-engagement finding to a *second*, structurally stronger mechanism, suggesting the entire
  family of live-elicitation strategies may not function in this pipeline regardless of framing —
  a finding that would call the whole surviving strategy set into question, not just one member of
  it.
- **User provides free text instead of selecting from the closed set.** A genuinely new class of
  finding not previously surfaced anywhere in this chain, because every closed-set fact tested so
  far was authored by the experimenter, never actually produced by an unconstrained human. This
  would reveal friction between what's easy to *consume* (a closed-set value, per the Value
  Experiment) and what's natural for a real person to *produce* — a distinct failure mode from
  non-response, and one this chain has no prior evidence about either way.
- **The consumer uses the fact** (the ADR-recorded copy resolves `authorization` citably, or the
  `Role::Described` copy visibly reaches a later story's context). Confirms the Value Experiment's
  controlled result generalizes to organically-elicited content, not just carefully-phrased,
  experimenter-authored input — closing the one gap in that result's evidence that this chain has
  already named honestly.
- **The consumer ignores the fact despite it being present and well-formed.** A different, more
  sobering finding than any seen so far: it would mean the mechanism *can* work (already shown) but
  does not *reliably* work with real, unpolished human phrasing — separating "the citation mechanism
  exists and functions" from "it functions robustly against whatever a real person actually types,"
  a distinction this chain has not yet had the evidence to draw.
