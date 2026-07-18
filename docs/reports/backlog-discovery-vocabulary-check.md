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

## Open items for the next run

- Get a genuine human response (not a scripted default) to at least one instance of the question,
  ideally across a mix of real "Yes"/"No"/"Not sure" answers.
- Once several real responses exist, check whether `stays`/`organized`-shaped noise is common
  enough to warrant a lightweight part-of-speech filter, or whether it's rare enough in practice
  to leave as-is and let the "Not sure"/"No" options absorb it.
- No follow-through signal (whether an acknowledged gap later becomes a real story) is measurable
  yet — this needs multiple sessions over time against `stories.yaml`'s accumulated history, per
  the original design's stated signal list.
