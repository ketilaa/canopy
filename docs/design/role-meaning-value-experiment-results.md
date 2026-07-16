# Role Meaning Value Experiment — Results

Status: results of a live run, not a redesign. Executes `docs/design/role-meaning-value-
validation.md`'s methodology for real, against a real local LLM, using the standalone example
`canopy-llm/examples/role_meaning_value_experiment.rs`. Strictly read-only against the dogfooding
project; every result below is printed output, nothing was saved.

Date: 2026-07-17

Run: `cargo run -p canopy-llm --example role_meaning_value_experiment -- /Users/ketil/code/ketilaa/
canopy-e-commerce manufacturer-001`, four conditions (`none`, `internal`, `external`, `affiliated`),
each a single call — **N=1 per condition**, not a repeated-run comparison. Every finding below is
stated with that scope attached; this is a first look, not a reproducibility-tested result.

---

# Summary

**Policies and Specifications both show the theorized effect, for two of the three role-fact
conditions — and the third condition's negative result is itself informative, not a null.**
`authorization` moved from a correctly-unresolved open question (baseline) to a citation-backed
`resolved` classification in both the `internal` and `external` conditions, each citing the
injected role fact by name as evidence. In lockstep with that, both conditions also produced a new
scenario type — an actor-lacks-the-required-role rejection — that never appeared in the baseline
or in the `affiliated` condition. The `affiliated` condition left `authorization` unresolved and
produced no such scenario, a genuine, mechanically-consistent negative result worth taking
seriously rather than averaging away.

---

# Policies — the `authorization` Area, Across Conditions

| Condition | `authorization` classification | Evidence cited |
|---|---|---|
| `none` (baseline) | Unresolved (in `open_questions`) — correct, no basis | — |
| `internal` | **Resolved**: "requires the 'Manufacturer Representative' role" | "Role Definition: Manufacturer Representative: Internal" |
| `external` | **Resolved**: "requires a specific role or permission beyond the actor already being authenticated" | "Role Definition: Manufacturer Representative: External" |
| `affiliated` | Unresolved (in `open_questions`) — unchanged from baseline | — |

This is the single most directly attributable result in the whole run. The citation-requirement
mechanism (`unresolved-decisions-become-explicit-decision-points`) worked exactly as documented in
both directions: it correctly left `authorization` unresolved at baseline (no supporting fact
existed), and it correctly resolved it, with a real, checkable citation pointing at the injected
fact, once one did. Note that `uniqueness` also moved between conditions (resolved at baseline and
`internal`, unresolved at `external`) — this is ordinary run-to-run variance already documented
elsewhere in this project's reproducibility work, not attributable to the role fact, since nothing
about the injected fact concerns uniqueness at all.

**The `affiliated` result is a genuine negative, not a gap in the experiment.** The same fact-
injection channel was used, the same way, with the same mechanism reading it — but the model did
not treat "a recognized, ongoing but organizationally separate party" as grounds to resolve
`authorization`, where it did treat "an employee" and "a representative of the manufacturer" as
grounds to resolve it. Two explanations remain genuinely open with only one run each: this could be
sampling variance (the same kind already seen in the `uniqueness` area across conditions), or it
could be real content-sensitivity — `docs/design/role-classification-stability-test.md` already
flagged `affiliated` as the harder-to-articulate category, and a softer, hedged classification may
simply be less obviously actionable to the model than a clean binary. This run cannot distinguish
the two explanations; only repeated runs per condition could.

---

# Specifications — Scenario Changes, Across Conditions

Exactly in step with the Policies result: **both conditions that resolved `authorization` also
generated a new scenario type that had no equivalent at baseline or in `affiliated`.**

- `internal`, scenario 11: *"Reject registration when the actor does not have the required role"*
  — `given`: "The actor is authenticated but does not have the Manufacturer Representative role."
- `external`, scenario 10: *"Reject registration when the actor lacks the required role"* —
  `given`: "The actor is authenticated but does not have the required role."
- `affiliated` and `none`: no equivalent scenario in either.

This is a concrete, checkable instance of the value the design doc named as a possible outcome: a
specification became more complete — a real gap a Product Owner would want covered (what happens
when someone is authenticated but not authorized) that the baseline specification simply never
raised, because nothing in the baseline run ever resolved `authorization` in the first place. The
two artifact categories moving together — resolved policy, new scenario; unresolved policy, no new
scenario — is itself a coherent, mechanically sound pattern, not two independent coincidences.

Field-naming variance across conditions (`contactEmail` vs `websiteUrl`/`website`, `phone` vs
`contactPhone`) is present in every condition including `none`, and matches this project's already-
documented generation variance unrelated to the experimental variable — noted so it isn't mistaken
for an effect of role meaning.

---

# Reviews — Checked Against the Product-Owner Perspective Experiment's Own Findings

Per the design doc's own scoping decision, this was a targeted re-check against the two findings
most directly attributable to missing role meaning — the Governance-Oriented persona's
authorization gap and the Terminology-mapping role-semantics ambiguity — applied directly, not via
a fresh five-persona review pass.

- **"Authenticated as what, and authorized to do what? ... Nothing decides this."** — measurably
  addressed in the `internal`/`external` conditions: something now does decide it (a resolved,
  cited policy), and a scenario now exists describing the consequence of lacking it. Not addressed
  in `affiliated` or baseline.
- **Role-semantics ambiguity itself (internal or external?)** — resolved by construction in this
  experiment, since the injected fact *is* the assumed answer to that question; this was never
  something the experiment could test independently of supplying an answer, and isn't claimed as a
  discovery here.
- **What was *not* addressed, in any condition**: the same persona's separate findings about who
  consumes `ManufacturerRegistered`, what its payload contains, and whether an approval step exists
  before registration takes effect. None of the four runs touched any of these — correctly so, since
  role meaning was never theorized to address them. Stating this plainly to avoid overclaiming a
  broader "governance concerns resolved" result than what actually happened: this is a precise,
  bounded improvement to one specific finding, not a general uplift in specification governance.

---

# Domain Understanding — Structural Check, Not a Live Run

Verified by direct code reading, not execution — `stories_from_intent_prompt`
(`canopy-llm/src/prompts/intent.rs:5-27`) is a private function and cannot be called from a
standalone example without changing its visibility, which would be a production code change this
investigation's charter excludes. Re-confirmed the exact logic already established in
`role-semantics-investigation.md`: when a role has a description, this function renders it as
`"{name} — {description}"` into the next `intent` call's "Known roles" context; when it doesn't, it
renders the bare name. This is deterministic, pure Rust with no model involved — confirmed correct
by reading, not in need of a live run to verify. **What remains untested**: whether a *second* real
story that reuses the role actually produces different downstream content as a result — this
project has never had a second role or a second fully-specced story to observe that against, and
manufacturing one solely to test this would repeat the exact anticipatory-evidence pattern this
project has already declined more than once.

---

# What This Run Does and Doesn't Justify Concluding

**Justified**: for `manufacturer-001`, at this frozen input, supplying an explicit role-meaning fact
through the same context channel an ADR already uses caused a real, traceable, citation-backed
change in the `authorization` policy area and a corresponding new scenario, in two of three tested
classifications — a genuine "Policies" and "Specifications" value signal, not a wiring failure
(the fact was demonstrably read and used, twice). This is the first live evidence for a claim this
project's design chain had, until now, only argued for structurally.

**Not justified**: any claim about repeatability (N=1 per condition — the `affiliated` non-result
could be content-sensitivity or could be sampling noise, and this run cannot tell which),
generalization to a different role or story (only `manufacturer-001` has a complete real artifact
set to test against), or a general uplift across every governance concern the original review
raised (only the one it was built to address moved; the others, correctly, did not).

# Natural Next Step

A repeated-run version of this same comparison — several runs per condition, the same tiered
variance classification this project already uses elsewhere — would distinguish whether the
`affiliated` result is stable content-sensitivity or ordinary sampling noise. Not started here,
consistent with this document's own scope: report the run that happened, not propose the next one
as a commitment.
