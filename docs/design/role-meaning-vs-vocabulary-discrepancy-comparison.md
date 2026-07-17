# Role Meaning vs. Story-Vocabulary Discrepancy — Head-to-Head

Status: evidence comparison only, extending `docs/design/operational-facts-worth-establishing.md`.
No mechanism, UX, workflow, or implementation proposed. Answers which of the two top-ranked
candidate operational facts has the strongest evidence-to-uncertainty ratio.

Date: 2026-07-17

---

## Role Meaning

- **Evidence quality**: High for the *mechanism*, low for the *sample*. The structural gap
  (`Role::Described` exists, `intent`'s automatic registration never reaches it) is grounded in
  direct code reading, not inference. The consumption claim is grounded in a controlled,
  four-condition experiment (Role Meaning Value Experiment) where only the injected context varied
  and the real, unmodified production function was called.
- **Measured downstream effect**: **Yes — the only candidate in this comparison with one.**
  `authorization` moved from a correctly unresolved baseline to a citation-backed `resolved` in 2 of
  3 tested conditions, with the injected fact quoted verbatim as `evidence`. A corresponding new
  scenario (an actor-lacks-required-role rejection) appeared in lockstep in the same two conditions
  and in no others.
- **Replication status**: The *structural gap* has never been replicated — this project has had
  exactly one real role in its entire history. The *consumption mechanism*, however, was itself
  exercised across three independent conditions in one controlled experiment, and the classification
  scheme it depends on was already revised once (three-way → four-way) after direct stress-testing
  against 20 realistic role names — meaning what's actually known here has already survived one
  round of adversarial checking, not just a single untested guess.
- **Existing consumers**: Partial-to-real. Nothing currently reads a role's classification on its
  own, but the experiment directly demonstrated the existing `authorization` citation mechanism
  consumes it once supplied through the same channel an ADR already uses.
- **Existing storage location**: Real and already built. `Role::Described { name, description }`
  exists in `canopy-core` today, unused by the one path that actually populates roles.
- **Remaining uncertainty**: Whether the gap and its effect generalize past one role and one story;
  whether the four-way classification set is itself final, given it was already revised once.

## Story-Vocabulary vs. Domain-Vocabulary Discrepancy

- **Evidence quality**: High for *existence*, absent for *value*. The discrepancy itself
  (`manufacturer-001`'s `so_that` naming `Product`, never extracted; `order-001` independently
  showing the identical pattern for `Product`/`Order`) is directly observable in real artifacts —
  not inferred, not modeled.
- **Measured downstream effect**: **None.** No experiment in this chain ever tested what happens if
  this discrepancy is surfaced — whether flagging it changes any later artifact, any human
  decision, or any generated content. The gap's *existence* is well evidenced; its *consequence* is
  entirely untested.
- **Replication status**: **The strongest replication in this entire investigation chain** — the
  same gap recurred, unprompted, across two independently-chosen stories in two different domains
  (manufacturer registration, product returns), neither designed to test the other.
- **Existing consumers**: None. No prompt, checklist area, or Decision Point mechanism checks a
  story's own language against domain vocabulary at all.
- **Existing storage location**: Partial. `out_of_scope` (a real, already-used `IntentSpec` field)
  could plausibly hold an acknowledgment, but nothing today populates it for this purpose — this is
  an available field, not a built pathway.
- **Remaining uncertainty**: Whether *surfacing* this discrepancy (the only shape
  `structure-emerges-from-behavior`'s own evidence would sanction — flagging, not resolving) would
  produce any measurable value at all. This is not a minor gap in an otherwise strong case; it is
  the central unknown.

---

## Evidence-to-Uncertainty Comparison

| | Role Meaning | Vocabulary Discrepancy |
|---|---|---|
| Gap replicated? | No (n=1) | **Yes (n=2, unprompted)** |
| Downstream effect measured? | **Yes, causally, in a controlled comparison** | No |
| Consumer exists? | Yes, demonstrated | No |
| Storage exists? | Yes, built and unused | Partial, unused |
| Central open uncertainty | Generalization across more roles | Whether the finding has any downstream value at all |

The two candidates fail on opposite axes. Role Meaning's weak point is *sample size* — everything
that has been measured points the same direction, there simply isn't much of it yet. Vocabulary
Discrepancy's weak point is *causal evidence* — the phenomenon itself is the best-replicated
observation in the whole chain, but nothing has ever shown that doing anything about it matters.

A gap in replication is a narrower, more tractable uncertainty than a gap in demonstrated value.
Generalizing an already-measured causal effect to a second instance is a smaller evidentiary step
than establishing, for the first time, that a well-replicated but purely observational finding has
any downstream consequence at all.

## Which Has the Strongest Evidence-to-Uncertainty Ratio

**Role Meaning.** It is the only one of the two candidates with a real, measured causal chain from
supplied fact to downstream artifact — the single piece of evidence this whole investigation chain
has that most directly answers "does this kind of fact matter," as opposed to "does this kind of
gap exist." Vocabulary Discrepancy has the better-replicated *observation*, but replication of an
unmeasured phenomenon does not close the same gap that a controlled causal result does — no amount
of additional replication would tell us whether the discrepancy matters if its consequence is never
actually tested. Role Meaning's remaining uncertainty (sample size) is the more tractable of the
two: it asks whether an already-demonstrated effect holds up again, not whether an effect exists at
all.
