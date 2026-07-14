---
title: "Unresolved Business Decisions Should Become Explicit Decision Points, Not Silent Interpretations"
status: draft

confidence: high

maturity: validated

themes:
  - human-in-the-loop
  - business-rules
  - specification-completeness

evidence_strength: high

source_artifacts:
  - "commit e61043f — Record Decision Points and the enumeration-over-holistic-review principle"
  - "commit 0fd89d7 — Force per-item policy classification instead of a freeform resolved/open split"
  - "commit c77f322 — Require textual evidence before Policy Discovery may classify a policy resolved"
  - "reproducibility sweep, manufacturer-001, 2026-07-14 (5/6 confidently-resolved-with-no-basis → 1-2/6, remainder correctly surfaced as open questions)"

related_principles:
  - compute-facts-mechanically
  - exhaustive-enumeration-over-holistic-review
---

# Principle

When a language model encounters a genuine business question with no supporting basis in the
information it has been given, the system should force that question into a first-class, visible,
human-reviewable artifact rather than allow the model to silently pick an interpretation and
proceed as if the question had been answered.

# Problem That Revealed It

This started as a design prediction, then became a concretely observed bug. The prediction, made
while designing an earlier stage of the same system: "a small model asked to extract [requirements]
will not stop and ask what an unresolved question should mean, it will pick an interpretation, and
that becomes a hidden business decision with no record it was ever made." The design response was
to make unresolved business questions (duplicate-handling policy, uniqueness rules, default values,
and similar) into a first-class, gated artifact rather than a note buried in generated output.

The concrete case that proved the prediction correct, later, was sharper than the original design
discussion anticipated. A checklist-driven step explicitly asked a model to classify six named
business-policy questions into "resolved," "not applicable," or "unresolved" — a scheme that should
have made silent interpretation structurally impossible, since "unresolved" was an explicitly
offered, low-friction option. Inspecting a live-generated result anyway showed the model had
confidently "resolved" most of these questions with specific, invented answers — a role
requirement, a retention period, a default value — none of them present anywhere in its actual
inputs. Having an explicit option to defer a decision did not, by itself, stop the model from
making the decision unasked.

# Evidence

- Commit `e61043f`: the original design rationale, written before any of the later confirming
  evidence existed — "Elevates unresolved business questions... to a first-class, gated Decision
  Point artifact instead of a note that planning can silently work around... [a model] will not
  stop and ask what an unresolved question should mean, it will pick an interpretation, and that
  becomes a hidden business decision with no record it was ever made."
- A live-generated specification showing the predicted failure concretely: five of six policy
  questions "resolved" with specific, unsupported answers, in a step whose own instructions already
  offered an explicit "unresolved" classification.
- Commit `0fd89d7`: the first fix attempt — forcing the model to classify every named question
  explicitly rather than allowing a freeform response — closed a related bucketing bug but, on its
  own, did not stop the underlying fabrication.
- Commit `c77f322`: the fix that actually changed outcomes — requiring every "resolved" (and
  "not applicable") answer to cite a specific, checkable source, and treating an answer with no
  citation as invalid output rather than an acceptable low-confidence guess.
- A controlled reproducibility comparison before and after this fix: before, most runs resolved 5
  of 6 questions with unsupported specifics; after, runs resolved 1–2 of 6, with the remainder
  correctly routed to an explicit open question a human is expected to answer.

# Counter-Evidence

The mechanism that actually forces genuine deferral turned out to be more specific than "offer an
unresolved option and instruct the model to use it." That instruction alone (the state of the
system when the fabrication was first observed) was insufficient — evidence directly contradicts
treating "provide an escape hatch" as sufficient on its own. What worked was pairing the escape
hatch with an external, structural cost for not using it (a required citation, checked outside the
model's own self-report). This narrows the principle: it is not enough to make deferral *possible*;
deferral has to be made *cheaper than fabrication*, or the model will not reliably choose it.

# Applicability

- Business-rule and policy discovery in AI-generated specifications
- Any workflow where a model is asked to fill a gap in incomplete input rather than flag the gap
- Requirements-extraction and requirements-completeness tooling more broadly

# Confidence Assessment

High. The principle was stated as a design prediction before the evidence existed, and the
evidence that followed — a concretely observed fabrication, a first fix that didn't fully work, and
a second fix (evidence-grounding) that produced a measured, repeated shift in behavior across a
controlled comparison — matches the prediction closely enough to treat it as validated rather than
merely plausible. The one caveat (an escape hatch alone is not sufficient) is itself well-evidenced,
which strengthens rather than weakens overall confidence in the refined version of the principle.

# Generalization

This generalizes directly to any AI-assisted system extracting requirements, configuration, or
business logic from incomplete source material — legal document review, contract analysis,
data-migration rule inference, or any pipeline where a model fills gaps in what a human actually
specified. The core risk being guarded against — an AI system quietly making a decision a human
should have made, with no trace that a decision point ever existed — applies to any domain where
downstream consequences depend on assumptions the model had no basis to make. The refinement (an
escape hatch needs an enforced cost for not using it, not just an instruction to use it) is likely
the more broadly useful half of the lesson, since "offer an unresolved/unknown option" is common
advice that this evidence shows is necessary but not sufficient on its own.

# Future Validation

The current enforcement mechanism (require a named citation, reject answers without one) closes
the fabrication-with-zero-basis case. It does not yet verify that a cited source genuinely supports
the specific claim made — evidence from the same work shows the model sometimes cites a real,
existing source that doesn't actually say what's being claimed. A future test of a stricter
citation requirement (quoting the exact supporting substring, rather than naming a source category)
would clarify whether that closes the remaining gap or whether an even stronger enforcement
mechanism is needed once fabrication-with-a-real-but-irrelevant-citation becomes the dominant
remaining failure mode.
