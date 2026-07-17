# Backlog Discovery — First Learning Iteration

Status: recommendation for the smallest real intervention, not a framework or roadmap. Extends
`docs/design/exploration-as-backlog-discovery-hypothesis.md`'s conclusion that story-shaped
concerns are currently undetected by any mechanism. Names one concrete, minimal, implementable
experiment — not a complete feature, not a new pipeline stage — to enter a real
implement → measure → learn cycle as fast as possible.

Date: 2026-07-17

---

# Strongest Candidate Concern Category

Ranked by expected impact, expected frequency, and confidence, drawing only on evidence already
gathered:

| Category | Expected impact | Expected frequency | Confidence | Rank |
|---|---|---|---|---|
| Domain-Boundary / Vocabulary Discrepancy | High | High | High | **1** |
| Lifecycle actions beyond creation | High, when present | Medium — varies by domain | Medium | 2 |
| Downstream consumers | Medium-high | Low-medium | Low-medium | 3 |
| Approval / Accountability | High, when present | Low | Low | 4 |

**Domain-Boundary / Vocabulary Discrepancy ranks first, and not narrowly.** It is the only
category with a 100% hit rate across the only two real stories this project has ever produced with
a complete spec (`manufacturer-001`: `Product` referenced, never captured; `order-001`: `Product`
and `Order` referenced, never captured) — found the second time unprompted, during unrelated setup
work, which is the strongest form of replication this chain has anywhere. Every other category was
elicited through persona review specifically structured to find it, and appeared only once.

---

# Why It Ranks First

Beyond the evidence ranking, this category has a property none of the others share: **its
detection is mechanical, not judgment-based.** A story's own `want`/`so_that` text either contains
a term absent from `domain_registry.yaml`'s known entities/events or it doesn't — this is a
deterministic check, not an LLM inference. This aligns directly with `reserve-the-model-for-
genuine-ambiguity`'s own governing principle: when a mapping is fully enumerable, compute it in
code rather than asking a model. Lifecycle, approval, and downstream-consumer concerns all require
genuine judgment to detect ("does this story imply a lifecycle beyond creation?") — vocabulary
discrepancy does not. It is simultaneously the best-evidenced category and the cheapest to check.

---

# Smallest Possible Intervention

Not a new stage, not a blocking gate, not an LLM call: a small, additive, **purely observational**
mechanical check, run once after a story's domain vocabulary already exists (i.e., after `intent`'s
own extraction has run), that scans the story's own `as_a`/`want`/`so_that` text for capitalized,
noun-shaped terms not present anywhere in `domain_registry.yaml`'s entities or events, and surfaces
each as a printed note — not a file, not a gate, not a new artifact type for this first pass.
Nothing about the existing pipeline changes; nothing blocks; no review gate is added. The
intervention is scoped deliberately small enough that it could be added without touching any
existing command's control flow — a side observation printed alongside output that already exists.

---

# Signals To Measure

- **Raw frequency**: how often the check fires across real sessions — establishes the baseline
  rate independent of the two data points already in hand.
- **Backlog uptake**: whether a subsequent, separate `canopy intent` call — checkable directly
  against `stories.yaml`'s own accumulated history — introduces a story whose domain plausibly
  resolves a previously-flagged term (e.g., a later story about `Product` following an earlier flag
  on `manufacturer-001`). This is the one signal that actually tests the hypothesis, not just its
  precondition.
- **False-positive rate**: how often a flagged term turns out to be incidental (a capitalized word
  with no real domain significance) rather than a genuine missing story — measurable simply by
  whether anything ever follows up on a given flag across a reasonable number of subsequent
  sessions.

---

# Success Interpretation

If flagged terms correlate with humans later creating the implied story, this is the first direct,
practical (not just structural) evidence that backlog discovery has real value, not merely
plausible value — justifying further investment in making this signal more prominent or reliable.
If the false-positive rate is low, it further justifies trusting the mechanical detection itself
without needing a more sophisticated, judgment-based check.

# Failure Interpretation

If the check fires reliably (confirming frequency, as already strongly suspected) but humans never
subsequently create the implied story, this would not indict the underlying pattern — that pattern
is already well-evidenced — it would show that **passive surfacing alone is insufficient to change
backlog behavior**, the same lesson this project has already learned once for a different mechanism
(Policy Discovery's own escape-hatch-without-cost finding, and the still-unresolved Link 2 question
from the Role Meaning thread). A failure here would specifically argue the next iteration needs to
make the signal harder to ignore, not that the signal itself is wrong. A high false-positive rate
would instead argue the mechanical check itself needs refinement before its output is trustworthy.

---

# Recommendation For A First Learning Iteration

Add the smallest possible version of the mechanical vocabulary-discrepancy check as a passive,
printed note — no gate, no new artifact, no blocking behavior — and let it run across real
dogfooding sessions as they naturally occur. Measure, over time, whether flagged terms correlate
with stories that later get written. This is the smallest change that could be made to the existing
pipeline (a side observation, not a control-flow change), it targets the single best-evidenced
concern category in the entire investigation chain, and its result — either direction — teaches
something the chain does not yet know: whether backlog discovery is a real, actionable capability
or a correct observation that still needs a stronger mechanism than passive surfacing to matter.
