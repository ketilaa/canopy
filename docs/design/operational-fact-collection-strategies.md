# Collection Strategies for an Operational Fact

Status: design-space mapping only. No screens, prompts, commands, or UX proposed. No strategy is
recommended. Maps the fundamentally different ways Canopy could obtain an operational fact before
specification generation, evaluated against the five consumability properties established in
`docs/design/why-role-meaning-succeeded-analysis.md` (closed-set/quotable, single-purpose,
consumer-targeted, shape-compatible with storage, concrete/operational) and against the completed
evidence chain, using Role Meaning as the reference fact type throughout.

Date: 2026-07-17

**A correction worth surfacing before the strategies themselves**: the Role Meaning Value
Experiment's actual proven consumption pathway injected the fact through `existing_adrs` — the same
channel a real ADR occupies — not through `Role::Described`, the storage location earlier documents
in this chain (correctly) identified as real, built, and unused. `Role::Described` has never
actually been exercised as the *source* of a successful consumption test; the ADR channel has. This
matters directly below, since it separates "ADR-driven declaration" from other storage-adjacent
strategies on evidentiary grounds, not just plausibility.

---

# Collection Strategies

## 1. Direct Question

### Description
Canopy asks a bounded, closed-set question at the moment a role (or comparable fact) is
introduced, and a human answers it directly.

### Evidence For
Matches the exact fact *shape* the Value Experiment proved consumable — closed-set, single-purpose,
concrete. The experiment's own injected facts (`internal`/`external`/`affiliated`) are what a direct
question's answer would look like verbatim.

### Evidence Against
The *collection act* itself — a live human answering a live question — has zero evidence in this
chain. Every fact tested was pre-authored and injected programmatically; no experiment measured
whether a real person, prompted live, answers reliably. The original MVP design's own response-rate
question was explicitly set aside in favor of the value question, leaving this specific gap
unresolved by choice, not oversight.

### Expected Fact Quality
High, if the closed-set shape is actually enforced in the answer — directly evidenced by the
consumption experiment's clean results for `internal`/`external`.

### Expected User Effort
Low per instance, but uninvestigated at scale — accumulates once per newly-introduced role or
equivalent fact, with no evidence yet on whether that rate is tolerable.

### Risks
Question fatigue — directly evidenced elsewhere in this project's own history, not hypothetical:
`explore`'s original clarifying questions were removed for "adding friction without value."

### Remaining Unknowns
Whether a live human, asked directly, answers at a meaningfully higher rate than the one directly
comparable precedent this project has (`init`'s existing optional description prompt, skipped 0 of
1 times it has ever been offered).

---

## 2. Correction of an Inferred Suggestion

### Description
Canopy proposes a default/suggested classification; the human corrects it if wrong, otherwise it
stands unchanged (opt-out, not opt-in).

### Evidence For
This is not hypothetical — it is the exact shape of `init`'s existing bootstrap flow
(`bootstrap_select`, an opt-out `MultiSelect` with an optional description prompt). The shape aligns
well with all five properties on paper: closed-set if the suggestion offers bounded options,
single-purpose, targetable, and it already matches `Role::Described`'s own storage shape exactly.

### Evidence Against
Directly measured non-engagement: this exact optional-correction shape, for the one real role this
project has ever produced, was skipped 100% of the time it was offered. This is the single most
directly relevant negative data point available for this strategy — not an analogy, the literal
mechanism.

### Expected Fact Quality
High if corrected, but the one available data point shows it usually isn't — realized quality is
better characterized as low given the evidence, not high given the theoretical shape.

### Expected User Effort
Lowest of all strategies considered (correction only, no authorship) — but the evidence suggests low
effort and low engagement co-occur here, not that low effort guarantees uptake.

### Risks
Reproducing the exact non-result already observed — an escape hatch offered without any
accompanying cost, the same shape `unresolved-decisions-become-explicit-decision-points` already
found insufficient on its own for a structurally similar problem (Policy Discovery's original
fabrication case).

### Remaining Unknowns
Whether the non-engagement is specific to this one instance (n=1) or a general property of
optional, skippable corrections in this pipeline.

---

## 3. Discrepancy Surfacing

### Description
Canopy automatically flags when a story's own language names something absent from domain
vocabulary (the `Product`/`Order` pattern), without necessarily resolving it.

### Evidence For
The underlying phenomenon this strategy would surface is the single best-replicated finding in the
entire investigation chain — recurring, unprompted, across two independent stories.

### Evidence Against
Zero measured downstream effect exists for *surfacing* this kind of discrepancy at all — no
experiment in this chain tested what a human does with the flag once raised, or whether anything
downstream changes as a result. `structure-emerges-from-behavior`'s own evidence also constrains
what resolving the flag could safely mean — anticipating structure not yet concretely described is
the exact anticipatory-modeling risk that principle was built to guard against, which bears directly
on whatever a human would be asked to do next.

### Expected Fact Quality
Unknown — never measured, in either direction.

### Expected User Effort
Potentially very low if fully automatic (detection requires no new human input at all until a
decision point is reached) — but the interaction shape past detection is entirely untested.

### Risks
Reproducing the anticipatory-modeling risk if the *response* to a surfaced discrepancy pushes
toward prematurely modeling a relationship the story hasn't concretely described yet.

### Remaining Unknowns
Whether flagging alone (with no forced resolution) has any measurable value, and whether
`out_of_scope` (the one plausible existing storage location) is an adequate destination for this
kind of fact or merely a coincidentally-available field.

---

## 4. Forced Classification

### Description
A bounded question that cannot be silently skipped — a genuine "unresolved" option remains
available, but skipping requires actively choosing it, not simply declining to engage.

### Evidence For
The closest fit to an already-validated mechanism in this exact codebase, not a new hypothesis:
`unresolved-decisions-become-explicit-decision-points`'s own citation-requirement fix is precisely
this shape — an escape hatch paired with an enforced cost for the alternative — and it measurably
changed behavior for a structurally similar non-engagement problem (Policy Discovery's fabrication,
5/6 → 1–2/6 after the fix).

### Evidence Against
No experiment in this chain tested "forced classification" for role meaning specifically as a live
interaction — the analogy is strong, but it is an analogy, drawn from a different fact category
(business-policy resolution, not actor identity).

### Expected Fact Quality
Predicted highest of all strategies, by direct analogy to the one mechanism in this codebase already
proven to fix a documented non-engagement problem of the same shape (escape hatch without
consequence).

### Expected User Effort
Higher than optional correction, by design — the evidence's own lesson is that only a
cost-bearing shape reliably works, not that low effort is preferable in isolation.

### Risks
A real, evidence-grounded subtlety, not a hypothetical one: the Policy Discovery lesson was never
"never allow skipping" — it was "make honest deferral cheaper than fabrication." If "forced" is
read as eliminating the unresolved option rather than making it a genuine, low-cost, honest choice,
this strategy risks reproducing the original fabrication problem instead of fixing it.

### Remaining Unknowns
Whether the citation-requirement mechanism's specific enforcement shape (a checked, named source)
has any analog for a role-classification question, which has no external source to cite against —
the two problems are similar in shape but not identical in what "evidence" would even mean.

---

## 5. Story-Derived Extraction (Fully Automatic, No Human Check)

### Description
Infer the classification automatically from the story's own text — e.g., from `as_a` phrasing —
with no human confirmation step at all.

### Evidence For
None directly — no experiment in this chain tested a fully-automatic, no-human-check classification
approach for this fact type.

### Evidence Against
Strong, and drawn from the very investigation that found the underlying gap: this is essentially
the *current* status quo for role registration (`intent`'s automatic path, no gate at all) —
the exact mechanism the Role Semantics Investigation found produces an unexamined, silently-invented
classification. It is also the precise failure mode
`unresolved-decisions-become-explicit-decision-points` names directly: "a model asked to extract
[requirements] will not stop and ask what an unresolved question should mean, it will pick an
interpretation."

### Expected Fact Quality
Low-confidence, by direct analogy to a documented, validated fabrication risk — not a guess.

### Expected User Effort
Zero — the defining feature of this strategy, and also its core problem.

### Risks
Highest of all strategies considered: this doesn't introduce a new risk, it reproduces the exact
gap this entire investigation chain exists to address.

### Remaining Unknowns
None material — this strategy's expected outcome is already directly evidenced by the status quo it
would reproduce.

---

## 6. Review-Time Confirmation

### Description
Present the model's own inferred/default classification for human Accept/Modify/Reject at a later
review point (e.g., folded into `spec`'s existing ADR review gate), rather than at the moment of
creation.

### Evidence For
Routes through a mechanism that does actually fire in real use — `spec`'s ADR review gate is
real and exercised (`manufacturer-001`'s real history shows every ADR proposal went through an
Accept action), unlike role registration's own gate, which doesn't exist at all today.

### Evidence Against
Direct, measured evidence this specific gate does not reliably differentiate scrutiny: the
Human-Insight Inventory found every ADR in the one real review session studied — from the most
reproducible category to the least — was Accepted identically, with no visible difference in
scrutiny. The gate firing is not evidence the gate would catch anything here either.

### Expected Fact Quality
Uncertain, and the available evidence leans low — the Inventory's finding suggests this timing
tends toward rubber-stamping rather than genuine review, independent of what's being reviewed.

### Expected User Effort
Low — reuses an interaction pattern the human already performs for other ADRs, requiring no new
habit.

### Risks
Directly reproducing the "review gate gives no differentiated signal" finding — the single most
directly transferable negative result available for this strategy.

### Remaining Unknowns
Whether the non-differentiation finding is specific to the one session/story studied, or a general
property of this review gate's current design regardless of content.

---

## 7. ADR-Driven Declaration

### Description
Record the operational fact as a first-class ADR (the same shape `identify_architectural_questions`
already produces and `spec`'s review gate already handles), rather than as a field on the role/
entity itself.

### Evidence For
The strongest direct evidence of any strategy in this set: this is *literally* the mechanism the
Role Meaning Value Experiment used to achieve its one clean, measured consumption result — the fact
was injected through `existing_adrs`, and `authorization`'s citation mechanism read it from there.
No inference or analogy is required for this claim.

### Evidence Against
An ADR's own shape (`title`/`decision`/`reason`/`alternatives`) is generic — built for architecture
decisions, not tied one-to-one to any of the six business-policy checklist areas — meaning this
strategy inherits whatever ambiguity a broader fact would still carry about *which* consumer should
read *which* part, independent of the declaration mechanism itself. The persona-policy facts in
Phases 2–3 were also ADR-declared and mostly failed to leave a trace — ADR-driven declaration alone
does not guarantee the other four consumability properties; it only supplies the channel proven to
work when those properties are also met.

### Expected Fact Quality
High, *conditional on* the fact itself also being closed-set, single-purpose, and concrete — the
declaration mechanism is proven; the fact shape still has to carry the rest of the burden.

### Expected User Effort
Not directly evaluated as its own dimension — this strategy is about *where* a fact lives, not
*how* a human supplies it; it composes with strategies 1, 2, or 4 rather than replacing them.

### Risks
Mistaking "this channel is proven" for "any fact placed in this channel will be consumed" — the
persona-policy facts' failure, using the identical channel, is direct evidence against that
inference.

### Remaining Unknowns
Whether `Role::Described` (the storage location actually built for this purpose) would perform
differently from the ADR channel if it were ever actually exercised — genuinely unknown, since it
has never been tested as a consumption source at all.

---

# Comparative Assessment

Not a recommendation — a relative read of alignment against the five consumability properties and
the two governing principles (`structure-emerges-from-behavior`, minimal upfront elicitation),
using only what's been measured.

**Best-aligned with the consumability properties on paper**: Direct Question, Forced
Classification, and ADR-Driven Declaration — each can straightforwardly be closed-set,
single-purpose, targeted, and concrete by construction, and ADR-Driven Declaration additionally has
direct (not analogical) evidence of working at least once.

**Best-aligned with prior validated fixes for a structurally similar problem**: Forced
Classification, by direct analogy to the Policy Discovery citation-requirement fix — the one case
in this codebase where an escape-hatch-without-cost problem was identified and then measurably
solved.

**Weakest evidence given the aligned shape**: Correction of an Inferred Suggestion and Review-Time
Confirmation — both have real, structurally favorable shapes, but both also have direct, specific,
already-measured non-results in this exact pipeline (0-of-1 engagement; zero differentiated
scrutiny), not merely theoretical risk.

**Actively contraindicated by the evidence**: Story-Derived Extraction — the only strategy whose
expected outcome is not just uncertain but already known, since it reproduces the status quo gap
this investigation chain exists to address.

**Least understood, not yet placeable**: Discrepancy Surfacing — the underlying phenomenon is the
best-replicated finding in the whole chain, but nothing is known about what collecting a response to
it would even look like or whether it has any value, making it harder to place on this comparison
than the others, not necessarily weaker.

**On the two governing principles**: every strategy except Story-Derived Extraction is compatible
with `structure-emerges-from-behavior` *if* triggered only after a concrete story/role has already
emerged — none of the strategies inherently violate this, it is a property of *when* each fires, not
which one is chosen. Story-Derived Extraction is the one strategy whose "compatibility" with minimal
upfront elicitation is illusory — it achieves zero elicitation by eliminating the human step
entirely, which is exactly the shape already shown to fail, not a genuine instance of the
emergent-over-upfront principle this chain has otherwise validated.
