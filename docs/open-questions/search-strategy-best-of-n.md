---
title: Search at the step level (best of N) against the model's single-sample limits
status: open
origin: the exact-boundary experiments X1 to X6 (2026-10-18), canopy-implementer/docs/known-limitations.md
date_discovered: 2026-10-18
related_principles: []
related_narratives: []
related_reports: []
related_design_docs: [implementer-first-course.md]
---

# Question

When a step has a deterministic verifier (the build and a test the model cannot change), is it better to draw several
independent answers and keep the first that passes than to retry the failed one with feedback? And does that recover failures that
no prompt, plan or retry text has recovered?

# Why It Matters

The exact-boundary limit (known-limitations.md, entry 1) resisted every prompt-side and protocol-side lever: about 100 attempts, one run
in which the comparison changed. In that run (X6 run 08) the model did write the right comparison, so it is reachable for this model at a
low rate. Retries with feedback repeat the failed answer (44% identical, 9% recover overall), and a restart from zero was no better in
the real harness (R3). Both draw from the same narrow distribution, one answer at a time, with the previous answer in context.
Best of N draws N independent answers against a fixed test; it costs N calls per failing step and needs no trust in the model.

# Evidence So Far

- R3: `--retry restart` (a fresh draw per attempt) equals `feedback` on four stories, so extra draws at three attempts per step did not help
  on plans that fail systematically.
- X6 run 08 produced the intended comparison in 3 of 3 production answers of that run and none in the other 9 runs (27 of 27); so the
  per-run rate is about 1 in 10, and runs differ more than attempts within a run, which says draws inside one run are not independent
  (the same prompt state), while draws across runs are.
- No best-of-N run exists. The test-first protocol (`--protocol test-first`) provides the fixed test that makes the verifier trustworthy.

# Options

1. Best of N with `--protocol test-first`: N production samples per step (temperature as is), the first that builds and passes wins. Prediction
   to write before running: with N=8, X3 passes at least 4 of 10; falsified at 1 or fewer.
2. Best of N without the protocol: samples of the whole answer; the model's own test decides, so a wrong test can win. Cheaper to build,
   weaker verifier.
3. Beam over plan variants (the Planner's job, course step 3), not over answers.

# What We Don't Know

- Whether independent draws change the comparison more often than 1 in 10 per run, and what N is worth its cost.
- Whether it generalises beyond one rule and one model.
- How it should be counted in Planner scoring (cost per step is part of the result).

# Exit Criteria

Resolved when a best-of-N batch has been measured on the exact-boundary task and one other story where first attempts fail, with cost per
passed step reported next to the pass rate.
