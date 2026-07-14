# Theme Analysis — Canopy's Intellectual History

Derived bottom-up from the artifacts in `docs/principles/` and `docs/blog-drafts/` after they were
written, not designed in advance. Method: list every principle and blog post produced from the full
commit history (2026-06-19 through 2026-07-14), then group by what each one is *actually about*
rather than by a pre-existing category, and name each resulting cluster after the theme it turned
out to share.

---

## The full artifact set going into this clustering

**Principles (9):**
1. Exhaustive enumeration outperforms holistic review for coverage-critical tasks
2. Compute facts mechanically; let the model act on facts
3. Deterministic audits are safe; silently rewriting model output is not
4. Unresolved business decisions should become explicit decision points
5. Cross-artifact consistency audits prevent semantic drift
6. Coverage should be generated deliberately, not discovered accidentally
7. Reserve the model for genuine ambiguity; go deterministic once a mapping is enumerable
8. Structure should emerge from described behavior, not be solicited upfront
9. Freeze an established specification so generation cannot silently redefine correctness

**Blog posts (5):**
A. Why We Replaced Holistic Review with Enumeration
B. Every Example Noun Is a Candidate Answer
C. Policy Discovery vs. Policy Invention
D. We Deleted the LLM Call and Replaced It With a Static Template
E. The Same Fix, Rediscovered Two Weeks Apart

---

## Cluster 1 — Enumeration Over Holistic Review

**Members:** principles 1, 6; blog A.

Every member of this cluster is about the same specific failure and fix: a task defined over a
bounded, knowable set of items (constraints, scenarios, policy questions) fails when framed as
"review/generate holistically," and succeeds when framed as "walk this fixed list, answer for each
item." This is the single most concretely-measured cluster — it has a directly observed before/after
number (4 of 9 constraint gaps found → 9 of 9) and reproduced independently across at least four
parts of the pipeline once looked for deliberately.

## Cluster 2 — Compute, Don't Ask

**Members:** principles 2, 3, 5, 7; blogs D, E.

The largest and most recurring cluster by a wide margin, and the one this reconstruction pass most
changed our understanding of. Every member shares one shape: something the system already knows, or
could determine mechanically, was instead being asked of the model — and the fix was always to
compute the answer and hand it over, never to ask more clearly. Principle 7 (reserve the model for
genuine ambiguity) is the broadest, earliest-evidenced member — it traces back to day 0's scaffold-
generation rollback, three weeks before the more specific instances (Entity Continuity, Event
Continuity, domain-event-ADR detection) were built. Blog E documents directly that this exact shape
was rediscovered independently at least three times, roughly two weeks apart, before being named as
one standing rule.

This cluster is also where the audit-vs-compensation distinction (principle 3) lives: computing a
fact and checking it is encouraged; silently rewriting what the model produced is not — a
distinction that had to be sharpened after review found code doing the latter while believing it was
doing the former (see `docs/blog-drafts/policy-discovery-vs-policy-invention.md`'s evidence trail).

## Cluster 3 — Protecting What's Already Decided

**Members:** principles 4, 9; blog C.

A distinct cluster from Cluster 2, though closely related. Cluster 2 is about *new* judgments — is
this thing the system is about to determine actually a fact it can compute? Cluster 3 is about
*already-settled* things — a resolved business decision, an accepted test specification — being
protected from silent re-interpretation or overwrite by a later automated step. The clearest
evidence is temporal: TDD test-file freezing (principle 9) was built weeks before evidence-grounded
policy discovery (principle 4, via blog C) existed, yet both solve the same underlying problem in
their own domain — stopping a later step from quietly redefining something a prior step already
settled.

## Cluster 4 — Emergent Design

**Members:** principle 8.

Currently a cluster of one, but a foundational one — this is the earliest-dated finding in the whole
reconstruction (day 0, 2026-06-19) and it's also the project's own stated design philosophy
(CLAUDE.md's "Core Design Insight: Everything emerges. Nothing is decided upfront" table). Worth
watching for further principles to join this cluster as more of the project's history gets reviewed,
since the reconstructed retrospectives show this same shift (upfront → emergent) recurring across at
least three different kinds of information (architecture, roles/boundaries, domain vocabulary) in
its first week alone.

## Cluster 5 — Prompt Crafting Idiosyncrasies of Small Models

**Members:** blog B, plus one un-principled finding (see below).

The narrowest cluster, and the only one made up of tactical, wording/placement-level findings rather
than architectural ones: an example noun in a prompt functions as a candidate answer, not a neutral
illustration (blog B); and — found during this reconstruction but not yet its own principle
document — a rule's physical position in a prompt measurably affected whether the model complied
with it, independent of the rule's own wording (`docs/retrospectives/2026-07-02-to-07-03-
reconstructed.md`, commit `3da65d8`). Both findings are about how the *literal construction* of a
prompt (word choice, physical placement) affects a small model's behavior in ways not obviously
predictable from the instruction's content alone — genuinely distinct from the larger architectural
question of *what* a prompt should ask the model to do at all (Clusters 1–4).

---

## What this clustering surfaced that individual artifacts didn't

The single most useful result of doing the clustering *after* writing the artifacts, rather than
before: Cluster 2 turned out to be far larger and more temporally spread out than any single
principle document suggested on its own. Reading principles 2, 3, 5, and 7 side by side, alongside
blog E, makes visible a fact none of them state individually — this exact fix shape was
independently rediscovered at least three times across the project's history before anyone
recognized it as one thing. That recognition is itself the reason blog E exists; it wouldn't have
been written if the artifacts had been generated one at a time without a clustering pass afterward.
