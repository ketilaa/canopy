# Domain Exploration — Version 1 Design

Status: design-level recommendation only. No prompts, no commands, no workflow changes, no
implementation. Answers one question: what is the smallest domain-exploration capability that
would provide real, checkable value while disrupting the existing pipeline the least?

Date: 2026-07-16

Builds directly on `docs/design/canopy-assisted-domain-exploration-vision.md`. Re-grounds every
candidate against the same evidence base that vision cited — `docs/design/product-owner-
perspective-experiment.md`, `docs/design/exploration-enumeration-gap-investigation.md`,
`docs/design/role-semantics-investigation.md`, `docs/open-questions/{role-semantics-
explicitness,domain-boundary-explicitness}.md`, `docs/principles/{structure-emerges-from-
behavior,unresolved-decisions-become-explicit-decision-points,reserve-the-model-for-genuine-
ambiguity}.md` — rather than starting from the vision's conclusions as given.

---

# Candidate Exploration Slices

Five candidates, drawn directly from the vision's own Exploration Outputs list, stated neutrally
here before any evaluation:

1. **Role meaning capture** — ask what a newly-introduced actor label actually denotes (internal
   operator vs. external party, or another distinguishing classification), at the moment a role is
   first registered.
2. **Identity/uniqueness clarification** — ask what makes two instances of a newly-introduced
   entity the same real-world thing, before any scenario commits to an assumption about it.
3. **Forward-reference detection** — flag when a story's own language (`want`/`so_that`) names a
   concept absent from domain vocabulary, without materializing it.
4. **Hot-spot capture** — a general-purpose artifact for marking any unresolved meaning-level
   uncertainty, broader than a Decision Point.
5. **Domain glossary enrichment** — a cumulative, human-curated vocabulary layer on top of
   `domain_registry.yaml`'s existing entity/event list.

---

# Evaluation Of Each Candidate

**1. Role meaning capture.** Evidence: the strongest of the five by a clear margin. Three
independent investigations converge on it — the Product-Owner Perspective Experiment found the
ambiguity from a terminology/beneficiary angle; the Exploration Enumeration Gap Investigation
checked it against current code and found no competing explanation (unlike uniqueness and
authorization, which turned out to already be enumerated, this concern survived active elimination
attempts); the dedicated Role-Semantics Investigation traced it through all six pipeline stages and
found a real counter-mechanism (`Role::Described`) that exists but is bypassed by the one code path
that actually creates roles. Speculation: minimal — every claim is grounded in a specific file, a
specific code path, or a specific timestamp comparison. Smallest interaction: a single, bounded
question, fired once per newly-registered role — and this project's entire real history has
produced exactly one role, ever, so the current blast radius of *not* asking is already fully
known. Disruption: the lowest of the five — the type already supports storing an answer; no new
artifact family is needed even for the smallest version.

**2. Identity/uniqueness clarification.** Evidence: real (the Product-Owner Perspective
Experiment's duplicate-name finding), but weaker as a candidate for *new* capability than it first
appears — the Exploration Enumeration Gap Investigation already found "uniqueness" is one of six
areas the business-policy checklist explicitly enumerates today, and that `manufacturer-001`'s own
gap is better explained by that story's artifact predating the checklist mechanism than by any
current absence. The honest next step for this concern is a live-behavior check of an *existing*
mechanism, not a new exploration capability. Building a new slice here risks re-litigating a
question this project has already answered, or duplicating a checklist that already works when it
runs — a direct instance of the "same rule reaching the model twice" pattern this project's own
house style treats as a defect.

**3. Forward-reference detection.** Evidence: real and specific (the undefined `Product`
relationship), but entangled with a genuine design tension the vision document already identified:
`structure-emerges-from-behavior`'s domain-extraction step *deliberately* excludes concepts named
only in a story's purpose clause, for reasons this project has independent, validated evidence for.
Building this slice first would require resolving that tension — deciding how to flag without
anticipating — before the slice could even be scoped safely. More design judgment required, not
less; a poor fit for "smallest possible slice."

**4. Hot-spot capture.** Evidence: conceptually attractive (the vision names it as the sharpest
Event-Storming translation), but it was never independently *found* as a gap by any investigation —
it was proposed as an architecture for holding other findings, not discovered as a symptom in real
data the way role semantics was. It is also the broadest candidate in scope: a general-purpose
artifact needs its own shape, its own review treatment, and a policy for what qualifies, all before
a single real question is ever captured in it. Better understood as a *container* a narrower slice
might eventually want, not a slice in its own right.

**5. Domain glossary enrichment.** Not an independent candidate on inspection — it is what
accumulates automatically if role meaning (or identity, or any other) capture happens consistently
over time, using the `Described` shape `domain_registry.yaml`'s types already support. Treating it
as its own initiative would mean building an aggregation mechanism ahead of having anything real to
aggregate.

---

# Recommended MVP

**Role meaning capture, and nothing else.**

**Problem addressed.** The one pipeline path that actually creates a role today —
`intent`'s automatic per-story registration — records an actor label as an established fact with no
question asked about what it denotes, and no human gate at all. The type that could hold an answer
(`Role::Described`) already exists; nothing currently reaches it from this path.

**Evidence supporting it.** Independently surfaced by three different methods (a simulated multi-
persona critique, a code-level enumeration audit, and a dedicated stage-by-stage trace), and it is
the one candidate among the five that survived an active attempt to explain it away — the same
elimination process that correctly downgraded uniqueness and authorization to "already handled,
stale artifact" found no equivalent alternative explanation here.

**Expected value.** Closes the sharpest, best-evidenced gap this project's last five investigations
converged on. Because role identity sits logically prior to authorization (per the Role-Semantics
Investigation's own §4), a confirmed role definition gives every downstream authorization/business-
rule question — which already has a working mechanism — a settled foundation to reason from,
instead of an unexamined assumption. It also establishes, for the first time, a live (not just
structurally-argued) test of whether "ask a bounded question, preserve whatever answer comes back"
generalizes past business policy to a new category of meaning question.

**Expected risks.** The clearest one is not hypothetical: `init`'s existing, structurally similar
optional description prompt ("leave blank to skip") has already been skipped for the one real role
this project has ever produced. A version of this MVP that only adds an equivalent optional prompt
risks reproducing that exact non-result. This is worth stating plainly as a live design tension for
whoever eventually designs the actual mechanism, not resolved here — the Policy Discovery precedent
(`unresolved-decisions-become-explicit-decision-points`) already shows an escape hatch alone doesn't
change behavior without some enforced cost attached to the alternative, and that lesson is directly
relevant, not just adjacent.

**Relationship to existing principles.** Directly compatible with `structure-emerges-from-
behavior` — this fires after a role has already emerged from an accepted story, never before one
exists, avoiding the exact upfront-elicitation shape that principle found harmful. A clean instance
of `reserve-the-model-for-genuine-ambiguity` — a role's real-world meaning is genuinely input-
dependent and not enumerable in advance, exactly the class of question that principle argues should
stay a human call. Shape-compatible with `unresolved-decisions-become-explicit-decision-points`
(a bounded question, an explicit "still unknown" preserved as legitimate) without extending that
principle's own mechanism or scope — this MVP is a new instance of the same *shape*, not a change
to that principle's existing, narrower remit.

**Relationship to existing stages.** Sits immediately after `intent`'s existing automatic role
registration, before `spec` begins — the exact point the Role-Semantics Investigation identified as
currently ungated. Does not touch `spec`'s business-policy checklist, `behaviors`, or Decision
Points at all.

---

# Expected Learning Value

**If it succeeds** (the question gets answered, not skipped, at a meaningfully higher rate than
`init`'s existing optional-description precedent): this is the first live evidence — not just
structural argument — that "ask a bounded question, preserve the answer" generalizes beyond
business policy to a genuinely different category of question. That would be a real, new data
point for extending the same pattern to identity/uniqueness timing, or eventually to ownership and
boundary questions, once those have their own supporting evidence.

**If it fails** (the question gets skipped at a rate similar to the existing optional-description
field): this is not a null result — it would be strong, direct evidence that the Policy Discovery
lesson (an escape hatch alone is insufficient without an enforced cost) generalizes past business
policy too, which is itself a valuable, symmetric finding worth having on record either way.

**What would remain unknown regardless of outcome**: whether this generalizes across differently-
worded roles beyond the one this project has ever produced (still only one or two data points after
this MVP, unless further dogfooding naturally produces more); whether asking at role-creation time
is early enough, or whether some ambiguity only becomes visible later, once `spec`'s own scenarios
are written; and whether the other four candidates evaluated above would benefit from the same
shape of mechanism this MVP validates, or need a genuinely different one.

---

# Risks

- **Question fatigue**, if scope drifts beyond one bounded question per newly-created role — this
  project has direct historical evidence this specific failure mode is real (`explore`'s own
  clarifying questions were removed for "adding friction without value").
- **A non-result indistinguishable from success**, if the mechanism's own design doesn't make
  answering meaningfully easier or more attractive than skipping — the `init` bootstrap precedent
  makes this a live risk, not a remote one, and is exactly why this document's own "Expected
  Learning Value" section treats a skip-heavy outcome as informative rather than as simple failure.
- **Scope creep toward the other four candidates** before this one has been tried and observed —
  the entire point of recommending a single slice is to avoid this; role meaning capture should be
  allowed to stand alone and be judged on its own result before any of the others are picked up.

---

# Relationship To Existing Canopy Principles

Restated compactly, since each was argued in full above: compatible with, and a clean instance of,
`structure-emerges-from-behavior` (post-emergence, not upfront) and `reserve-the-model-for-genuine-
ambiguity` (a genuinely human question, not a fact to compute or a plausible guess to generate).
Shape-compatible with, but not an extension of, `unresolved-decisions-become-explicit-decision-
points` — this MVP borrows that principle's validated *shape* (bounded question, explicit
non-answer preserved) for a new category of question that principle's own existing mechanism was
never scoped to cover.

---

# What This Does NOT Attempt To Solve

- **Identity/uniqueness clarification** — not a new gap; the honest next step is verifying live
  behavior of the checklist that already enumerates this, not building new capability.
- **Forward-reference detection (the `Product` relationship)** — genuinely entangled with
  `structure-emerges-from-behavior`'s deliberate design exclusion; needs that tension resolved
  first, which this MVP deliberately defers.
- **General-purpose hot-spot capture** — a broader artifact-design decision; a role-meaning answer
  that comes back "still unknown" can be preserved as a simple, minimal deferred marker without
  building the full generalized mechanism first.
- **Domain glossary enrichment as its own initiative** — an emergent byproduct of consistent
  capture over time, not something to build separately.
- **Bounded contexts, architecture ownership, service decomposition, aggregate design, event-
  modeling frameworks, or any large domain-modeling exercise** — none of these are touched by role
  meaning capture at all, and none of the evidence gathered so far requires them.

---

# What Success Looks Like From the User's Perspective

Still design-level — no prompt wording, no command, no schema. What a user should experience if
this MVP works, and what "working" concretely means for each part of it.

**What question should be answered?** One question, asked once per newly-introduced role: *is this
actor internal to the business — someone who acts on the business's own behalf (an employee, an
operator, an internal team) — or external to it — someone outside the business itself (a customer,
a partner, a third party, or the counterpart entity the story is even about)?* This is the single
distinction the Role-Semantics Investigation found sits underneath the authorization question and
was never asked anywhere. It is deliberately narrower than "describe this role" — a free-text
description is still available for anything beyond this (the same `Described` shape already
supports one), but the one thing that must be *forced*, per this project's own validated lesson
that bounded questions outperform open invitations, is this specific classification.

**What answers are possible?** Three, and only three, mirroring the resolved/not_applicable/
unresolved shape Policy Discovery already validated rather than inventing a new one: **internal**,
**external**, or **unresolved** — a genuine, first-class third option, not a fallback for a broken
interaction. Whichever of the two substantive answers is given, an optional free-text elaboration
can accompany it (e.g., *which* kind of external party) — but the elaboration is never a substitute
for the forced classification itself, the same way a Policy Discovery answer needs both a
classification and grounding, not grounding alone.

**What does "unresolved" look like?** Exactly as legitimate and exactly as visible as a substantive
answer — never a silently-defaulted guess, and never hidden. Concretely: a role marked unresolved
must be distinguishable, wherever roles are shown or reused, from one marked internal or external —
not merged into a generic "unknown" bucket indistinguishable from a role nobody ever asked about in
the first place. It must not be treated as a blocking gap the way an unresolved Stage 0 completeness
finding is — this MVP is deliberately lower-stakes than that — but it also must not be silently
resolved by a later stage without a human explicitly revisiting it; the same "protect what's already
recorded, even when what's recorded is 'we don't know yet'" posture `freeze-the-established-spec`
already argues for once *something* has been decided.

**How is the answer preserved?** Attached to the role itself, in the roles registry — not
per-story — so the question is asked once per role, not once per story that happens to reuse it.
Once given, a substantive answer is treated as an established fact for that role from then on, the
same way an accepted ADR is treated as established once resolved. An `unresolved` answer is
preserved too, not discarded — a later story reusing the same role should see that it is still
unresolved, so the option to revisit it stays open without ever being *forced* open again
automatically.

**How does downstream Canopy benefit?** Three ways, in order of how directly evidenced each is.
First, and most directly: the business-policy checklist's `authorization` area, which today asks
about permission "beyond the actor already being authenticated" with no way to know who that actor
is, gains a settled fact to reason from instead of an unexamined assumption — the exact dependency
the Role-Semantics Investigation's §4 already established. Second: a future review of this same
kind — another Human-Insight-Inventory-style pass, or another Product-Owner Perspective Experiment
on a new story — has real, human-confirmed ground truth to check against, rather than needing to
reconstruct the ambiguity by hand the way this entire investigation thread has had to do for
`manufacturer-001`. Third, and most speculative: if a role's internal/external status turns out to
correlate with anything `identify_architectural_questions` already reasons about (e.g., whether a
customer-facing vs. an internal-admin frontend is the right shape) — a genuine possibility, not a
claim being made here, since nothing in this MVP wires the two together.
