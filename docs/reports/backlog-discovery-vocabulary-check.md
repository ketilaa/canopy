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

### Finding 4 — All three response options were used naturally, without any artificial pressure to do so

`Not sure` was never used as a dumping ground — every instance had a specific, stated reason
(compound-phrase overlap: `location`/`levels`; concept-vs-synonym ambiguity: `issues`/`settled`/
`delivery`). `Meaningful` was reserved for terms naming a genuinely absent, distinct concept
(`inventory`, `warehouse`, `team`, `customers`) — every one of these, read back now, still looks
correct on reflection. This directly answers the question this run was designed to test: **yes, a
real (attentive, unscripted) judgment does naturally separate `catalog`-shaped signal from
`stays`-shaped noise** — not because the detector got smarter, but because the review step itself
absorbed the noise, exactly as the design predicted it might.

### Finding 5 — Noise did not cause friction, but this evidence is limited to one attentive reviewer

Across 21 judgments, dismissing a `not-meaningful` term took exactly the same one keystroke as
confirming a `meaningful` one — no hesitation, no extra step, no visible cost to being shown a
noisy candidate. This is a real, disclosed data point in favor of tolerating imprecision rather
than adding a filter now. **Important limitation, stated plainly**: this session's reviewer (this
agent) already understood the mechanism's purpose going in, which is not the same as an
independent human encountering it cold, with no context, possibly repeatedly across many sessions
where fatigue could change the answer. This evidence supports "noise is tolerable for an attentive
reviewer," not yet "noise is tolerable for every real user in practice."

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

## Open items for the next run

- **Resolved by Run #2**: whether a real, attentive reviewer naturally distinguishes signal from
  noise across the three response options. Answered yes, with reasoning recorded per term above.
- **Still open**: whether this holds for an actual independent human, not this agent acting as
  reviewer — the next real test of the hypothesis needs a session where the person interacting has
  no foreknowledge of the mechanism's purpose.
- **Still open**: no follow-through signal (whether an acknowledged gap later becomes a real story)
  is measurable yet — this needs multiple sessions over time against `stories.yaml`'s accumulated
  history, per the original design's stated signal list.
- **Deliberately not acted on yet, per explicit instruction**: the detection algorithm (stopword
  list, part-of-speech blindness) was left untouched throughout Run #2, even though several
  clear-noise terms (`reflect`, `receive`, `accurate`) would be filtered by a part-of-speech check.
  Precision tuning is the next candidate only after enough genuine human sessions establish whether
  it's actually needed, not before.
