# What Kind of Story Readiness Failure Did `product-010` Actually Reveal?

Status: diagnosis only. No mechanism proposed, no check designed, no framework sketched — answers
one question, using only evidence already on file, per explicit instruction. Directly extends
`docs/open-questions/story-readiness-vs-backlog-evolution.md` rather than replacing it; this
document supplies the deeper categorical analysis that open question deferred, not a new
investigation.

Date: 2026-07-19

Reviewed: `docs/reports/product-010-customer-vertical-slice.md`, `docs/open-questions/story-
readiness-vs-backlog-evolution.md`, `docs/design/human-insight-inventory.md` and `docs/design/
human-insight-inventory-rerun.md`, `docs/design/roadmap-reassessment.md` (Backlog Execution Plan),
`docs/principles/{unresolved-decisions-become-explicit-decision-points, cross-artifact-consistency-
audits-prevent-drift, exhaustive-enumeration-over-holistic-review, structure-emerges-from-
behavior}.md`, `docs/open-questions/domain-boundary-explicitness.md`, and the actual mechanism code:
`canopy-llm/src/prompts/spec.rs` (`bucket_policy_checklist`), `canopy-llm/src/prompts/decisions.rs`
(`extract_decisions`, `linking_prompt`, `classification_prompt`).

---

## 1. The Facts, Restated Precisely

From `product-010`'s own artifacts, not inference:

- `spec.yaml`'s `resolved_policies` contains an `authorization` entry: `detail`/`evidence`
  amounting to *"the story does not explicitly mention any authorization requirements for browsing
  a catalog."*
- `spec.yaml`'s `out_of_scope` explicitly lists "Customer authentication and authorization."
- The accepted BDD scenarios include `product-010-05` ("Customer receives an error message if they
  are not authenticated"); the accepted contract's `required_tests` include a full
  `GET /catalog without authorization` → `HTTP 401` behavior pair.
- `domain_registry.yaml`, `roles.yaml`, and `stories.yaml` contain no authentication/session/identity
  concept or story anywhere in the project, before or after this story.
- Stage 0 (`gaps: []`), Stage 2 (`decisions: []`), Stage 3 (`findings: []`), and Stage 4 (both
  contract-audit and dependency-review `findings: []`) all reported clean.

## 2. Candidate Explanations

### 2.1 Unresolved policy decision, misclassified as resolved

**Evidence for.** Directly confirmed against the actual mechanism, not just the artifact: `spec.rs`'s
`bucket_policy_checklist` (`canopy-llm/src/prompts/spec.rs:694-`) only checks that `detail` and
`evidence` are both *present* (`Some(_)`) before accepting a `"resolved"` classification — it has no
check on whether `evidence`'s *content* is a positive, on-topic fact versus a report of absence.
"The story does not explicitly mention any authorization requirements" is a non-empty string, so it
clears the presence bar cleanly while being vacuous as grounding. This is exactly the shape
`unresolved-decisions-become-explicit-decision-points` already names, and its own Future Validation
section (updated 2026-07-19) confirms this live instance as "one step further than what this
section already anticipated... the presence check alone cannot distinguish 'cites a real, on-topic
fact' from 'cites the fact that nothing was said.'"

**Evidence against.** This explains precisely why Stage 2 never saw the question (it only processes
items already classified `unresolved` — a clean, confirmed, single-cause explanation for *that one
gap*). It does not, by itself, explain the rest of the incident. Even a corrected classification
(this item correctly landing in `open_questions` instead) does not obviously guarantee the eventual
contradiction gets caught — see §2.2 for why.

**Locality.** Single-artifact. The defect is fully detectable by reading `spec.yaml`'s own
`resolved_policies` section in isolation — no comparison against any other file, story, or project
state is needed to see that an absence was cited as if it were a resolving fact.

**Capability dependency.** None. Nothing about whether authentication exists anywhere else in the
project is relevant to noticing this specific misclassification.

**Confidence.** High that this occurred and is real — directly quoted, code-verified. Medium that
it is *sufficient* to explain the whole miss (see §2.2's counter-argument) — it is a necessary piece
of the explanation, not the whole one.

### 2.2 Cross-artifact (same-story) semantic inconsistency

**Evidence for.** `out_of_scope` and the accepted scenario/contract content directly contradict each
other within the same story's own artifact set. `cross-artifact-consistency-audits-prevent-drift`'s
own Future Validation section names this exact case as the first live test of whether its
fail-loud, compare-against-established-state approach generalizes from lexical checks (does a name
match) to semantic ones (does a claim contradict another claim) — and states plainly this is "not a
case of the existing lexical checks failing at a harder task, it's a case of no check existing at
that layer yet." Confirmed directly in code: a project-wide grep for every consumer of the
`out_of_scope` field (`canopy-core`'s struct definition, `spec.rs`'s own generation/test code, and
one `println!` in `commands/spec.rs` that only echoes it to the console at review time) shows it is
never read by Stage 1, Stage 2, Stage 3, or Stage 4 — not "the comparison exists but is weak,"
literally no downstream code path ever consults this field again after it's printed.

Crucially, this framing is fully checkable using *only* `product-010`'s own directory — no other
story, no `domain_registry.yaml`, no `roles.yaml`. The story's own text asserts two things that
cannot both be true at once, independent of anything else in the project.

**Evidence for, mechanism-level**: fixing §2.1's misclassification would *not* by itself have
guaranteed this gets caught. `extract_decisions`'s two LLM calls (`linking_prompt`,
`classification_prompt` in `canopy-llm/src/prompts/decisions.rs`) never reference `out_of_scope` at
all — they operate purely over `spec.open_questions` text and blocked-behavior reasons. Even in the
counterfactual where the authorization item is correctly filed as `unresolved` and becomes a real
Decision Point, nothing in Stage 2's existing prompts shows a resolver `out_of_scope`'s content
side-by-side with the question being resolved. A human could still resolve "should this story
require authentication?" without ever being shown that the same story already excludes the answer
"yes" from scope. This makes §2.2 a structurally independent gap from §2.1, not a downstream
consequence of it.

**Evidence against.** None found that weakens this framing — every check performed (grepping every
`out_of_scope` reference, reading Stage 2's actual prompt construction) corroborates rather than
complicates it.

**Locality.** Cross-artifact, but *within one story* — spans `spec.yaml` and the contract/behaviors
files generated later for the same `story_id`, not across stories or the wider project.

**Capability dependency.** None, for the core contradiction. Whether authentication exists
*anywhere else* in the project is not required to notice that this story's own `out_of_scope` and
its own accepted output disagree — see §4 for why this matters for the boundary question.

**Confidence.** High. This is the most directly observable, least-inferred fact of the whole
incident — independently reached the same way by three separate write-ups (the vertical-slice
report, the open question, and this document's own code check), and confirmed by code (not just
absence of evidence) that no check exists at this layer.

### 2.3 Missing capability assumption (project-wide capability grounding)

**Superseded (2026-07-19):** a confirmed business fact (catalog browsing is public, no
authentication required) later established that this section's classification does not hold for
`product-010` — there is no missing capability being presupposed, because the 401 requirement was
never legitimate in the first place. See
`docs/design/product-010-reassessed-with-confirmed-public-browsing-intent.md` for the full
reasoning. This section's analysis is left as originally written below, since it was sound given
what was known at the time — not rewritten, only superseded.

**Evidence for.** The accepted 401-Unauthorized behavior presupposes an authentication/session
mechanism. That mechanism has zero representation anywhere in this project — not as an entity, not
as a role capability, not as a story, not as an ADR. Structurally the same *shape* of question
entity-with-no-story already asks ("does X exist yet"), just applied to a capability rather than an
entity — which is precisely why the open question document raises it as a serious candidate.

**Evidence against.** This framing requires a registry-like concept the project does not currently
track anywhere (an inventory of "capabilities," as opposed to `domain_registry.yaml`'s inventory of
entities/events) — a heavier structural claim than §2.2 needs. More importantly: **this fact is not
necessary to detect the defect**, only to fully characterize its consequence. Construct the
counterfactual directly: if this same project already had a complete, working authentication
capability (a `Session` entity, a login story, a real ADR), `product-010`'s own two fields would
*still* contradict each other — `out_of_scope` would still say "not this story's job" while the
accepted scenarios and contract would still presuppose it directly. The internal contradiction is
independent of whether the presupposed capability happens to exist elsewhere. What the missing-
capability fact adds is severity, not detectability: it's why this contradiction is actively
dangerous (a 401 check with nothing behind it) rather than merely untidy (a redundant contract
against an already-real mechanism).

**Locality.** Genuinely project-wide — by definition requires checking `domain_registry.yaml`,
`roles.yaml`, and every other story to establish "this doesn't exist anywhere."

**Capability dependency.** Yes, definitionally — this category *is* the capability-dependency axis,
not merely related to it.

**Confidence.** High that the fact itself is true (no authentication capability exists in this
project, confirmed by inspection). Medium-low that this is the *primary* characterization of the
failure a check would need to key on, given the counterfactual above — it is real, relevant
context, likely necessary for a complete account of *why this specific case turned out badly*, but
not the load-bearing fact for detecting that something is wrong in the first place.

### 2.4 Missing dependency

**Evidence for.** One could frame this as "Customer browsing should `depend_on` an authentication
capability/story that was never linked." Structurally similar to the mechanical dependency rule
work from the Contract Composition investigation, and to Stage 4's own cross-contract dependency
review.

**Evidence against.** The story's own `out_of_scope` field explicitly declines to require this
capability — it isn't an omitted link to a prerequisite the story *needs*, it's a stated position
that the story does *not* need it, contradicted by its own accepted output. Framing this as "missing
dependency" mischaracterizes a self-contradiction as an omission. It's also not what Stage 4's
dependency review is built to catch: that mechanism compares contracts/behaviors that were actually
generated against each other, and has no way to represent "this behavior presupposes a capability
that was never generated as any contract at all" as a dependency edge to check.

**Locality.** Would be project-wide (same as §2.3), since establishing "was there a story to depend
on" requires checking the rest of the project.

**Capability dependency.** Yes, if pursued.

**Confidence.** Low. Weaker fit than §2.2 or §2.3 — it recasts a same-story contradiction as an
inter-story omission, which the story's own text argues against.

### 2.5 Incorrect/incomplete checklist coverage

**Evidence for.** Stage 0 is built entirely on `exhaustive-enumeration-over-holistic-review` — a
mechanically-computed, per-item checklist drives what gets checked. If that checklist simply lacked
an item for "does `out_of_scope` contradict the generated scenario set," this looks at first glance
like the same failure shape as that principle's own documented history (a bounded set walked
incompletely).

**Evidence against.** That principle's evidence base is specifically about a *known, bounded,
already-enumerated* set not being fully walked (9 known constraints, only 4 checked; a fixed list
of policy areas, only some resolved with grounding). `out_of_scope`-vs-scenario-content comparison
was never on Stage 0's enumerated checklist at all — there is no existing bounded-set walk this
falls outside of; the checklist axis for this comparison doesn't exist, full stop. This is a
different, arguably prior gap to what `exhaustive-enumeration-over-holistic-review` diagnoses:
under-enumeration of an existing list versus absence of a list dimension entirely. Framing it as
"incorrect checklist coverage" risks collapsing two distinct principles (enumeration-completeness
vs. cross-artifact-consistency) into one, which is exactly the kind of question-blurring this
project's own house style warns against.

**Locality.** Would be single-artifact/single-stage (Stage 0's own generation step), same locality
as §2.2, if it were the right frame.

**Capability dependency.** None.

**Confidence.** Low-medium. Real overlap with §2.2 (both point at Stage 0 lacking something), but a
less precise fit — better described as "no check exists at this layer" (§2.2) than "an existing
check under-walked its list" (this principle's established shape).

### 2.6 Something else — the citation-quality loophole, named precisely

Worth separating from §2.1 as its own, more mechanism-specific observation rather than folding it
back in as a restatement: the *specific* reason the existing evidence-citation enforcement
(`3241e8f`, per `unresolved-decisions-become-explicit-decision-points`) didn't catch this is that it
was built and validated against a different failure shape — fabricating a specific answer (a role
name, a retention period) with **zero** citation — and this case supplies a citation that is
**present but vacuous** (it cites the absence of a fact, not a fact). The enforcement mechanism's
own prior validation (the reproducibility comparison showing 5/6 → 1-2/6 fabricated-with-no-citation
answers) never tested this adjacent case, because it hadn't occurred yet. This is a precise,
narrower statement than "unresolved policy decision" in general: it names exactly which layer of an
*already-existing* enforcement mechanism has an untested gap, distinct from either "no enforcement
exists" (false — enforcement exists and mostly works) or "the enforcement is entirely broken" (also
false — it still blocks true zero-citation fabrication).

**Confidence.** High that this is an accurate, narrow description of the mechanical root cause
underneath §2.1 — not a new category so much as the most precise possible statement of §2.1's
actual failure point.

## 3. Summary Table

| Category | For | Against | Locality | Capability-dependent | Confidence |
|---|---|---|---|---|---|
| Unresolved policy, misclassified | Code-confirmed presence-only check; matches a named principle exactly | Explains the Stage 2 miss, not the whole incident | Single artifact | No | High (occurred) / Medium (sufficient) |
| Cross-artifact same-story inconsistency | Code-confirmed: `out_of_scope` read nowhere downstream; independent of §2.1's fix | None found | Cross-artifact, same story | No | High |
| Missing capability assumption | Real, confirmed absence project-wide | Not needed to detect the defect, only to size its consequence | Project-wide | Yes (by definition) | High (fact) / Medium-low (as primary explanation) |
| Missing dependency | Superficially similar to existing dependency-review shape | Story's own text declines the dependency; miscasts contradiction as omission | Project-wide | Yes | Low |
| Incomplete checklist coverage | Stage 0 is enumeration-built | No enumerated item ever existed for this axis — not the same failure shape as the principle's evidence base | Single artifact/stage | No | Low-medium |
| Citation-quality loophole (mechanism-precise) | Names the exact untested edge of an existing, otherwise-working enforcement | N/A — a sharpening, not a competing claim | Single artifact | No | High |

## 4. Backlog Incompleteness or Story Unreadiness?

**Story Unreadiness — with a specific, evidence-backed reason for placing it there, not by
elimination.**

The load-bearing property is §2.2: `product-010`'s own artifacts assert two things that cannot both
be true, and this is fully checkable using only that story's own directory. Construct the
counterfactual explicitly, since it's the cleanest test of which side of the boundary this belongs
on: **if a complete, working authentication capability already existed elsewhere in this project**,
would this story still be defective? Yes — `out_of_scope` would still claim authentication is not
this story's concern while the accepted contract still behaves as though it is. The defect does not
depend on the answer to "does this capability exist anywhere else" — that fact (§2.3) changes how
dangerous the defect is, not whether it exists. A property checkable from one story's own artifacts,
independent of everything else in the project, is Story Readiness's defining shape, not Backlog
Evolution's — Backlog Evolution's own defining question ("does a story exist for concept X yet") is
answered entirely by project-wide state and says nothing about whether any single existing story is
internally sound.

This sharpens, rather than repeats, the open question document's own tentative conclusion. That
document left this genuinely undecided ("the miss doesn't cleanly sort into either bucket... full
resolution requires knowing whether an authentication capability exists anywhere else"). The
code-level check performed here — that Stage 2's actual prompts never reference `out_of_scope`, and
that no downstream stage reads it at all — plus the explicit counterfactual above, both point the
same direction: full *resolution* of the underlying business question (should this story require
auth, and if so what handles it) does need project-wide capability knowledge, but *detection* of the
defect — that this story cannot currently ship as internally consistent — does not. Since the
question being asked here is what kind of failure this is, and detectability is the more direct
test of that, this document's answer is more decided than the open question's own hedge: **Story
Readiness**, with Backlog Evolution's capability-axis extension (§2.3) as real, relevant, but
secondary context about consequence rather than the defining property of the failure itself.

**What would change this conclusion.** A second real instance where the *same* class of
contradiction (a story's own scope-exclusion field disagreeing with its own accepted output) turns
out to be undetectable without first knowing the project's wider capability state — i.e., a case
where the internal-consistency framing genuinely fails to reach the defect and only a project-wide
capability check would. No such case has been observed; this is a conclusion from one story,
reasoned through a counterfactual, not from two independent data points. That limitation is stated
plainly, not glossed over — matching this project's own standing discipline (see the Human-Insight
Inventory rerun) of not generalizing past a single-story sample without saying so.

## 5. What This Document Does Not Do

Per explicit instruction: no mechanism is proposed, no check is designed, no Story Readiness
framework is sketched. `docs/open-questions/story-readiness-vs-backlog-evolution.md`'s own "What We
Know" items 1–3 remain the correct inventory of *what a signal would have needed to detect*; this
document's contribution is categorizing *what kind of failure* those three items jointly describe,
and answering, with reasoning rather than a hedge, which side of the backlog-incompleteness /
story-unreadiness boundary it falls on. That open question's exit criteria (a second real instance,
or an authorized design pass) are unchanged and still govern what happens next.
