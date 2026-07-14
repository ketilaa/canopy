---
title: "Cross-Artifact Consistency Audits Prevent Semantic Drift Even When the Right Context Was Shown"
status: draft

confidence: high

maturity: validated

themes:
  - ai-assisted-verification
  - system-design
  - specification-completeness

evidence_strength: high

source_artifacts:
  - "commit a254b25 — Add Entity Continuity gate and fix the want-field example that anchored on it"
  - "commit 0dd7073 — Add Event Continuity gate, the sibling check to Entity Continuity"
  - "reproducibility sweep, manufacturer-001 (Account-vs-Manufacturer entity divergence, live-verified)"

related_principles:
  - compute-facts-mechanically
  - deterministic-audits-vs-compensation
---

# Principle

A generated artifact can silently diverge from vocabulary, decisions, or naming already established
elsewhere in the same system, even when that established context was shown to the model directly
and correctly in the same prompt. A mechanical check comparing each newly-generated artifact
against already-known project state — and refusing to save the result on mismatch — catches this
class of failure that no amount of correct context alone reliably prevents.

# Problem That Revealed It

A reproducibility test running the same underlying request three times, from an identical starting
state, surfaced a severe divergence in one of the three runs: a generated data schema for what
should have been a "Manufacturer" entity — already established by name in the same story and in
the project's own accumulated vocabulary, both shown directly in the prompt — instead produced a
completely unrelated "Account" schema, complete with username, password, and email fields that had
no basis anywhere in the actual request. Nothing about that specific run's input differed from the
other two, aside from the model's own sampling. The correct context was present. The output still
drifted onto a different entity entirely, silently, with no signal in the output itself that
anything had gone wrong.

# Evidence

- Commit `a254b25`: "one run's entity_schema fully diverged to 'Account'... despite the story's
  as_a and the domain registry both already establishing Manufacturer as the entity." The fix
  added a mechanical, non-LLM check run immediately after generation: does the newly-generated
  entity name match an entity already known to the project? If established vocabulary exists and
  nothing matches, the operation fails loudly and nothing is saved — "a fully different domain can
  never silently persist as if accepted." Covered by dedicated tests including the exact observed
  divergence.
- Commit `0dd7073`: the same pattern applied one stage later, to domain-event naming — checking
  whether a newly-accepted event's name shares the same entity prefix as an already-established
  entity, "since that's the same class of derailment Entity Continuity already guards against one
  stage earlier." Also deliberately scoped to avoid over-triggering: it checks only the entity
  prefix, not the specific verb used, "live-verified across the reproducibility sweeps that this
  wording varies legitimately... and gating on it would just produce noise" — an explicit, tested
  decision about what to check and what to deliberately leave unchecked.
- Both checks were exercised across multiple subsequent live and reproducibility-sweep runs with
  zero false positives reported against healthy, non-divergent output — evidence the check
  distinguishes real drift from normal legitimate variation, rather than just being noisy in the
  opposite direction.

# Counter-Evidence

The Event Continuity check's own design documents a real, deliberate limitation as a counterbalance
to over-application of this principle: checking too strictly (matching the exact event verb, not
just the entity prefix) was tried conceptually and rejected, because legitimate wording variation
in a separate part of the same pipeline (event-naming conventions have some accepted ambiguity)
would have produced false positives if the check were made stricter than the entity-prefix level.
This is evidence *for* the principle applied carefully, but also a caution against it: an overly
strict consistency check can itself become a source of noise or false failures if it doesn't
account for legitimate variation elsewhere in the same system. The principle's value depends on
correctly scoping what counts as "drift" versus "acceptable variation" — getting that scope wrong
in either direction (too loose, missing real drift; too strict, flagging healthy variation) both
carry real costs.

# Applicability

- Multi-stage AI pipelines where later stages generate artifacts that should stay consistent with
  earlier-established vocabulary, decisions, or naming
- Any system where a model re-derives or re-references an entity/concept that was already
  established once, rather than referencing it by a stable identifier
- Long-running or multi-session AI-assisted workflows where context can't always fit entirely in
  one prompt

# Confidence Assessment

High. The principle rests on a directly observed, severe real-world failure (not a hypothetical),
a fix that was live-verified against the exact failure case with dedicated regression tests, a
second independent application of the same pattern to a related but distinct artifact type, and
a documented record of zero false positives across subsequent runs — evidence the check's
precision, not just its ability to catch the one case it was built for.

# Generalization

This generalizes to any AI-assisted system with more than one generation step operating over shared
context — which is most non-trivial AI pipelines. The core insight is that "the model was shown the
correct information" is not the same guarantee as "the model's output will be consistent with that
information," even for information as simple and salient as an entity's own name repeated in the
same prompt. Any pipeline that generates related artifacts across multiple calls (a multi-step
content-generation workflow, a multi-agent system where different agents produce artifacts that
must reference the same underlying concepts, a long document assembled from multiple model calls)
faces the same risk and can apply the same fix: a lightweight, mechanical, fail-loud comparison
between each new artifact and what's already been established, rather than trusting that shared
context alone guarantees consistency.

# Future Validation

The current evidence covers two closely related consistency checks (entity naming, event naming)
within one pipeline. A useful next test is whether the same pattern holds for consistency checks
that are less exact-match-friendly — checking semantic rather than lexical consistency (does a
newly-generated description contradict an earlier one, rather than does a name match) — since the
current checks all rely on relatively clean string-comparison logic. It's not yet established
whether the same reliability gain holds once the "check" itself requires more judgment to perform,
or whether that shifts the problem back into the same kind of unreliable-judgment territory this
principle was built to route around.
