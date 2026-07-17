# Where Does Human Insight Enter a Full Canopy Run?

Status: experiment design only. No implementation, no Domain Exploration redesign, no new feature
proposal. Answers what experiment would tell us, across a full real run rather than one injected
fact, which categories of human judgment actually change Canopy's output and which don't — building
directly on the Role Meaning Value Experiment's first live evidence that explicit meaning can move
downstream artifacts, extended from one variable to a whole real session.

Date: 2026-07-17

---

# Experiment Purpose

Every prior experiment in this chain tested one thing at a time: the Product-Owner Perspective
Experiment tested passive *review* of a fixed artifact set; the Role Meaning Value Experiment
tested one injected *fact* against one mechanism. Neither tested a full, *active* run — a real
person making the real decisions a Product Owner actually makes, at every point Canopy already lets
a human decide something, from `intent` through `behaviors`. This experiment runs the same story
through that real process five times, once per persona, and asks a sharper version of the question
this whole chain has been converging on: not just "can explicit meaning help" (answered), but
**which categories of human judgment, exercised through Canopy's own existing gates, actually move
the artifact — and which categories converge no matter who is deciding, because they were never
meaning-dependent in the first place.**

---

# Proposed Story

**"Customers must be able to request a return for a product they previously purchased."**
(candidate id prefix: `return-001`)

Deliberately not `manufacturer-001`, and deliberately chosen with a **clean**, unambiguous role
(`customer` — already classified `clean` in `docs/design/role-classification-stability-test.md`)
rather than a role already known to be contentious. This is an intentional design choice, not an
oversight: testing a story whose *role* is not in question is a more rigorous test of whether
persona-driven divergence shows up in *other* categories of meaning (policy, relationships,
ownership) than reusing a role already suspected to diverge — a story built to reproduce a known
result would prove less than one that doesn't lean on it.

The story is small (one action, one entity — `ReturnRequest`) but naturally rich in exactly the
ways the earlier investigations found meaning tends to hide:
- **Trust/verification, a different flavor than manufacturer-001's role/permission question**:
  must the customer be verified as the actual purchaser of this specific order? This is an
  ownership-of-record question, not a role-permission one — genuine diversification of what
  "authorization-shaped" can mean.
- **Forward references**: "a product they previously purchased" names `Product` and an implied
  `Order`/`Purchase` concept, neither of which exists in a fresh domain registry — a direct,
  independently-chosen test of whether the manufacturer-001 forward-reference finding recurs.
- **Business rules with real, divergent defaults**: how long after purchase can a return be
  requested; what condition must the item be in; is a returned order's status a one-time flag or a
  richer lifecycle — none of these have an obvious, training-data-dominant default the way
  "PostgreSQL for a relational store" does, unlike the stable categories the reproducibility sweep
  already found.
- Small enough that five full sessions through `intent`→`spec`, and optionally `behaviors`, is a
  bounded, plannable amount of work, not a second domain to build out.

---

# Product Owner Profiles

All five are Product Owners — none is an architect, developer, or operator — differentiated by
**decision-making instinct**, not review instinct, since this is a process experiment: each profile
is defined by what it would actually *decide* at a real gate, not what it would *notice* reading a
finished artifact (the axis the earlier review experiment's personas were built around).

1. **Risk-Averse / Loss-Prevention PO** — decides toward protecting the business from abuse: would
   require verified proof of purchase, a narrow return window, one return per order.
2. **Customer-Experience PO** — decides toward minimizing friction and preserving goodwill: would
   accept a generous window, lenient verification, and push back on anything that reads as a
   customer-facing obstacle. The most direct, natural contrast to (1) on the exact same questions.
3. **Operationally-Minded / Efficiency PO** — decides toward the lowest-overhead, most automatic
   resolution: prefers deterministic, rule-based outcomes (approve within a fixed window
   automatically) over anything requiring a case-by-case human step.
4. **Compliance/Finance-Minded PO** — decides from external obligation rather than internal
   preference: return-window and refund-handling choices driven by what consumer-protection
   or financial-reporting requirements would actually demand, a genuinely different *source* of
   judgment than instinct-driven preference.
5. **Growth/Retention-Minded PO** — decides toward the relationship, not the transaction: attends
   to what happens *after* the return (a replacement offer, capturing the reason for product
   feedback) — the profile most likely to pull toward relationship/ownership questions rather than
   narrow policy wording, the process-experiment counterpart to the earlier review experiment's
   Product-Portfolio persona.

---

# Experimental Method

**Held constant** across all five sessions: the exact behavioral statement given to `intent`,
verbatim; the LLM/model configuration; which pipeline stages are run (`intent` → `spec` at minimum,
extended into `behaviors` where practical — see Risks for the cost trade-off); and the mechanical,
non-judgment parts of the pipeline (Stage 0's completeness structure, Stage 1's mechanical
behaviors, clustering) — these are expected to be identical or near-identical by construction and
are not where this experiment expects to find anything.

**Varies**, by design, across the five sessions: every decision made at a real, already-existing
human gate — `intent`'s per-story Accept / Accept-with-edit / Reject choice; `spec`'s per-ADR
Accept / Modify (with free-text revised decision) / Reject choice; and, where the story's own
`open_questions` reach it, `behaviors`' Stage 2 Decision Point resolution (`select_required` over
the model's own proposed options, plus the considered-decision-vs-temporary-assumption confirm).
Each session is driven live, matching this project's own established interactive-dogfooding
convention, with the persona's stated instinct determining every choice at every gate — not five
separate LLM calls asked to "act as" a persona in isolation, but one real, gate-by-gate session per
persona, the same shape of interaction a real Product Owner would actually have.

**One necessary addition, reusing rather than inventing a mechanism**: today's real pipeline has no
production gate for role meaning specifically — that capability doesn't exist yet. Where a
persona's own instinct would naturally lead them to clarify what the actor is or supply a
distinguishing business rule beyond what an existing ADR proposal covers, the same technique the
Role Meaning Value Experiment already validated is reused: the persona may introduce a synthetic,
ADR-shaped fact into context, exactly the way that experiment injected a role classification. This
is not a new mechanism for this experiment to justify — it is the same one already run once, applied
here as one more available choice within an otherwise unmodified real session, not a parallel,
separately-designed channel.

---

# Artifacts To Compare

In priority order, reasoned explicitly rather than listing the full example set flatly:

1. **Decision Point resolutions (Stage 2)** — the single most direct test available, and one no
   prior experiment in this chain has actually exercised: `manufacturer-001` never reached a real
   Modify/Reject/Decision-Point-resolution gate in its whole history. A story that naturally raises
   open questions (return window, verification requirement) gives five personas a genuine,
   already-built gate to resolve differently and comparably.
2. **Policy divergence** (`resolved_policies` — especially any area touching verification,
   uniqueness of a return-per-order, and retention/window questions) — the direct, natural extension
   of the Role Meaning Value Experiment's own strongest finding, now tested against instinct-driven
   persona choice rather than one injected fact.
3. **Role and domain-vocabulary divergence** — does any persona's own edits or added facts lead to a
   *different* entity or event being extracted (e.g., a `RefundIssued` event, a `ReturnReason`
   field) that another persona's session never produces — a deeper kind of divergence than wording,
   testing whether persona instinct changes what counts as "the domain" at all.
4. **Specification differences** (entity schema fields, scenario content, mandatory/optional splits)
   — the direct parallel to the Value Experiment's Specifications category, now tested across five
   differently-minded drivers instead of four injected conditions.
5. **`open_questions` count and content** — a secondary, corroborating signal for (1) and (2), not
   an independent primary measure.
6. **Behavior/contract differences (Stage 1–4), if the run is extended that far** — valuable for
   understanding whether spec-level divergence compounds or gets absorbed by the more mechanical,
   schema-derived later stages, but downstream of, and largely explained by, (1)–(4).
7. **Architectural proposals (service naming, tech stack)** — lowest priority. The reproducibility
   sweep already established these vary from pure model-sampling noise independent of any human
   input, and Product Owners have no legitimate standing over a database or framework choice in
   this project's own established norms — divergence here would be the least diagnostic of anything
   this experiment could find.

---

# Analysis Framework

Explicitly not "which persona was correct" — a four-step framework instead, reusing this project's
own already-validated classification discipline rather than inventing new judgment criteria:

1. **Classify each artifact category as convergent or divergent** across the five sessions, using
   the same tiered scheme the reproducibility sweep already validated (wording-only / equivalent /
   materially different / structural divergence) — not a new severity scale invented for this
   comparison alone.
2. **For every divergent item, trace it to a specific decision.** Did a persona's actual edit,
   rejection, or Decision Point resolution demonstrably cause the difference — or did the divergence
   show up between two sessions that made the *same* choice at every relevant gate, in which case it
   is ordinary model-sampling variance, the exact kind the reproducibility sweep already
   documented, and not evidence of persona-driven meaning at all. This distinction is load-bearing:
   without it, any five-run comparison would find *some* divergence purely from sampling noise, and
   mistaking that for a persona effect would repeat the "unestablished referent" over-synthesis this
   project has already had to correct once.
3. **For every convergent item, ask why it converged.** Genuine boilerplate — the artifact simply
   doesn't depend on business judgment — is a different finding from *no persona ever being given a
   real opportunity to diverge* at that specific point, the same wiring-vs-value distinction the
   Role Meaning Value Experiment already had to draw for its own null results.
4. **Only after (2) and (3) are both done**, look across categories for where traceable,
   instinct-attributable divergence actually concentrated — the answer to "where does human meaning
   enter" is whatever survives both filters, not the raw diff between any two runs.

---

# Success Signals

The single cleanest, most informative possible result: a **split-screen pattern** — mechanical,
structural output (field types, basic length-validation shapes, the core happy-path scenario)
converges tightly across all five sessions, while policy resolutions, Decision Point outcomes, and
relationship/ownership discovery diverge, and the divergence *correlates* with each persona's
stated instinct (the Risk-Averse and Customer-Experience sessions landing on opposite, internally
consistent answers to the same verification/window questions, not on unrelated, scattered
differences). That correlation is what would separate a real instinct-effect from noise that merely
happens to look like one.

A second, narrower success signal worth naming on its own: **at least one Decision Point resolved
differently, and traceably, by at least two personas** — the first time in this project's real
history that mechanism would have been exercised by genuine divergent human judgment at all, since
`manufacturer-001` never reached it.

A result where divergence appears but does *not* correlate with instinct, or concentrates in
architecture/tech-stack territory rather than policy/relationship territory, would be a materially
weaker, more ambiguous outcome — informative, but not the clean confirmation the split-screen
pattern would be.

---

# Risks

**Confounding persona-driven divergence with ordinary model-sampling noise is the central risk**,
not a minor caveat — the reproducibility sweep already showed identical, unmodified input produces
real variance across repeated runs with no persona or human difference involved at all. Any
five-session comparison will show *some* divergence from this alone; the Analysis Framework's
step 2 exists specifically to guard against attributing that portion to persona judgment. Ideally a
same-persona, repeated-session control would establish the noise floor this comparison should be
read against — not proposed as part of this run, but named as the missing baseline that would make
the result fully rigorous.

**All five personas are still driven by one evaluator**, the same limitation this project has
already had to name and correct itself on once (the "unestablished referent" synthesis). Divergence
across these five sessions is evidence that different *reasoning stances* produce different
outcomes through Canopy's real gates — not evidence that five actual, independent Product Owners
would diverge the same way. This should be stated plainly in any result, not softened.

**Cost and scope**: five full interactive sessions through `intent`→`spec`, extended into
`behaviors` where practical, is substantially more real LLM-call volume than any single-variable
comparison this chain has run so far — worth sizing honestly before committing to running all five
through every stage; extending into `behaviors` for a subset first, rather than all five at once, is
a reasonable way to bound this without abandoning the deeper comparison.

**N=1 per persona** — the same limitation the Value Experiment's results already carried forward.
This experiment cannot distinguish "this instinct reliably produces this outcome" from "this one
session happened to."

---

# Expected Learning Value

**If the split-screen pattern holds**: this becomes the first evidence that the Role Meaning Value
Experiment's finding generalizes past role meaning specifically to a broader class of business
judgment — a materially stronger foundation for the Domain Exploration vision than one injected-fact
comparison alone, spanning policy, relationships, and Decision Points rather than one mechanism.

**If divergence doesn't correlate with instinct, or doesn't concentrate where theorized**: at least
as informative, and not a failure — it would mean either today's real gates (Accept/Modify/Reject
text edits) don't give a Product Owner enough leverage to express distinct judgment through Canopy
as it exists now, or the model's own defaults dominate regardless of who nominally holds the
decision — a materially different, important finding about how much influence a human reviewer
actually has today, independent of whether any new capability like role meaning capture ever ships.

**What would remain unknown regardless of outcome**: whether this generalizes across more than one
fresh story; whether the instinct-correlated pattern (if found) would replicate on a second run per
persona; and whether today's improvised gates plus Wizard-of-Oz role-fact injection understate what
a purpose-built Domain Exploration mechanism could actually elicit — this experiment tests what
Canopy's *existing* interaction surface can reveal about where meaning enters, not the ceiling of
what a dedicated capability might achieve.
