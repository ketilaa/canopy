# Is Internal/External the Most Stable Role Classification?

Status: design validation only. Tests the classification proposed in
`docs/design/domain-exploration-mvp-design.md`'s "What Success Looks Like" section against a
representative set of realistic role names, before that proposal is treated as settled. No
implementation, no prompts, no commands.

Date: 2026-07-16

---

# Method

Twenty realistic role names, deliberately spanning several domains (not just manufacturer/
e-commerce, so the test isn't accidentally scoped to the one project this whole investigation has
real data for), each classified honestly under the proposed internal/external question: *does this
actor act on the business's own behalf, or is it a party outside the business itself?* A role is
marked **clean** if the label alone answers this without strain, **strained** if a defensible answer
exists but requires a judgment call the label doesn't obviously settle, and **fails** if the binary
itself doesn't have room for the right answer at all.

# Results

| Role | Internal/External | Fit |
|---|---|---|
| manufacturer representative | External (represents the manufacturer, not the business) — *or* internal (a catalog/admin person recording data). Genuinely either, by design — this is the case the whole investigation started from. | Strained — but this is the *known* strain the classification exists to surface, not a new problem |
| customer | External | Clean |
| administrator | Internal | Clean |
| product manager | Internal | Clean |
| warehouse operator / staff | Internal | Clean |
| guest checkout user | External | Clean |
| anonymous visitor | External | Clean |
| store manager | Internal | Clean |
| system administrator | Internal | Clean |
| delivery driver | Could be an internal fleet employee or a third-party courier — the label alone gives no basis to prefer one | Strained |
| supplier | Structurally identical ambiguity to "manufacturer representative" — a supplier's own staff (external) vs. an internal procurement person recording supplier data (internal) | Strained |
| auditor | An internal audit function, or an external audit firm/regulator — both real, common readings | Strained |
| reviewer | Internal QA reviewer, an external peer reviewer, or a customer leaving a review — three plausible readings, not two | Fails — the binary has no room for a third, equally plausible reading |
| franchise partner | Operates under the business's brand, but is a distinct, independent legal entity — neither "acts on the business's own behalf" nor "is an arms-length outside party" describes this cleanly | Fails |
| contractor | Depends entirely on engagement terms the label doesn't state — long-term embedded contractor reads close to internal; a one-off gig reads close to external | Fails |
| physician / affiliated clinician | An employed hospital physician is internal; an independently affiliated physician with granted privileges is neither a hospital employee nor an arms-length outsider | Fails |
| compliance officer | Internal | Clean |
| bank teller | Internal | Clean |
| vendor account manager | Internal (manages the relationship *on the business's behalf*) | Clean |
| API integration partner | Not a human actor at all — see §Scope Boundary below | Out of scope, not a binary failure |

**Tally**: 9 of 20 clean, 5 strained, 4 fails (excluding the one out-of-scope case), and one of those
5 strained cases is the original, already-known case the classification was built to surface. **A
genuine, non-trivial fraction — 9 of 19 human-actor roles, given the strains are real too — do not
resolve cleanly as a strict internal/external binary**, and this is not a fringe result: suppliers,
auditors, contractors, franchise partners, and affiliated professionals are ordinary, common role
shapes, not exotic edge cases invented to break the test.

# Diagnosing the Pattern in the Failures

The four **fails** share a structure the nine **clean** cases don't: each names a relationship to
the business that is neither "employed/directly controlled by it" nor "a stranger to it" — an
ongoing, recognized, partially-trusted affiliation with a party that remains organizationally or
legally separate. A franchise partner, a long-term contractor, and an affiliated physician are the
same *shape* of relationship, just from different domains. Internal/external, read as a strict
employment-status binary, has no third bucket for this shape — it forces a choice between two poles
neither of which is actually correct.

The **reviewer** case fails differently: not because a third relationship-shape is missing, but
because the label itself doesn't specify a single interaction at all — "reviewer" could denote three
unrelated actors (internal QA, external peer reviewer, customer) depending on *what* is being
reviewed, a piece of information the role name alone never carries.

# Testing a Reframed Primary Question

Internal/external, tested directly, turns out to be a **derived answer**, not the most primary
question — the more fundamental question underneath it, tested against the same twenty roles, is:
***on whose behalf does this actor act, in this interaction?*** If the answer is "the business's
own," the role is internal-shaped. If the answer is "an outside party — including themselves," it
is external-shaped. Internal/external falls out of this question in the nine clean cases exactly as
before — nothing is lost there. What changes is how the failures behave under the reframe:

- **Supplier / manufacturer representative** — the reframe doesn't resolve the ambiguity (it was
  never going to; the label genuinely doesn't say), but it asks a sharper, more directly
  authorization-relevant question than "are they an employee," and it correctly produces the same
  legitimate **unresolved** answer the classification already needs to support. No regression.
- **Auditor** — "does this auditor act on our own organization's behalf, or an outside firm's?" is a
  cleaner, more precise version of the same question, and resolves to the same **unresolved** state
  when the label alone doesn't say — again, no worse than before, and more precisely targeted.
- **Franchise partner, long-term contractor, affiliated physician** — the reframe doesn't force
  these into "unresolved" the way the strict binary did; it reveals that the honest answer is
  neither pole. This is a real, structural finding, not a labeling nicety: **a third state —
  affiliated: a recognized, ongoing relationship with a legally or organizationally separate party —
  is needed alongside internal/external/unresolved, not folded into external by default.** Folding
  a franchise partner into "external" would understate the trust relationship in exactly the
  direction that matters for authorization design; folding it into "internal" would overstate it.
- **Reviewer** — the reframe doesn't fix this one either, because the underlying problem (the label
  names a function, not a relationship) is a different failure mode than the other four. This case
  argues for the classification applying to a *specific* role-in-context rather than a bare label in
  isolation, not for a different set of categories — worth naming as a separate, narrower limitation
  rather than folding it into the affiliated-party finding above.

# Scope Boundary: Non-Human Actors

`API integration partner` was included deliberately, to check whether the classification needs to
cover system/service actors at all. It doesn't, and mostly by existing design: `suggest_roles`'s own
prompt rule (`canopy-llm/src/prompts/intent.rs:207-211`) already states "Human actors only — not
systems, services, or technical components" for the `init`-bootstrap path. This is not, however, an
equally firm guarantee for the *other* path that actually matters most — `intent`'s automatic
per-story role registration has no equivalent explicit rule; it relies on the conventional "As a
&lt;role&gt;" user-story phrasing biasing toward human personas, not a stated constraint. Worth
flagging honestly as a soft, convention-based boundary on the path this whole investigation has
focused on, not a hard, verified one — though nothing in this project's real history has ever
produced a non-human role via either path, so this remains a theoretical gap, not an observed one.

# Conclusion

**Internal/external is not the most stable primary distinction — it is a correct derived answer in
most, but not all, realistic cases, and it fails structurally (not just awkwardly) for a real,
non-trivial fraction of common role shapes.** The more stable, more primary question this test
surfaces is *on whose behalf does this actor act in this interaction* — internal/external falls out
of it as the two common poles, exactly as before, for the 9 of 20 roles that were already clean.
What the reframe adds, rather than merely relabels, is room for a genuine third state —
**affiliated** — that a strict binary has no honest place for, evidenced directly by franchise
partners, long-term contractors, and affiliated professionals failing the original binary in the
same specific way.

This revises, rather than discards, the MVP design's proposed answer set. The three-way
`internal | external | unresolved` set from `domain-exploration-mvp-design.md` should be read as
**superseded** by a four-way set — `internal | external | affiliated | unresolved` — pending
whichever future document actually updates that design; this document does not itself edit that
proposal, consistent with the discipline this project has already used once before for a same-day
correction (`docs/design/roadmap-reassessment.md`'s own disclosed correction after the
"unestablished referent" synthesis). The underlying MVP recommendation — role meaning capture, one
bounded question, fired once per newly-registered role — is unaffected; only the shape of the
answer set is.
