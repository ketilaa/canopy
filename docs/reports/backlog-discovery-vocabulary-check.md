# Report — Backlog-Discovery Vocabulary-Discrepancy Check

A running record of live sessions against the vocabulary-discrepancy meaningfulness check —
`canopy intent`'s first learning-loop iteration for the backlog-discovery hypothesis (see
`docs/design/backlog-discovery-corrected-intervention.md` for the design, and
`docs/design/capability-relationship-types-reassessment.md` for why this relationship type,
dependency/vocabulary-discrepancy, was chosen over the initially-proposed lifecycle heuristic).
Kept in date order, one file per mechanism, matching `docs/reports/manufacturer-001.md`'s shape.

---

## Run #1 (2026-07-18) — First live firing, real project, real LLM

**Setup:** a dogfooding e-commerce project already containing `manufacturer-001`
(`domain_registry.yaml`: `Manufacturer` entity, `ManufacturerRegistered` event). Model:
Qwen2.5-Coder-14B-Instruct-GGUF, served locally. Driver: a scripted pseudo-terminal session
(`expect`), since `intent`'s story-review gate needs a real pty. Canopy binary: freshly built and
installed from commit `2fbb455` (the commit that shipped this check).

**Statement given:** "As a catalog manager, I want to register a product model so that the
catalog stays organized." Chosen to exercise the mechanism against fresh input, not a replay of
the already-known `manufacturer-001`/`order-001` cases.

**What happened, in order:**

1. Story derived and accepted as `catalog-001` (`want: register a product model`,
   `so_that: the catalog stays organized`).
2. Domain extraction added `ProductModel` (entity) and `ProductModelCreated` (event) —
   confirmed correct by direct inspection of the LLM call/response in the debug log.
3. The new check ran against `catalog-001`'s `so_that` clause using the domain registry as it
   stood immediately after step 2, and fired three times:
   - `catalog` — "Is that gap meaningful?"
   - `stays` — "Is that gap meaningful?"
   - `organized` — "Is that gap meaningful?"
4. All three were answered by the driving script sending a bare `Enter` — i.e. the default
   option, **Yes** — at each prompt.
5. All three fired correctly to `review-log.yaml` under `category: vocabulary-discrepancy`,
   with `story_id: catalog-001`, the exact flagged term as `subject`, and `outcome: meaningful`.

```yaml
- timestamp: 2026-07-18T09:32:22Z
  command: intent
  story_id: catalog-001
  category: vocabulary-discrepancy
  subject: catalog
  outcome: meaningful
- timestamp: 2026-07-18T09:32:22Z
  command: intent
  story_id: catalog-001
  category: vocabulary-discrepancy
  subject: stays
  outcome: meaningful
- timestamp: 2026-07-18T09:32:22Z
  command: intent
  story_id: catalog-001
  category: vocabulary-discrepancy
  subject: organized
  outcome: meaningful
```

### Finding 1 — The mechanism fires and logs correctly end to end

Detection, question wording, and logging all worked exactly as designed against a real story,
with no code changes needed after the live run. This confirms the plumbing (mechanical scan →
`select_required` prompt → `record_review` → `review-log.yaml`) is sound before any judgment about
the signal's value is drawn.

### Finding 2 — Real, disclosed evidence on detection precision: 1 genuine candidate, 2 noise

Of the three terms flagged, only `catalog` is a plausible missing-domain-concept candidate in the
sense the design intends. `stays` (a verb) and `organized` (a participle) are false positives — the
stopword list (deliberately generic function words only, not tuned to this test sentence) has no
part-of-speech filtering, so any content word absent from the domain registry gets flagged
regardless of whether it's noun-shaped. This is the first real data point on the false-positive
rate the design explicitly named as a signal to watch, not a defect discovered after the fact —
finding it was the point of running a live session before drawing conclusions.

### Finding 3 — This run does not yet contain a genuine meaningfulness judgment

**Important caveat, disclosed rather than glossed over:** all three "Yes" answers came from the
`expect` script's default keypress, not a human actually weighing whether each flagged term named
a real gap. This run validates that the *mechanism* works — the right question, at the right
moment, logged correctly — but it does not yet constitute real evidence about whether humans find
the signal meaningful. That question needs a session where a person genuinely reads each prompt
and answers from judgment, including at least one real "No" or "Not sure" to confirm the negative
paths behave correctly too (untested so far — every response in this run took the same branch).

---

## Run #2 (2026-07-18) — Four genuine, unscripted sessions; no algorithm changes

Directly answers Run #1's open item: get real judgments, not scripted defaults. Explicit
constraint honored throughout — `find_vocabulary_discrepancies` and its stopword list were not
touched between or during these sessions; the goal was to observe the existing signal, not tune it.

**Method:** four fresh behavioral statements, each in a different domain (warehouse/inventory,
support/complaints, finance/payments, delivery/shipment) run through a real `canopy intent`
session in the same dogfooding project used in Run #1, driven interactively via `tmux` one
keystroke at a time. At each "Is that gap meaningful?" prompt, the flagged term was read and
judged genuinely — reasoning recorded below — rather than defaulted to "Yes."

**A methodology defect surfaced mid-session 1, disclosed rather than smoothed over:** two rapid,
un-verified keystrokes caused `counts` to be recorded as `not-meaningful` when the intended
judgment was `not-sure`, and caused `across` to be recorded as `meaningful` via a stray confirming
keystroke that fired before any real judgment was made on it. Both entries stayed in
`review-log.yaml` uncorrected — the project's own standing rule against hand-patching a generated
artifact after the fact applies here too. The driving technique was corrected immediately after
(single keystroke, capture pane, verify cursor position, only then confirm) and no further slips
occurred across the remaining 18 judgments. Both tables below report the log exactly as recorded,
with the two affected rows flagged.

### Full results, all 4 sessions (21 flagged terms)

| Story | Term | Recorded outcome | Reasoning |
|---|---|---|---|
| warehouse-001 | inventory | meaningful | Names a real concept (running stock levels per location) distinct from the story's own `StockAdjustment` |
| warehouse-001 | levels | not-meaningful | Part of the compound "inventory levels" — the noun concept is `inventory`, already flagged separately |
| warehouse-001 | reflect | not-meaningful | Verb |
| warehouse-001 | real | not-meaningful | Adjective |
| warehouse-001 | counts | **not-meaningful*** | *Intended `not-sure` — "stock count" is a real, distinct concept in some inventory systems, but redundant with `inventory` here; recorded value is a keystroke-timing slip, not the genuine judgment |
| warehouse-001 | across | **meaningful*** | *Stray confirmation before any real judgment was formed — genuine call would have been `not-meaningful` (a preposition) |
| warehouse-001 | warehouse | meaningful | Names a location/facility concept distinct from `StockAdjustment` |
| warehouse-001 | location | not-sure | Could be redundant with `warehouse`, or a genuinely distinct sub-location/bin concept — real ambiguity |
| support-001 | quality | not-meaningful | Modifies "team" (an actor reference), not itself a domain noun |
| support-001 | team | meaningful | A plausible distinct actor/capability (a future "review flagged complaints" story), same shape as the downstream-consumer pattern found earlier in this investigation |
| support-001 | review | not-meaningful | Verb here, and redundant with `team` if `team` becomes its own story |
| support-001 | recurring | not-meaningful | Adjective modifying "issues" |
| support-001 | issues | not-sure | Plausibly just a synonym for `CustomerComplaint` (the entity already extracted), or a broader issue-tracking concept complaints roll up into — real ambiguity |
| finance-001 | outstanding | not-meaningful | Adjective; the noun it modifies (`invoices`) was already correctly extracted as `Invoice` |
| finance-001 | marked | not-meaningful | Participle, part of the already-extracted `InvoiceMarkedSettled` event |
| finance-001 | settled | not-sure | Describes invoice state (covered by the extracted event), but could hint at a distinct `Settlement` record — real ambiguity |
| delivery-001 | customers | meaningful | Same forward-reference shape as the original `Product`/`Order` finding |
| delivery-001 | receive | not-meaningful | Verb |
| delivery-001 | accurate | not-meaningful | Adjective |
| delivery-001 | delivery | not-sure | Could be redundant with `Shipment`/`DeliverySlot`, or denote a distinct "completed delivery" record — real ambiguity |
| delivery-001 | windows | not-meaningful | Synonym for the already-extracted `DeliverySlot` |

### Distribution

Using the log exactly as recorded (21 terms, including the 2 disclosed slips):

| Outcome | Count | Share |
|---|---|---|
| meaningful | 5 | 24% |
| not-meaningful | 12 | 57% |
| not-sure | 4 | 19% |

Using the genuine intended judgment instead (correcting only the 2 disclosed slips — `counts` →
not-sure, `across` → not-meaningful):

| Outcome | Count | Share |
|---|---|---|
| meaningful | 4 | 19% |
| not-meaningful | 12 | 57% |
| not-sure | 5 | 24% |

### Finding 4 — Signal-validation evidence: the flagged terms sort into distinguishable classes

`Not sure` was never used as a dumping ground — every instance had a specific, stated reason
(compound-phrase overlap: `location`/`levels`; concept-vs-synonym ambiguity: `issues`/`settled`/
`delivery`). `Meaningful` was reserved for terms naming a genuinely absent, distinct concept
(`inventory`, `warehouse`, `team`, `customers`). This shows the *signal itself* is a classifiable
mix — not uniform noise, not uniform gold — and that the three-option review interaction is
expressive enough to represent that mix without forcing anything into the wrong bucket.

**What this finding does not show, stated precisely so it isn't overread**: this classification
was performed by this agent, who went in already knowing the hypothesis, the implementation, and
which distinctions the experiment was designed to surface. That makes this evidence about the
*signal's composition* (tier 2 below) — it is not evidence that an independent human, encountering
these prompts with no prior context, would sort them the same way, as easily, or at all. That
question is still open; see "Three tiers of validation" below.

### Finding 5 — Mechanical observation: dismissing noise took no extra steps for this reviewer

Across 21 judgments, dismissing a `not-meaningful` term took exactly the same one keystroke as
confirming a `meaningful` one — a plain, countable fact about the interaction, not a usability
verdict. **This does not support a conclusion about noise tolerance, usability, or friction for a
real user.** This agent is not an independent human: prior knowledge of the mechanism's purpose
plausibly makes classification faster and more confident than it would be for someone encountering
the prompt cold, possibly repeatedly, with no stake in the experiment's outcome. Keystroke-count
parity is a mechanism-level fact (tier 1); whether noise is *tolerable* is a human-validation
question (tier 3) this run cannot answer.

### Finding 6 — Domain extraction quality visibly changes how much the check has left to flag

`finance-001` produced only 3 flagged terms (all noise or ambiguous, zero clear signal) because
domain extraction itself correctly captured both `Payment` and `Invoice` from the story text
before the check ran — the same term (`invoices`) that would have been the clearest signal in a
weaker extraction went unflagged because it was already known. This wasn't true in the other three
sessions, where extraction only picked up the story's primary created entity and left the
referenced-but-unactioned concept (`inventory`, `team`'s implied actor, `customers`) for this
check to catch. The two mechanisms are already working as complements, not redundant with each
other.

### Finding 7 — A related, disclosed defect: story-decomposition scope creep in session 4

Independent of the check under test: the delivery/shipment statement produced 5 stories from one
requested action, in a domain area other than the vocabulary check with no story of that shape
yet on file — a live reproduction of exactly the "one intent action = one story" violation
documented as reproducible elsewhere in this project's own retrospectives. Handled the way an
actual reviewer would: accepted only the story matching what was asked (`delivery-001`), rejected
the other four as over-decomposition. Noted here for completeness, not investigated further — it's
orthogonal to the vocabulary-discrepancy check and already a known, tracked pipeline issue.

---

## Three tiers of validation — what Runs #1 and #2 actually support

Kept as its own section because the three questions are easy to blur together, and blurring them
is exactly what would make this report overclaim. Each is a different question, answered by
different evidence, and progress on one does not transfer to the others.

| Tier | Question | Status | Evidence |
|---|---|---|---|
| 1. Mechanism validation | Does the check fire, render, and log correctly? | **Supported** | Run #1 + Run #2: detection, question wording, and `review-log.yaml` entries all worked correctly across 5 real sessions and 24 flagged terms, no code changes needed. |
| 2. Signal validation | Does the underlying signal (referenced-but-uncaptured term) actually contain a distinguishable mix of real gaps, noise, and genuine ambiguity? | **Partially supported** | Run #2: 21 terms sorted cleanly into meaningful/not-meaningful/not-sure with a stated reason each, under one reviewer's classification. Suggestive that the mix is real and classifiable — not yet confirmed independent of who's doing the classifying. |
| 3. Human validation | Do independent humans find the signal useful — tolerate the noise, use all three options naturally, treat it as worth their attention? | **Not supported — open** | No data yet. Every judgment so far (Run #1's defaults, Run #2's 21 classifications) came from either a scripted default or this agent, not an independent human with no foreknowledge of the hypothesis. |

**The strongest conclusion these two runs support:** the mechanism works mechanically, and
produces a mix of signal and noise that can be classified.

**The strongest conclusion these two runs do *not* support, and must not be read as supporting:**
independent humans find the signal useful. That question — usability, noise tolerance, whether the
three response options get used naturally by someone without inside knowledge, whether the signal
is worth a real user's attention at all — remains entirely open. Findings 4 and 5 above were
revised to stop short of claiming otherwise.

---

## Open items for the next run

- **Tier 1 (mechanism): considered closed** unless a future session surfaces a new plumbing defect.
- **Tier 2 (signal): still accumulating** — more sessions, ideally across more varied domains and
  more reviewers (not only this agent), would strengthen or weaken confidence that the
  meaningful/noise/ambiguous mix generalizes.
- **Tier 3 (human validation): not started.** The next real test needs a session where an actual
  independent person — no foreknowledge of the hypothesis, the implementation, or the desired
  distinctions — encounters the prompts and responds. Nothing run so far substitutes for this.
- **Still open, independent of the above**: no follow-through signal (whether an acknowledged gap
  later becomes a real story) is measurable yet — this needs multiple sessions over time against
  `stories.yaml`'s accumulated history, per the original design's stated signal list.
- **Deliberately not acted on yet, per explicit instruction**: the detection algorithm (stopword
  list, part-of-speech blindness) was left untouched throughout Run #2. Precision tuning is the
  next candidate only after tier 3 evidence establishes whether it's actually needed, not before.
