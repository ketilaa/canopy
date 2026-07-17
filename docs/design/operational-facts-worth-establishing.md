# What Operational Facts Are Worth Establishing Before Planning?

Status: evidence synthesis only, extending `docs/design/exploration-phase-first-principles-
reassessment.md` and `docs/design/exploration-output-consumability-properties.md`. No workflow,
prompt, stage, UX, or implementation is proposed anywhere in this document. Answers a narrower
question than "how do we improve exploration": using only the surviving findings from the
completed investigation chain, which specific operational facts appear justified to establish
before specification generation begins — and which don't.

Date: 2026-07-17

---

# Candidate Operational Facts

Five candidates evaluated, drawn directly from the surviving findings rather than invented for
this document: **role meaning**, **story-vocabulary-vs-domain-registry discrepancy**, **identity/
uniqueness criteria**, **ownership/entitlement verification criteria**, and **actor relationship**
(evaluated separately to test whether it's genuinely distinct from role meaning, since the two are
easy to conflate).

---

# Evidence For

**Role meaning.** The Role Semantics Investigation found a real, structural gap: `Role::Described`
exists and is reachable through `init`'s bootstrap path, but `intent`'s automatic per-story
registration — the only path that has ever populated a role in this project's real history — never
reaches it. The Role Meaning Value Experiment then showed, in a controlled comparison, that when a
role's classification *is* supplied, it produces a citation-backed, traceable change in
`authorization`'s resolution in 2 of 3 tested conditions. This is the only candidate in this set
with both a confirmed structural gap *and* a measured downstream causal effect.

**Story-vocabulary-vs-domain-registry discrepancy.** First found on `manufacturer-001` (the
Product-Owner Perspective Experiment): the story's own `so_that` names `Product`, never extracted
into domain vocabulary. Recurred, unprompted, on `order-001` during Phase 3's unrelated setup work —
the same gap, a different story, never anticipated the second time. This is the only candidate in
this set with genuine, unprompted cross-story replication.

**Identity/uniqueness criteria.** The Product-Owner Perspective Experiment's original duplicate-name
finding suggested this was missing. Some support exists in that a real ambiguity was genuinely
found. But see Evidence Against — this support does not survive the Enumeration Gap Investigation's
direct code check.

**Ownership/entitlement verification criteria** (what proof establishes that an actor may act on a
specific record — distinct from role meaning, which is about the actor's general identity, not a
per-instance entitlement check). Directly evidenced by the single cleanest citation result in the
Human Insight Process Experiment: `risk_averse`'s fact ("must include the original order/purchase
confirmation number, verified against our records") was cited verbatim as `evidence` for both a
`uniqueness` and a `consistency` resolution.

**Actor relationship** (relationships between distinct actors, or between an actor and another
entity, as its own category separate from a single actor's identity). No surviving finding in this
chain evaluates a relationship *between* two actors as distinct from a single actor's own
internal/external classification. The closest evidence — the Role Semantics Investigation's own
framing ("does this actor act on the business's own behalf, or an outside party's") — is a
relational question, but it was investigated and evidenced entirely as part of role meaning, not
separately.

---

# Evidence Against

**Role meaning.** Tested against exactly one real role (`manufacturer representative`) across the
entire chain. The stability test found internal/external/affiliated is itself a live, evolving
classification (the original three-way set was superseded by a four-way one after the stability
test), meaning the "correct" shape of what to establish was still being refined as of this chain's
last check.

**Story-vocabulary-vs-domain-registry discrepancy.** The downstream *impact* of surfacing this was
never measured — unlike role meaning, no experiment in this chain tested what happens if this
discrepancy is flagged. It is entangled directly with `structure-emerges-from-behavior`'s own
anticipatory-modeling caution: the domain-extraction step's exclusion of purpose-clause-only
concepts is itself evidenced, deliberate design, not an oversight — surfacing the discrepancy
without resolving it is the only shape this chain's own evidence would sanction, and even that
narrower shape is untested.

**Identity/uniqueness criteria.** The Enumeration Gap Investigation found this is *already* an
enumerated, working checklist area (`uniqueness`, `entity_schema_prompt`). `manufacturer-001`'s
original duplicate-name finding is better explained by that story's artifact predating the checklist
mechanism than by a current gap. Establishing this as a new category would duplicate a mechanism
already shown to work — the "same rule reaching the model twice" anti-pattern this project's own
house style already flags as a defect.

**Ownership/entitlement verification criteria.** Also already maps onto existing checklist areas
(`uniqueness`, `consistency`, and implicitly `authorization`) — `risk_averse`'s clean result is
itself evidence the *existing* mechanism handles this well when the supplied fact is specific
enough. This is not evidence of a missing category; it is confirmation an existing one already
works, given the right input shape.

**Actor relationship.** No surviving finding provides direct support distinct from role meaning.
Treating it as its own category would risk manufacturing a candidate the evidence doesn't actually
justify separately.

---

# Existing Consumer

| Candidate | Existing consumer? |
|---|---|
| Role meaning | Partial — nothing currently reads a role's classification, but `authorization`'s citation mechanism was directly shown (Value Experiment) to consume it once supplied through the existing ADR-context channel. |
| Story-vocabulary discrepancy | None. No prompt, checklist, or Decision Point mechanism checks a story's `so_that`/`want` against domain vocabulary at all. |
| Identity/uniqueness criteria | Yes — the `uniqueness` checklist area, confirmed directly in code. |
| Ownership/entitlement verification | Yes — `uniqueness`/`consistency`/`authorization` checklist areas; `risk_averse`'s result is direct proof of consumption. |
| Actor relationship | Not separately evaluated; whatever consumer exists is the same one already credited to role meaning. |

---

# Existing Storage Location

| Candidate | Existing storage? |
|---|---|
| Role meaning | Yes — `Role::Described { name, description }` already exists in `canopy-core`, unused by the one path that matters. |
| Story-vocabulary discrepancy | Partially — `out_of_scope` (a real, already-used field in `IntentSpec`) could hold an acknowledgment, but nothing currently populates it for this purpose. |
| Identity/uniqueness criteria | Yes — `ResolvedPolicy` under `area: uniqueness`. |
| Ownership/entitlement verification | Yes — same `ResolvedPolicy` structure, multiple areas. |
| Actor relationship | Not separately evaluated. |

---

# Replication Status

| Candidate | Replicated? |
|---|---|
| Role meaning | Structural gap: not replicated (one role, ever). Downstream consumption mechanism: replicated in the sense that the same citation mechanism was independently confirmed to generalize to human-supplied facts across the Value Experiment's own multiple conditions. |
| Story-vocabulary discrepancy | **Yes — the strongest replication in this set.** Unprompted recurrence across two independent stories. |
| Identity/uniqueness criteria | N/A as a gap — the "gap" itself failed to replicate as a real current-code issue once checked directly against source. |
| Ownership/entitlement verification | Single clean instance (`risk_averse`); not independently repeated elsewhere in the chain. |
| Actor relationship | No dedicated replication — inherits whatever applies to role meaning. |

---

# Confidence

- **Role meaning: Medium-high.** Strong mechanism-level evidence (the Value Experiment's causal
  chain is the cleanest in the whole investigation), weak sample-size evidence (one role).
- **Story-vocabulary discrepancy: Medium.** The gap itself is the best-replicated finding in this
  set, but confidence in its *value* is capped by having no measured downstream effect at all.
- **Identity/uniqueness criteria: Low, as a new category** — not because the underlying concern
  isn't real, but because the evidence points at "already handled," not "needs establishing."
- **Ownership/entitlement verification: Low, as a new category**, same reasoning — the evidence
  supports the existing mechanism, not a new one.
- **Actor relationship: Insufficient evidence to evaluate as distinct from role meaning.**

---

# Ranked Recommendations

Ranked by expected downstream impact divided by additional exploration required — both estimated
strictly from evidence already gathered, not from intuition about the topic's importance.

**1. Role meaning.** Highest ratio: the downstream impact is not merely plausible but *measured*
(the Value Experiment's own controlled result), and the additional groundwork required is the
lowest of any candidate — the storage location already exists and is simply unpopulated by the one
code path that matters; the fact shape that reliably produces an effect (narrow, single-question,
one classification) is already established by the same experiment. What remains uncertain
(replication across more than one role, and whether the internal/external/affiliated set is final)
is refinement, not foundational discovery.

**2. Story-vocabulary-vs-domain-registry discrepancy.** Second: the gap itself is the single
best-replicated finding in this entire investigation chain, which argues strongly for its
*reality*. It ranks below role meaning only because its downstream impact has never actually been
measured — establishing this fact requires more new groundwork (no consumer, no dedicated storage
use yet) than role meaning does, and the one principle most directly relevant to it
(`structure-emerges-from-behavior`) narrows what could even be safely established here to
*flagging*, not resolving — a real, but smaller, next step than role meaning's.

**3. Identity/uniqueness criteria and ownership/entitlement verification criteria — not
recommended as new categories.** Both already have a working consumer and storage location; the
evidence in this chain argues for leaving them alone, not for further exploration investment. The
one clean success in this whole chain involving ownership/verification content
(`risk_averse`) is itself proof the existing mechanism already does this correctly when given a
specific enough fact — this is evidence *against* new capability, not for it.

**4. Actor relationship — not independently justified.** No surviving finding treats this as
distinct from role meaning; recommending it as a separate category would not be grounded in this
chain's own evidence.
