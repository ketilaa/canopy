# Canopy-Assisted Domain Exploration — Vision

Status: design exercise only. No implementation, no prompt design, no roadmap commitment. Answers
what a domain-exploration capability should *achieve*, reasoned from first principles and grounded
in this project's own accumulated evidence, before any question of how it would work.

Date: 2026-07-16

Grounded in: `docs/design/product-owner-perspective-experiment.md`, `docs/design/human-insight-
inventory.md`, `docs/design/exploration-enumeration-gap-investigation.md`, `docs/design/role-
semantics-investigation.md`, `docs/open-questions/domain-boundary-explicitness.md`,
`docs/principles/structure-emerges-from-behavior.md`, `docs/principles/unresolved-decisions-
become-explicit-decision-points.md`, `docs/principles/exhaustive-enumeration-over-holistic-
review.md`, `docs/principles/reserve-the-model-for-genuine-ambiguity.md`, `docs/principles/freeze-
the-established-spec.md`.

---

# Vision

Every investigation this project has run in the last two days converges on the same shape of
finding, from five different methods: a review-log reconstruction, a simulated multi-persona
critique, a code-level enumeration audit, a role-semantics trace, and a domain-boundary hypothesis
test. Canopy generates *mechanics* — fields, validation, scenarios, contracts — reliably and well.
What it currently cannot generate, and does not currently ask a human to supply on purpose, is
*meaning*: what an actor label denotes, what makes two records the same real-world thing, what a
story's own language quietly assumes about the wider domain, who is accountable for a decision,
where one business concern ends and another begins.

**Canopy-Assisted Domain Exploration exists to make meaning a first-class thing Canopy asks for,
the same way it already makes recommendations a first-class thing it proposes.** Not a new
generation capability — a new *elicitation* capability, anchored to a concrete, already-described
behavior, that surfaces the specific, evidenced classes of question this project has found humans
currently contribute silently, unprompted, or not at all.

Two things this is explicitly not, stated up front because they are the two ways this vision could
most easily go wrong: it is not a big-picture, whole-domain modeling phase run ahead of any real
behavior (that already has an evidenced, medium-to-high-confidence cost —
`structure-emerges-from-behavior`'s anticipatory-over-generation finding); and it is not an attempt
to recreate Event Storming's physical, multi-person, simultaneous-sticky-note mechanism inside a
single-user CLI session (a category error about what kind of tool Canopy is, addressed directly in
§Event-Storming Mapping and §Solo And Team Usage below).

---

# Design Goals

Ranked, not merely listed — the evidence points at one goal as the connecting thread underneath
the rest, not five equally-weighted goals.

**1. Establish precise, human-confirmed meaning for the terms already in play (highest priority).**
Every other goal below is an instance of this one applied to a specific kind of term: a role name,
an identity criterion, a cross-entity reference, an ownership claim. `role-semantics-
investigation.md` found this gap in its purest form — a term (`manufacturer representative`) used
fluently for six pipeline stages with its actual referent never once confirmed. This is the goal
that, if met, makes the others tractable; the others, met without this one, would just be more
mechanics generated confidently on top of an unconfirmed foundation.

**2. Clarify actors — who they are, not just what they're called.** Directly evidenced, freshest
finding, clearest current gap (no mechanism reaches the path that actually creates a role today).

**3. Surface identity and uniqueness criteria before a scenario invents one.** Directly evidenced
(the duplicate-name finding) — and importantly, *not* a new checklist item to invent: this already
exists as an enumerated area in the business-policy checklist. Exploration's job here is *timing*,
not *coverage* — asking earlier, before a scenario's own prose commits to an assumption the
checklist would later have caught, had it run first.

**4. Flag relationships and forward references a story's own language implies but doesn't
establish.** Directly evidenced (the undefined `Product` relationship) — with a real, already-
identified tension against goal-negative-space: flagging is safe, materializing is not (see Risks).

**5. Surface candidate ownership and boundary questions, at the domain level, before architecture
decides them at the service level.** Real and evidenced (`domain-boundary-explicitness.md`), but
currently the least concretely testable of the five — this project has never had a second entity to
observe a real ownership question against.

**Deliberately not a primary goal**: rediscovering business rules the existing policy checklist
already enumerates (uniqueness, defaults, retention, authorization, idempotency, consistency).
Exploration's relationship to that checklist is explained in §Relationship To Existing Canopy
Stages — duplicating it would be the exact "same rule reaching the model twice" failure this
project's own Prompt House Style already names as a defect, not a safety net.

---

# Desired User Experience

Reasoning from first principles about what the *experience* — not the mechanism — should feel
like, for the single most common case this tool actually serves: one Product Owner, one accepted
story, no second human in the loop.

The experience should feel like being asked a small number of sharp, unavoidable questions at the
exact moment a term first enters the conversation — not a separate, dreaded "fill out the domain
model" phase, and not an open-ended "anything else to add?" invitation. `exhaustive-enumeration-
over-holistic-review`'s own validated finding — bounded, explicit, one-at-a-time questions
outperform open-ended review for both models *and*, on the evidence of `init`'s own optional
description prompt going unused for this project's one real role, quite plausibly for humans too —
argues directly against an open invitation and for a small, forced, bounded set.

The experience should never feel like an interrogation about the whole business. It should feel
proportionate to what just got introduced: a new role gets asked what it is; a new entity gets
asked what makes two of it the same; a story whose own language reaches outside the current
vocabulary gets a single flag, not a demand to resolve it now. The unit of exploration is the
concept just introduced, not the project as a whole.

The experience should make declining an answer as legitimate and as visible as giving one — the
same shape `unresolved-decisions-become-explicit-decision-points` already validated for business
policy (an "unresolved" classification, backed by an enforced cost for the alternative, is not a
failure state) should extend to meaning-level questions too. A Product Owner who genuinely doesn't
know yet whether "manufacturer representative" means an external or internal actor should be able
to say so, explicitly, and have that non-answer preserved and visible — not forced to guess, and
not silently defaulted past.

---

# Exploration Outputs

**Valuable:**
- A confirmed (or explicitly still-open) **definition per role** — what kind of actor it is, not
  just its name. The direct fix for the specific gap `role-semantics-investigation.md` found:
  `Role::Described` already has a field for this; nothing currently asks the targeted question that
  would fill it for the path that matters.
- A confirmed (or explicitly still-open) **identity/uniqueness statement per entity** — what makes
  two records the same real thing. Feeds the existing policy checklist rather than replacing it;
  see §Relationship To Existing Canopy Stages.
- A **reference log**: concepts a story's own language names but that aren't yet in domain
  vocabulary, each with a human decision attached — acknowledge and defer (the existing
  `out_of_scope` mechanism, already real and already used twice in `manufacturer-001`'s own spec),
  or flag as worth investigating now. Not a new entity, not a new relationship — a flag plus a
  choice.
- **Candidate reactive-policy notes**: "if this happens, what else in the business might care?" —
  captured as a domain-level observation, deliberately *not* an event-broker/topic decision (that
  stays spec's job entirely).
- **Hot spots**: a general-purpose marker for genuine, currently-unresolved uncertainty —
  deliberately broader in scope than a Decision Point (which today only ever fires for a
  *recognized* business-policy question derived from already-extracted behaviors). A hot spot can
  be about naming, meaning, scope, or ownership — categories the current Decision Point taxonomy
  (`business` / `technical` / `behavioral-ambiguity`) doesn't cleanly cover, evidenced directly by
  role semantics not fitting any of the three.
- A **cumulative domain glossary** — the human-confirmed layer on top of `domain_registry.yaml`'s
  existing entity/event list, using the `Described` shape that type already supports and that this
  investigation found underused specifically because nothing currently asks the question that
  would populate it.

**Unnecessary, and worth naming as explicitly excluded rather than left ambiguous:**
- Formal aggregate/bounded-context diagrams — an architecture artifact, premature at this stage,
  and not this project's evidenced gap (nothing in the last five investigations found a *diagramming*
  problem; they found a *meaning-capture* problem).
- A whole-business event/command timeline generated ahead of any accepted story — the single
  clearest way this capability could recreate the anticipatory-extraction failure mode already
  found and fixed once in this project's own history.
- Any output that commits to a service name, a technology, or an event-broker topic — that is
  `identify_architectural_questions`'s job; exploration supplying an opinion here would be a second,
  competing source for the same decision.
- A formal, resolved bounded-context declaration — exploration's job is to accumulate *candidate
  signals* toward this; deciding it is squarely `domain-boundary-explicitness.md`'s own eventual
  resolution, not a byproduct of exploration itself.

---

# Relationship To Existing Canopy Stages

**vs. `intent`.** `intent` decomposes a raw behavioral statement into stories, with domain
entity/event extraction and role registration running automatically, no human gate. This is exactly
where the role-semantics gap lives today (`intent.rs`'s automatic `Role::Simple` registration).
Exploration is not a replacement for `intent`'s mechanical decomposition — it is what happens
*immediately after* a story is accepted, before it proceeds, asking the smaller, sharper questions
`intent`'s own no-human-gate paths currently skip. It is anchored to one already-accepted story, not
to the raw intent statement — respecting `structure-emerges-from-behavior`'s own finding that
structure derived from a concrete, already-described action produces better results than structure
elicited abstractly before one exists.

**vs. `spec`.** `spec` is where architecture and the business-policy checklist live. Exploration
must not duplicate the checklist's six areas — its relationship to `spec` is that of upstream
grounding, not parallel coverage. A confirmed identity criterion, a clarified role definition, and
a flagged forward-reference are all *inputs* `spec`'s own generation should be able to consume
(the same way it already consumes ADRs and domain vocabulary as context) — not a second opinion
competing with `spec`'s own service-ownership or tech-stack proposals, which remain entirely
`spec`'s job.

**vs. `behaviors`.** No relationship at execution time — by the time `behaviors` runs, the
specification is meant to be a stable foundation the rest of the pipeline builds on without
reinterpreting it, the same protective posture `freeze-the-established-spec` already argues for at
the code-generation layer, generalized here to the specification layer: once exploration's outputs
are confirmed, they should be treated as established fact for everything downstream, not silently
revisited.

**vs. Decision Points (Stage 2).** Decision Points are narrower and later: they only ever fire for
a business-policy question already recognized as unresolved, derived from behaviors that don't yet
exist at exploration time. Exploration's hot spots are a broader, earlier category — some hot spots
may later crystallize into a formal Decision Point once behaviors exist to attach them to; others
(role meaning, naming, scope) may never fit that taxonomy at all, and don't need to. The two
mechanisms address adjacent but genuinely different classes of "unresolved," at genuinely different
points in the pipeline — worth stating plainly rather than assuming one subsumes the other.

---

# Event-Storming Mapping

**Translates well:**
- **Actors** — the single cleanest match. Event Storming's practice of attaching a named actor to
  every command it discovers, and asking who that actor actually is as the workshop unfolds, is
  precisely the missing step `role-semantics-investigation.md` found.
- **Policies** ("whenever X happens, then Y") — maps directly onto the governance-facing "who
  consumes this event" question the Product-Owner Perspective Experiment raised, at exactly the
  right altitude: a domain-level *observation* ("something downstream cares about this"), not an
  architecture-level *decision* (a topic name, a broker).
- **Hot spots** — the most directly valuable Event Storming concept for this project's specific
  problem, and the one with no current Canopy equivalent at all. Event Storming's hot spots are
  explicitly for *marking* disagreement or uncertainty, not resolving it in the moment — the same
  discipline `unresolved-decisions-become-explicit-decision-points` already validated for one
  narrower category (business policy), generalized here to any meaning-level uncertainty.
- **Aggregates**, translated carefully — not as a committed technical consistency boundary (that's
  premature at exploration time, and belongs to architecture), but as the softer domain-level
  question underneath one: "what must be true together for this thing to make sense" — which is
  exactly what an identity/uniqueness question already is.

**Does not translate well:**
- **The physical, simultaneous, multi-person medium itself.** Event Storming's power is inseparable
  from many people writing in parallel, unfiltered, on one shared surface. A single-user CLI/REPL
  session cannot recreate that mechanism, and attempting to fake it (see §Solo And Team Usage) risks
  mistaking a structured single-evaluator technique for genuine independent evidence — a specific
  failure mode this project has already caught itself making once, in the "unestablished referent"
  synthesis that didn't survive its own counter-evidence review.
- **Whole-domain, single-workshop scope.** Traditional Event Storming often maps an entire business
  domain in one long session, before any single feature is built. That scope is in direct tension
  with `structure-emerges-from-behavior`'s own validated finding; Canopy's version must stay scoped
  to what a concrete, accepted story actually touches.
- **Commands, as a distinct new artifact.** A story's own `want` field already functions as
  something close to a command — introducing a separate "command" concept on top of it would likely
  be redundant vocabulary for the same thing, not a new capability.
- **Formal aggregate boundaries and bounded-context diagrams**, in their full DDD sense — as
  covered in §Exploration Outputs, this is architecture, not exploration.

---

# Solo And Team Usage

**Solo Product Owner** — the primary, best-fitting case, and the one every mechanism above is
designed around. The honest limitation: a single human and a single model cannot recreate the part
of Event Storming's value that comes from *independent* real people disagreeing with each other.
What *can* be offered, honestly labeled as what it is: structured, differentiated *elicitation* —
asking the same question through a governance-shaped lens and a domain-shaped lens can surface more
candidate ambiguity than one undifferentiated "does this look right?" pass, exactly as the
Product-Owner Perspective Experiment demonstrated. This must not be presented or relied upon as
independent *verification* — that overclaim is precisely what the "unestablished referent" review
had to walk back. Elicitation technique, not evidentiary substitute.

**Small team** — the more interesting design opportunity, because it doesn't require real-time
collaboration to restore Event Storming's actual mechanism. What that mechanism needs is
*independence*, not *simultaneity*: several real teammates each answering the same exploration
questions separately, without seeing each other's answers first, then having the disagreement
between their answers surface as a hot spot — is a legitimate, asynchronous approximation of a
workshop's real payoff, achievable one session at a time, in a tool that only ever serves one
person per session.

**Workshop** — the weakest fit for what Canopy currently is, and worth saying so plainly rather
than stretching the concept to cover it. A real-time, many-person, whiteboard-shaped session is a
different kind of tool than a single-user CLI. Canopy's more honest role here is likely downstream
of a workshop, not a replacement for one: capturing and structuring what an externally-facilitated
session already produced into the same vocabulary/relationship/hot-spot shapes described above —
not running the workshop itself.

---

# Human Insight Integration

Tying each of the five evidenced categories back to why a human — not a bigger model, not a better
prompt — is the right source, per `reserve-the-model-for-genuine-ambiguity`'s own governing
distinction: a model call is justified only where the mapping from input to correct answer is
genuinely input-dependent and not enumerable in advance. Role meaning, identity criteria,
forward-reference significance, ownership, and boundary judgments all fail that enumerability test
by construction — there is no fixed lookup table for "is this actor internal or external," because
the answer depends on a specific business's specific facts, which is exactly the kind of thing this
project's own principle argues should stay a human question, not be quietly modeled away.

Each category's marginal contribution: role meaning and identity criteria are things only the
business itself knows and a model can only guess at plausibly. Forward-reference significance
(does the `Product` mention matter *now*) is a judgment about timing and priority a model has no
basis to make on its own. Ownership and boundary questions are organizational facts — who this
business considers accountable for what — that live entirely outside anything a specification could
contain. Exploration's job in every one of these is to ask the bounded question and preserve
whatever answer (including "still unresolved") comes back — never to supply a plausible-sounding
answer of its own, which is precisely the fabrication failure mode `unresolved-decisions-become-
explicit-decision-points` already spent real, measured effort eliminating for a narrower category.

---

# Risks And Trade-Offs

**Over-modeling / anticipatory design.** The clearest, most evidenced risk this project has direct
experience with. Mitigation-shape (not a mechanism): exploration must never precede an accepted
story, only follow one — the same constraint that already keeps `spec`'s own architecture proposals
from running ahead of accepted intent.

**Generating entities or relationships that don't ultimately matter.** The direct reason
§Exploration Outputs insists on *flagging* a forward reference rather than *materializing* it. A
flag is cheap and reversible; a created entity, once other stages build on it, is not.

**Violating `structure-emerges-from-behavior` directly.** Would happen specifically if exploration
became a whole-project, upfront domain-mapping phase instead of a per-story, per-concept one — the
single most important scope discipline this vision depends on, restated because it is the easiest
one to lose sight of once a "domain exploration" capability starts sounding attractive on its own
terms.

**Duplicate injection against the existing policy checklist.** A live, concrete risk, not a
hypothetical one — this project's own house style already treats "the same rule reaching the model
twice" as a defect it actively audits for. Exploration's identity/uniqueness question must feed the
existing checklist, never run as a second, competing instance of it.

**Question fatigue for a solo user.** This project has direct, first-hand historical evidence this
specific failure mode is real: `explore`'s own original clarifying questions were removed for
"adding friction without value." A domain-exploration capability that reintroduces heavy upfront
questioning risks recreating exactly the problem this project already found and fixed once, in a
different part of the same pipeline.

**Mistaking simulated multi-perspective elicitation for independent verification.** Already
observed once, directly, in this project's own recent work (the "unestablished referent" synthesis)
— a real risk with a real precedent, not a theoretical caution.

**Scope creep into architecture.** If exploration's ownership/boundary outputs drift from "here is a
candidate signal" into "here is a recommended service boundary," it duplicates and potentially
conflicts with `identify_architectural_questions`'s own, already-working proposal mechanism.

---

# Success Criteria

Framed the same way this project already measures everything else — comparable, before/after,
X-of-Y — not as vague aspirations:

- Fewer roles reach `spec`/`behaviors` as an undefined `Role::Simple` entry with zero clarification
  attempted — directly checkable against `roles.yaml` and its `Described`/`Simple` split.
- The residual fabrication rate the policy-discovery citation fix left open (measured at 1–2 of 6
  questions in the controlled comparison) does not increase, and plausibly decreases further, for
  entities that went through exploration first — a direct, comparable extension of evidence this
  project already has.
- Fewer story `so_that`/`want` clauses reference a concept absent from domain vocabulary with no
  corresponding `out_of_scope` acknowledgment — directly checkable, the same way the `Product`
  relationship gap was found in the first place.
- Re-running the same Product-Owner Perspective Experiment methodology against a *future* story that
  went through exploration, and finding materially fewer analogous findings than `manufacturer-001`
  produced — the most direct possible success signal, since it is literally the same instrument that
  originally surfaced the problem.
- Hot spots, once introduced, actually get resolved over time rather than accumulating unresolved —
  a real usage signal, not just a mechanism's existence.

---

# Open Design Questions

Left open deliberately, consistent with this project's own preservation-not-premature-resolution
discipline:

- Whether exploration is a distinct new command or an extension of `intent`'s existing per-story
  flow — a workflow question explicitly out of scope for this vision document.
- How exploration's hot-spot artifact should relate structurally to Stage 2's Decision Point over
  time — kept genuinely separate, later merged, or left as two adjacent mechanisms indefinitely.
- Whether the small-team "independent answers, then diff" mode is worth its added complexity given
  this project has no current evidence of real multi-person usage to design against yet.
- Whether exploration's outputs belong inside existing artifacts (`domain_registry.yaml`,
  `roles.yaml`, enriched) or a new artifact family of their own — an implementation question this
  document deliberately does not decide.
- Whether a workshop-capture role for Canopy is worth pursuing at all, or whether that use case is
  better served by a genuinely different tool this project shouldn't try to become.
