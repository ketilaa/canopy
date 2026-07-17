# Exploration as Backlog Discovery vs. Specification Enrichment

Status: hypothesis evaluation only, extending `docs/design/missing-concern-categories-inventory.
md`. No solution, workflow, or UX proposed. Tests whether the evidence supports "the most valuable
outcome of post-intent exploration is discovering adjacent backlog items" more strongly than "the
most valuable outcome is a richer specification for the current story."

Date: 2026-07-17

---

# Evidence Supporting the Hypothesis

- **The domain-boundary/story-vocabulary discrepancy — the single most robustly evidenced
  recurring category in the entire investigation chain — is, by its own nature, evidence of a
  missing *story*, not a missing *specification detail*.** `manufacturer-001`'s own `so_that`
  names `Product`; `order-001`'s names both `Product` and `Order`. In neither case does the
  correct treatment (per `structure-emerges-from-behavior`'s own validated logic) involve
  inventing a `Product` schema inside the *current* story — the concept being referenced implies
  a different story about a different capability that simply hasn't been written yet. This is not
  a speculative reading; it is the same conclusion the domain-boundary-hypothesis-assessment
  already reasoned toward independently.
- **Approval/Accountability concerns decompose naturally into a distinct actor performing a
  distinct action** — "is there an approval step" implies an approver, with their own trigger and
  behavior, not a clause added to the registrant's own story.
- **Downstream-consumer concerns imply a different capability entirely** — "who consumes
  `ManufacturerRegistered`" only has a real answer once whatever consumes it (a notification
  recipient, an integration partner) is itself described as its own story.
- **Lifecycle actions beyond creation are directly, independently supported by Canopy's own
  existing design rule**, not merely by this chain's inference: `intent`'s own prompt already
  states "One intent action = one story. Do not decompose a single action into sub-steps." A
  story about *registering* a manufacturer was never meant to also cover *deactivating* or
  *merging* one — those are supposed to be separate stories by Canopy's own existing architecture.
  When Product-Portfolio and growth-retention-shaped concerns surfaced these lifecycle actions,
  they were pointing at exactly the kind of thing this rule already treats as backlog, not spec
  content — the gap is in *recognizing* this at spec-review time, not in the underlying design
  principle.

# Evidence Against (or Complicating) the Hypothesis

- **Several concerns were shown to have real, measured, causal value as specification
  enrichment, not backlog signals.** The Role Meaning Value Experiment's entire result — role
  identity feeding a citation-backed `authorization` resolution *within the same story* — is
  direct, controlled evidence that at least one category of "missing concern" is correctly
  addressed by enriching the current specification, not by spinning off a new story. If exploration
  were valuable *only* as backlog discovery, this demonstrated, causal result would have nowhere to
  fit.
- **The methodological caveat from the prior inventory still applies.** Approval, downstream
  consumers, and architectural ownership each appeared only once, in the one experiment structured
  to surface them. Their story-shapedness is plausible and consistent with Canopy's own design
  rules, but it has not been independently replicated the way the domain-boundary discrepancy has.
- **Some concerns resist a clean story/policy split.** A return-window exception, for instance,
  could correctly be either a business-rule elaboration ("unless X condition, then Y" — policy-
  shaped) or a distinct story ("a support rep grants an exception" — story-shaped), and nothing in
  the evidence gathered resolves which shape fits which instance. The hypothesis, stated as a
  general claim, risks overstating how cleanly concerns sort.

---

# Concern Categories That Look Story-Shaped Rather Than Policy-Shaped

- **Domain Boundary / Story-Vocabulary Discrepancy** — strongest case; each instance points at a
  concept (`Product`, `Order`) that would need its own story to be properly described, not a field
  added to the current one.
- **Approval / Accountability** — a distinct actor (an approver) performing a distinct action
  (approving), naturally expressible as its own story.
- **Downstream Consumers** — implies a distinct capability/actor (whoever consumes the event) that
  would need its own story to specify what it actually needs.
- **Lifecycle actions beyond creation** (deactivation, merging duplicate records, an exchange
  offered instead of a refund) — each is a distinct action, and Canopy's own existing "one action,
  one story" rule already treats distinct actions as separate stories by design.

# Concern Categories Already Handled by Existing Mechanisms

- **Authorization** (once role identity is supplied) — resolves within the current story's own
  `authorization` checklist area; the Role Meaning Value Experiment demonstrated this directly.
- **Business-Rule / Uniqueness Scoping** — already a working, enumerated checklist area, confirmed
  by direct code reading; its recurrence across both stories reflects the mechanism being exercised
  often, not a gap.
- **Entitlement Ownership** (`risk_averse`'s verification requirement) — resolved within the
  current story via `uniqueness`/`consistency`, the cleanest measured downstream effect of any
  persona fact tested outside Role Meaning itself.
- **Role Semantics** — already reduced, through direct evidence, to a classification that feeds the
  current story's own `authorization` resolution; it does not spawn a separate story.
- **Retention / Timing** (return-window rules, in their basic form) — already an enumerated
  checklist area.

---

# Implications for Treating the First Story as Implementation-Ready

The evidence points at a distinction the current pipeline does not draw: **"this story's own
specification is internally complete" and "this feature area is ready to build" are different
claims**, and Stage 0's completeness check only ever asks the first one. A story can pass Stage 0
with zero gaps — `entity_schema` complete, every checklist area resolved or correctly deferred,
every scenario covered — while several of the concerns raised against it (an approval step, a
downstream notification consumer, a deactivation capability, a referenced-but-unwritten `Product`
story) remain entirely absent from the backlog, not merely unresolved within the current story.
Nothing in the evidence gathered suggests these absences would ever surface as a *specification*
gap, because they are not gaps in the current story at all — they are stories that do not yet
exist. A pipeline that only checks the former cannot, by construction, notice the latter.

---

# Verdict: Backlog Discovery vs. Specification Enrichment

**The evidence does not support one hypothesis dominating the other outright — it supports a
genuine split, cutting cleanly along the story-shaped/policy-shaped line established above.**
Concerns that are policy-shaped (authorization, uniqueness, entitlement ownership, role semantics)
have real, in some cases causally-demonstrated, value as specification enrichment, and the
evidence for that value is not weaker than the evidence for backlog discovery — in the Role
Meaning case, it is the most rigorously *tested* finding in this entire chain. Concerns that are
story-shaped (domain-boundary discrepancy, approval, downstream consumers, lifecycle actions) have
no analogous specification-enrichment value at all — enriching the current story cannot correctly
capture them, by the same reasoning `structure-emerges-from-behavior` already established.

**What can be said with confidence**: the *single best-evidenced* finding in the whole
investigation chain — the domain-boundary discrepancy, replicated independently across two stories
by direct artifact inspection rather than persona judgment — is a backlog-discovery-shaped finding,
not a specification-enrichment one. If forced to name which value proposition currently rests on
the strongest evidence, it is backlog discovery, specifically because of this one category's
unusually clean replication. But this does not make specification enrichment a weaker or
lesser-evidenced *kind* of value overall — it makes it a *narrower* one, correctly scoped to a
smaller set of concerns (chiefly authorization and uniqueness/ownership) that this chain has
already shown belong inside the current story, not outside it.
