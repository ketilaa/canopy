# Mechanism B: Implementation-Readiness Evaluation

Status: pre-implementation evaluation of the resolution-side design only (from the critical
review), against real corpus data — not a reopening of whether B is a real blocker class (settled),
not a taxonomy or Story Readiness theory document. Scope held deliberately narrow per instruction.

Date: 2026-07-19

Checked directly: `product-010`'s actual persisted `resolved_policies` entry
(`../canopy-e-commerce/.canopy/stories/product-010/spec.yaml`), and every existing test fixture in
`canopy-llm/src/prompts/spec.rs`'s `policy_checklist_tests` module (`bucket_policy_checklist`,
line 694) — the real, already-passing regression corpus this change must not break.

---

# Evaluation Of Current Mechanism B

The design (per the critical review): a `resolved`/`not_applicable` policy item's `detail` text
(which becomes `ResolvedPolicy.resolution`) is rejected if it reports an absence — reasons about
what the input *doesn't* say — rather than stating a rule. Checked against the one real confirmed
instance:

```
resolution: "The story does not explicitly mention any authorization requirements for browsing a catalog."
```

This contains "does not explicitly mention" — a direct, unambiguous hit for a narrow phrase set
built around self-referential absence-of-mention framing ("the input doesn't say X"), not general
negation of a domain rule ("X is not required"). That distinction is the entire design — a phrase
list scoped to the former is targeted and precise; scoped to the latter it would misfire constantly,
since "not required"/"no ... needed" is completely ordinary, legitimate phrasing for a real,
grounded resolution (see Cases That Must Not Be Blocked below, drawn from the existing test suite).

The design is sound as re-derived in the critical review and unaffected by the later business-fact
reassessment (`product-010-reassessed-with-confirmed-public-browsing-intent.md`): it targets
unsupported *reasoning*, not incorrect *conclusions*, so it doesn't matter that this specific
resolution's conclusion turned out to be right.

---

# Minimum Viable Implementation

A pure function, no new parameters, no new call site:

```rust
fn is_unsupported_absence_claim(text: &str) -> bool {
    const MARKERS: &[&str] = &[
        "does not mention", "does not explicitly mention",
        "does not state", "does not explicitly state",
        "does not specify", "no mention of", "not mentioned",
        "not specified in", "not stated in",
        "nothing in the story", "nothing indicates",
    ];
    let lower = text.to_lowercase();
    MARKERS.iter().any(|m| lower.contains(m))
}
```

Called inside `bucket_policy_checklist` (`canopy-llm/src/prompts/spec.rs:694`) at the two branches
that already enforce grounding:

- `"resolved"` arm: check `resolution` (the bound `detail` value) before pushing to `resolved`; on
  a hit, return the same `LlmError::UnexpectedShape` shape already used for missing evidence, naming
  the offending text.
- `"not_applicable"` arm: currently only checks `is_none()` without binding the values — needs a
  small restructure to bind `detail`/`evidence` and check `detail`'s content the same way, matching
  the function's own doc comment that already states both classifications are "held to the identical
  bar."

No new LLM call, no new artifact, no new pipeline stage, no new parameters threaded through
`generate_story_spec` — smaller than the evidence-traceability design in every dimension the
critical review already compared them on.

---

# Regression Cases

Real, not hypothetical — every case below is either the confirmed failure instance or an existing,
currently-passing test in `policy_checklist_tests`:

| Source | Text | Expected |
|---|---|---|
| `product-010` (real, confirmed failure) | "The story does not explicitly mention any authorization requirements for browsing a catalog." | **Rejected** — contains "does not explicitly mention" |
| `resolved_item_with_evidence_becomes_a_resolved_policy` (existing test) | "name must be unique" | Accepted (unaffected) |
| `not_applicable_item_with_grounding_produces_no_output` (existing test) | "no other entities exist" | Accepted (unaffected) — a genuine structural fact, not an absence-of-mention report; does not match any marker |
| `resolved_without_evidence_fails_loudly...` (existing test) | "records persist indefinitely" | Already rejected today (missing `evidence`), for an unrelated reason — must stay rejected, for the same reason, not a new one |

New test to add: a `resolved` item whose `detail` is `product-010`'s actual text, `evidence` present
(so the *only* thing that should trigger rejection is the new check, isolating it from the existing
presence check) — the sharpest possible regression test, since it's the literal real failure with
nothing else confounding it.

---

# Cases That Must Not Be Blocked

- **Every existing `policy_checklist_tests` fixture that currently produces a `resolved` or
  `not_applicable` output** (`"name must be unique"`, `"no other entities exist"`) — checked above,
  none match the marker set. This is the primary regression bar: the change must not turn any
  currently-passing test red.
- **A legitimate domain-grounded "not required" resolution** — e.g., what a correct resolution for
  `product-010`'s authorization question would have looked like, now that the true business fact is
  known: `"authorization not required — catalog browsing is intentionally public"`. Contains "not
  required," not "does not mention" — correctly passes. This is the case that most directly tests
  whether the phrase set is scoped narrowly enough: it uses negation, just not the self-referential
  absence-of-mention shape the check targets.
- **A `not_applicable` citing a genuine structural absence** (e.g., "no persistence exists for this
  read-only query") — uses "no," not any listed marker; passes. Structurally identical in spirit to
  the existing `"no other entities exist"` fixture.

---

# Confidence Level

**High for the target field and phrase set's precision against the real corpus on hand; moderate
for recall beyond it.** Every real case checked — the one confirmed failure, every existing test
fixture, and the plausible legitimate "not required" phrasing — sorts correctly. The acknowledged
limit, unchanged from the critical review: a differently-worded absence report ("does not include,"
"the specification is silent on") would not match this specific marker set and would pass
uncaught. That is a real recall gap, not a precision risk, and not resolved by more analysis — only
by dogfooding against a wider real sample, which is exactly what "implement → measure → learn"
calls for rather than a reason to delay.

---

# Recommendation

**Implement now**, as the narrow v1 already described above — "implement now" and "implement a
narrower v1" collapse to the same answer here, since the correctly-scoped design (per the critical
review's own correction) already is the narrow version, not a larger design cut down. Collecting
more evidence first is not recommended: the confirmed failure instance's exact text is in hand, the
full existing regression corpus is in hand and already checked by hand above, and the residual risk
(recall against unseen phrasing) is not something further analysis can close — only real dogfooding
against new stories can, which this implementation should proceed to enable rather than wait on.

---

# Implementation and Measurement Results (2026-07-19)

Built exactly as specified above: `is_unsupported_absence_claim` (`canopy-llm/src/prompts/spec.rs`),
wired into both the `"resolved"` and `"not_applicable"` arms of `bucket_policy_checklist`.
`canopy-prompt-reviewer` found no issues with the check's design or scoping; it did catch a real
doc-comment placement bug during review (the new function's doc comment had merged with the
pre-existing one with no blank-line separation, which would have made rustdoc attach the whole
combined block to the wrong function and leave `bucket_policy_checklist` undocumented) — fixed by
reordering `is_unsupported_absence_claim` to follow `bucket_policy_checklist` in the file instead of
preceding it, each with its own correctly-attached doc comment.

**Historical instances it catches**: one — `product-010`'s real `authorization` resolution ("The
story does not explicitly mention any authorization requirements for browsing a catalog"),
verified via a regression test built specifically to isolate the new check from the pre-existing
evidence-presence check (real `evidence` text supplied, so a genuine `evidence`-quotes-the-story
case is confirmed insufficient to pass — only the absence-claim check can be causing the
rejection).

**False positives found**: none, against every case checked — the full pre-existing
`policy_checklist_tests` regression suite (15 tests, including the two real legitimate fixtures,
`"name must be unique"` and `"no other entities exist"`), plus the constructed-but-realistic
counter-example using the now-known-correct business resolution for `product-010`
("Authorization not required — catalog browsing is intentionally public").

**Newly surfaced failures**: none beyond the one already-confirmed instance. No other real
`resolved_policies` data exists to test against — `product-010` is the only story in the current
real dogfooding project with any `resolved_policies` entry at all (confirmed by direct inspection),
and `manufacturer-001`'s raw artifacts no longer exist on disk. This is stated plainly rather than
implied away: the real corpus this measurement draws from is one confirmed positive case and a
handful of pre-existing fixture negatives, not a broad sample. The open question this iteration is
meant to answer — does this narrow signal prove useful in real dogfooding — remains open until a
new real story runs through `canopy spec` and either fires correctly, stays quiet correctly, or
does neither.

Build and full workspace test suite green throughout (`cargo build --workspace`,
`cargo test --workspace`) — no regressions in any other crate.
