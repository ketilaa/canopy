# The Role Meaning Value Chain — Final Evidence Map

Status: final synthesis, drawing only on findings already established across this investigation
(Role Meaning Value Experiment, Human Insight Process Experiment Phases 2–3, Exploration Phase
Reassessment, Operational Facts Worth Establishing, Why Role Meaning Succeeded, Collection Strategy
Narrowing, Classification vs. Authorship). No implementation, UX, or new experiment proposed —
this maps the chain as currently evidenced, so the next uncertainty can be chosen deliberately.

Date: 2026-07-17

---

# The Chain

```
Human notices role → Human provides fact → Fact is stored → Fact is consumed → Downstream artifact changes
```

---

## Link 1 — Human Notices Role

**A necessary framing correction before evaluating this link**: the strategies still under
consideration (Direct Question, Forced Classification, ADR-Driven Declaration — per the Collection
Strategy Narrowing) are all *tool-initiated* — the question is posed automatically at the moment a
role is registered, not left for a human to spontaneously notice unprompted. Under that framing,
this link is mostly a **mechanical detection** question ("does the tool correctly identify the
moment to ask"), not a human-attention question.

- **Evidence for**: the mechanical half is well evidenced — the Role Semantics Investigation
  pinpointed the exact code path (`intent`'s automatic per-story registration) with precision,
  confirmed by direct reading, not inference.
- **Evidence against**: the *human-noticing* half — relevant only to a strategy already ruled out
  (Review-Time Confirmation) — is weak. The Product-Owner Perspective Experiment found only 1 of 5
  review lenses caught the role-semantics ambiguity, despite the relevant text ("the manufacturer
  representative is authenticated") appearing in every single scenario. This is real evidence
  spontaneous noticing is unreliable — but it bears on a discarded strategy, not the ones still in
  play.
- **Confidence**: High, for the mechanical-detection framing that actually matters to the surviving
  strategies. Low, for spontaneous human noticing — but that version of the question is no longer
  live.

## Link 2 — Human Provides Fact

- **Evidence for**: essentially none. No experiment in this entire chain has ever observed a real,
  live human producing an operational fact of any kind. Every fact tested — closed-set or prose,
  successful or not — was authored by the experimenter and injected programmatically.
- **Evidence against**: the one directly relevant real precedent (`init`'s optional role-
  description prompt) was skipped 100% of the time it was offered (0 of 1). This is a clean,
  specific, negative data point, not an absence of evidence.
- **Confidence**: Very low. This is the single most explicitly and repeatedly named open
  uncertainty across the last several documents in this chain — not a new observation, a
  confirmed standing gap.

## Link 3 — Fact Is Stored

- **Evidence for**: both real candidate storage locations are confirmed working at the mechanical
  level. `Role::Described { name, description }` exists in `canopy-core`, built for exactly this
  purpose. The ADR channel (`existing_adrs`) was directly exercised, successfully, in both the
  controlled Value Experiment and real Phase 3 dogfooding sessions — real files were written to
  disk and read back without incident.
- **Evidence against**: none of substance. No experiment in this chain has ever found a storage
  failure.
- **Confidence**: High. This is the most mechanically solid link in the entire chain.

## Link 4 — Fact Is Consumed

- **Evidence for**: the Value Experiment's own controlled result — `authorization` moved from
  correctly unresolved to citation-backed `resolved` in 2 of 3 tested conditions, with the
  `evidence` field directly reproducing the supplied fact.
- **Evidence against**: the third condition (`affiliated`) failed to be consumed, with the cause
  left genuinely unresolved between content-sensitivity and ordinary sampling noise. The
  persona-policy facts (Phases 2–3) show consumption is unreliable once a fact isn't narrowly
  scoped and concrete. And the *second* real storage channel — `Role::Described` — has never been
  tested for consumption at all; its wiring into `stories_from_intent_prompt` is confirmed by code
  reading, but whether that rendered content is ever actually used downstream is untested.
- **Confidence**: Medium. Real, controlled, positive evidence exists for one specific pathway
  (ADR channel + closed-set classification), but it is a single pass, not repeated for
  regeneration stability, with one unresolved failure out of three conditions, and a second real
  channel entirely unverified.

## Link 5 — Downstream Artifact Changes

- **Evidence for**: directly demonstrated, not inferred — a new scenario type (an actor-lacks-
  required-role rejection) appeared in lockstep with the resolved `authorization` policy, in the
  same two conditions that resolved it, and in no others. This is a clean, causally-linked,
  measured change in a real downstream artifact.
- **Evidence against**: tested for exactly one downstream artifact category (specification
  scenarios), on one story, in a single pass. Regeneration stability was never checked for role
  meaning specifically — a gap already named honestly in the Why Role Meaning Succeeded analysis,
  not new here. The broader persona facts' analogous downstream changes *were* shown to evaporate
  under regeneration (Phase 3), though that result was never re-run for role meaning itself.
- **Confidence**: Medium. The causal mechanism is demonstrated once, cleanly; its durability across
  repeated regeneration is an open, previously-flagged unknown, not a new one.

---

# Strongest Link

**Link 3 — Fact Is Stored.** The only link in the chain with no negative evidence of any kind, and
directly confirmed mechanically twice, in different contexts (a controlled experiment and real
dogfooding sessions).

# Weakest Link

**Link 2 — Human Provides Fact.** Not by a narrow margin: every other link has at least one
positive, controlled result to point to, even where confidence is only medium. Link 2 has zero
positive instances anywhere in this chain and one specific, clean negative data point. It is also
the link every recent document in this investigation has independently converged on naming as the
central open question — this synthesis confirms rather than newly discovers that convergence.

# Highest-Value Next Uncertainty

**Whether a real human, asked directly, provides an operational fact at all — Link 2, unchanged
from the prior audit's own conclusion.** Every other link either already has supporting evidence
(3, 4, 5) or has been reframed to no longer depend on the weaker version of itself (1). Link 2
remains the one point in the entire chain with no observed positive instance, and it gates
everything downstream of it: Links 3 through 5 cannot be exercised by a real, human-sourced fact
until Link 2 is resolved at least once.
