# Suggested Knowledge Taxonomy

Emerged from `docs/theme-analysis.md`'s bottom-up clustering, not designed ahead of it. Proposed as
a light tagging convention on top of the existing `docs/{principles,blog-drafts}/` directories — not
a directory reshuffle. Files stay where they are; each gains one additional front-matter field.

## The taxonomy

| Cluster | Name | Current members |
|---|---|---|
| 1 | **Enumeration Over Holistic Review** | principles: `exhaustive-enumeration-over-holistic-review`, `coverage-should-be-generated-not-discovered`; blog: `why-we-replaced-holistic-review-with-enumeration` |
| 2 | **Compute, Don't Ask** | principles: `compute-facts-mechanically`, `deterministic-audits-vs-compensation`, `cross-artifact-consistency-audits-prevent-drift`, `reserve-the-model-for-genuine-ambiguity`; blogs: `we-deleted-the-llm-call-and-replaced-it-with-a-template`, `the-same-fix-rediscovered-two-weeks-apart` |
| 3 | **Protecting What's Already Decided** | principles: `unresolved-decisions-become-explicit-decision-points`, `freeze-the-established-spec`; blog: `policy-discovery-vs-policy-invention` |
| 4 | **Emergent Design** | principle: `structure-emerges-from-behavior` |
| 5 | **Prompt Crafting Idiosyncrasies of Small Models** | blog: `every-example-noun-is-a-candidate-answer` |

## How to apply it going forward

Add a `cluster:` field to every principle and blog-draft's front matter, using one of the five names
above verbatim (or a new name, if a future artifact doesn't fit any existing cluster — see below).
This is additive to the existing `themes:`/`topics:` fields, which stay as free-text keywords; the
taxonomy field is the canonical, closed-set categorization meant to stay stable over time.

```yaml
cluster: "Compute, Don't Ask"
```

This wasn't retrofitted onto the 14 existing artifacts as part of this pass — doing so is a
mechanical follow-up, not a judgment call, and is listed as a next step below rather than done
speculatively here.

## Where the taxonomy is thin, and what that means

- **Cluster 4 (Emergent Design)** has exactly one member despite being the earliest-dated finding
  in the whole history and the project's own stated design philosophy. This is very likely a gap in
  *what's been written up*, not a gap in *what actually happened* — the reconstructed retrospectives
  for 2026-06-19 through 2026-06-25 describe the same emergent-vs-upfront shift recurring at least
  three times (architecture, then roles/boundaries, then domain vocabulary) before it was reviewed
  for this pass. A dedicated review of the 2026-07-09 onward period specifically for further
  instances of this cluster (the behavior-first pipeline's own "everything emerges" design is a
  direct continuation of it) would likely populate this cluster further.
- **Cluster 5 (Prompt Crafting Idiosyncrasies)** has one principle-shaped finding — the rule-
  proximity-affects-compliance observation from `docs/retrospectives/2026-07-02-to-07-03-
  reconstructed.md` — that was found during this reconstruction but never turned into its own
  principle document. Worth doing as a follow-up: it has real evidence (a specific commit moving a
  rule's physical position and stating that as the fix, not a rewording) and would strengthen this
  cluster from one member to two.

## Why this taxonomy, and not a different one

Two other groupings were considered and rejected specifically because they were designed *before*
reviewing the evidence, which the reconstruction task explicitly warned against:

- **By pipeline stage** (Stage 0, Stage 1, planning, scaffolding, implementation) — rejected because
  it would scatter genuinely identical fixes across different categories. The "Compute, Don't Ask"
  cluster's members span scaffolding (day 0), planning (weeks later), and specification generation
  (weeks after that) — grouping by pipeline stage would hide the exact cross-era rediscovery pattern
  that `the-same-fix-rediscovered-two-weeks-apart` documents as its whole point.
- **By artifact type** (principle vs. blog vs. retrospective) — this is already how the directories
  are organized; it answers "what kind of document is this," not "what is this document actually
  about," which is the more useful axis for someone trying to find everything the project has
  learned about one recurring problem.

## Next steps

1. Retrofit the `cluster:` field onto all 9 existing principle documents and 5 existing blog drafts,
   using the table above.
2. Write up the rule-proximity finding as its own principle document, strengthening Cluster 5.
3. Review the 2026-07-09 through 2026-07-14 period (already covered by real retrospectives) for
   further Cluster 4 (Emergent Design) instances — the behavior-first pipeline's Decision Points
   mechanism and its "let a behavior emerge from a scenario rather than be specified upfront" design
   look like strong candidates on inspection, but weren't formally clustered as part of this pass
   since this pass's research agents focused on the pre-retrospective period specifically.
4. Re-run this clustering exercise after the next batch of principles/blog-posts is written, rather
   than treating this taxonomy as final — per the same "derive, don't design upfront" method this
   version was produced with.
