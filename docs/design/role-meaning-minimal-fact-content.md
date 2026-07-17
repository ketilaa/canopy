# Role Meaning as a Fact Type — What Minimal Content Must It Capture?

Status: content-shape analysis only. Not a collection-mechanism question — how the fact is
obtained, presented, or stored is out of scope here entirely. Answers what information a Role
Meaning fact must contain to satisfy the survivability properties already established
(`docs/design/why-role-meaning-succeeded-analysis.md`): closed-set/quotable, single-purpose,
consumer-targeted, shape-compatible, concrete/operational. Uses only evidence already produced in
this chain.

Date: 2026-07-17

**Framing accepted directly**: the existing `Role::Described { name, description }` mechanism —
free text, open-ended, no forced structure — is the wrong shape for what this chain has shown
survives the pipeline. What follows treats Role Meaning as its own fact type, not as "a better
description," and asks what it minimally needs to contain.

---

## Candidate 1 — The Classification Value Itself

**What it is**: a single value from a bounded set, answering the more primary question the
Role-Classification Stability Test identified underneath the original internal/external framing —
*on whose behalf does this actor act in this interaction* — with `internal`, `external`,
`affiliated`, and `unresolved` as the four values that set resolves to.

- **Evidence it matters**: this is the exact content that produced the only measured causal effect
  in the whole chain — the Role Meaning Value Experiment's `authorization` resolution, citation-
  backed, in 2 of 3 tested conditions.
- **Evidence against**: the `affiliated` value specifically failed to be consumed in the one test
  run, with the cause left genuinely unresolved (content-sensitivity vs. sampling noise). The
  four-value set itself is not settled — it superseded an earlier three-value set after the
  stability test found the original binary failed on a real fraction of realistic role names
  (supplier, auditor, franchise partner, contractor). A further revision is not ruled out by
  anything tested since.
- **Downstream consumer**: the `authorization` checklist area, directly confirmed.
- **Operationally specific**: yes — this is the clearest instance of the property in the whole
  chain; the value itself, once selected, requires no further extraction or interpretation.
- **Satisfies survivability properties**: yes for `internal`/`external`; unresolved (not
  disconfirmed, but not confirmed either) for `affiliated`; `unresolved` itself was never actually
  exercised as a distinct input in the experiment — every tested condition supplied a substantive
  value, and the "no fact supplied" baseline is not the same claim as "a human explicitly selected
  unresolved." Whether those two inputs behave identically downstream has never been directly
  tested, only assumed by analogy to Policy Discovery's own resolved/not_applicable/unresolved
  handling.

## Candidate 2 — An Unambiguous Identifier for Which Role the Classification Applies To

**What it is**: a name or reference tying the classification to one specific role, distinguishing
it from any other role that might exist in the same project.

- **Evidence it matters**: every successful citation in the Value Experiment named the role
  explicitly and correctly ("Role Definition: Manufacturer Representative: Internal") — the
  consuming mechanism never once misattributed a classification to the wrong role across any
  tested condition.
- **Evidence against**: none — this was never a point of failure anywhere in this chain. Its
  necessity is more assumed-obvious than separately stress-tested, since this project's real
  history has only ever had one role to name.
- **Downstream consumer**: the same `authorization` mechanism — implicitly, since the citation
  mechanism has to know which role's classification it's reading.
- **Operationally specific**: yes.
- **Satisfies survivability properties**: yes, on the evidence available, though untested against
  a project with more than one role where misattribution could actually occur.

## Candidate 3 — A Short, Single-Purpose Rationale Tied Directly to the Classification

**What it is**: one sentence justifying the selected classification specifically — not a general
description of the role, not a bundle of unrelated policy statements, just the reason *this*
classification was chosen.

- **Evidence it matters**: every fact in the Value Experiment that succeeded included exactly this
  — a `reason` field scoped tightly to the classification ("The role registers data as part of
  this business's own operations, not on behalf of an outside party"), never introducing unrelated
  claims. This is a meaningful, evidenced distinction from `Role::Described`'s own `description`
  field, which has no such scoping requirement at all.
- **Evidence against**: the chain has never tested a classification *without* an accompanying
  rationale — every tested condition included one, so the rationale's own independent necessity
  (as opposed to the classification alone) has not been isolated. It is possible the classification
  value carries the entire effect and the rationale is incidental.
- **Downstream consumer**: same as Candidate 1 — the rationale is part of what the citation
  mechanism reproduces in its `evidence` field.
- **Operationally specific**: yes, when scoped correctly — but this is exactly the dimension that
  distinguishes success from failure elsewhere in the chain. `risk_averse`'s fact (a different fact
  type — verification criteria, not role meaning, cited here only as cross-type supporting
  evidence for the same underlying property) succeeded specifically because its elaboration named
  one concrete artifact and nothing else. `compliance`'s fact, bundling three separate claims under
  one statement, is the clearest evidenced case of what a rationale must **not** do.
- **Satisfies survivability properties**: yes, conditional on staying single-purpose — the same
  property that makes the classification value itself work has to extend to whatever accompanies
  it, or the combined fact reverts to the multi-concern shape already shown to fail.

## Candidate 4 — A List of Alternatives Considered

**What it is**: present in every fact actually tested (the injected ADRs each carried an
`alternatives` list, e.g. `["External — ...", "Affiliated — ..."]`), inherited from the shape of
the `Adr` struct these facts were expressed through.

- **Evidence it matters**: none directly. No citation in any tested result ever quoted or
  referenced the alternatives list — every successful `evidence` field reproduced the `decision`/
  `reason` content, never the `alternatives`.
- **Evidence against**: none disconfirming it either — it simply was never shown to do anything.
- **Downstream consumer**: none observed.
- **Operationally specific**: not evaluated — no evidence bears on this either way.
- **Satisfies survivability properties**: unknown — this candidate's presence in every tested fact
  is best explained as incidental to the storage shape used (an ADR requires an `alternatives`
  field), not as a piece of information shown to matter. Including it in a minimal Role Meaning
  fact is not evidenced; excluding it is not evidenced either. Genuinely unresolved, not a
  negative finding.

---

## What the Evidence Argues For Excluding

Stated as its own category, since defining the minimum also requires naming what's been shown to
actively work against survivability if included:

- **General-purpose, unscoped description** — `Role::Described`'s own current shape. Never tested
  directly for Role Meaning specifically, but directly analogous to every broader, multi-concern
  fact shown to fail in Phases 2–3.
- **Bundling multiple distinct claims into one fact.** Directly evidenced as harmful:
  `growth_retention`'s fact asserted two separate things at once and only one left a trace;
  `compliance`'s fact bundled three claims and none did.
- **Content unrelated to the classification itself** (e.g., business-policy statements riding
  alongside a role's identity classification). No tested Role Meaning fact ever did this, but every
  broader persona-policy fact that failed did something structurally equivalent — introducing
  scope beyond one bounded question.

---

## Summary — Minimal Content, As Evidenced

A Role Meaning fact that satisfies every survivability property demonstrated in this chain requires,
at minimum: **one classification value from a bounded set answering "on whose behalf does this
actor act," an unambiguous reference to which role it applies to, and a short rationale scoped
strictly to justifying that one value** — nothing else has been shown to matter, and several
things (general description, bundled claims, unrelated content) have been shown to actively work
against it. The alternatives list remains a genuine open question, not a settled inclusion or
exclusion — the evidence simply never bears on it either way.
