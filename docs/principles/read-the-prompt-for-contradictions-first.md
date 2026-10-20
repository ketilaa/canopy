---
title: "A Rule That Contradicts Another Injected Instruction Costs More Than a Missing Rule"
status: draft

confidence: medium

maturity: emerging

themes:
  - prompt-design
  - implementer
  - debugging-method

evidence_strength: medium

source_artifacts:
  - "commit 502d7ff — leave TOOL_READ_RULES out of a retry that shows the previous files (C1)"
  - "docs/reports/order-plain-java.md — the retry contradiction, loops 17/20 to 2/12"
  - "canopy-implementer/docs/prompt-and-plan-changelog.md — C1, B1-B6 (a contradicting test sentence inside a plan), the call-2 task text of the tickets experiment"

related_principles:
  - prompt-house-style-legacy
  - compute-facts-mechanically
cluster: "Subtract Before Adding"
---

# Principle

When a model keeps failing in the same way, read the whole prompt it received (system rules, task text, retry text,
plan text) for two instructions that cannot both be followed, before adding any new instruction. Where the model
obeys one of two contradicting instructions, the failure is the contradiction, and the fix is to remove one of them.
A new rule added beside the pair is a third voice in the argument.

The unit of inspection is the prompt as assembled for that call, not the prompt template. The two halves of a
contradiction often live in different places and are injected on different paths (a system rule that is always
sent, a retry message that is sometimes appended), so reading either one alone shows nothing wrong.

# Problem That Revealed It

The tool-mode system rules told the model "ALWAYS use glob and read_file to find and read the files you need".
A retry after a compile or test failure told it that its previous answer was undone and "NEVER look for the files
you submitted". In the single-step order runs the model obeyed the system rule: 17 of 20 retry failures were
tool-budget or repeated-call loops, and no first-attempt failure recovered. It was the largest single bug found in
the series, in a project whose prompts had been built to a detailed style guide and read as individually correct.

Leaving the two reading rules out of a retry that shows the previous files cut loops to 2 of 12 retry failures and
moved the single step from 0/10 to 4/10 scored. The change removed text. Each rule was reasonable alone; the
contradiction only existed in the assembled retry prompt.

# Evidence

- **Retry (C1, `502d7ff`):** loops 17/20 to 2/12 retry failures; 2 of 8 first-attempt failures recovered where none
  had; single step 0/10 to 4/10. Part of the 4/10 is sampling noise (the first-attempt prompt was unchanged yet its
  failure mix moved); the retry-loop effect is the clear part.
- **A plan that contradicted itself (shipping, B1-B6):** the plan's test sentence "free shipping at and just below
  100.00" read as "free at 100.00 and free just below 100.00", and the model followed it over the rule sentence and
  over an added value, in every run. Adding values did not help (hidden pass 3 to 5 of 10). Rewording the sentence
  removed the failure class, and with values for every named case the plan reached 10/10 (B5). The cause was found
  by reading the plan for contradictions, as the project's own habit says to do first.
- **Sentences the model must ignore (tickets, call 2):** the task text still said "Add TicketTest ..." in a call
  that also said the tests exist. Recorded as a known contradiction that could not be removed without parsing the
  instruction, with a watch for submissions that include tests. This one is a risk, not yet a measured loss.

# Counter-Evidence

- Three data points, one model (the 14B local model), plain Java, order and shipping stories. The retry case is the
  only one with a before/after on a pass rate, and it is partly confounded by first-attempt noise.
- Not every repeated failure is a contradiction. The same series found failures that were a missing value (the
  model invents expected values the plan never states) and a harness gap (one answer containing both code and a
  test that disagree). Fixing the contradiction alone left shipping at 5 of 10 until the missing values were also
  supplied (B1 to B6): the two defects added up, they did not substitute.
- A larger model might resolve a contradiction by judgment instead of by obeying the louder instruction. Untested.

# Applicability

- Any prompt assembled from parts added on different paths: system rules plus a retry message, a fix-loop message,
  a plan step plus a rendered context, a skill plus a task.
- Diagnosis order: print the assembled prompt for the failing call, list every imperative in it (ALWAYS, NEVER,
  "must", "do not"), and check each pair for a case where both cannot hold.
- Before adding a rule to fix a recurring failure: check whether the new rule would sit beside an existing one that
  it half-contradicts. If so, edit the existing one.

# Confidence Assessment

Medium. The mechanism is visible in the prompt text and the retry fix has a large measured effect, but the evidence
is one model, one language and two stories, and the pass-rate gain is partly noise. The principle is cheap to apply
(reading costs nothing), so acting on it before it is confirmed is low risk.

# Future Validation

- A check of the assembled prompts of the next batch for imperative pairs, done before the first change.
- A paired run (n=10 or more) that adds one deliberate contradiction to an otherwise passing prompt and measures
  the drop, to see whether the effect is as large when planted as when found.
- Another model (an Anthropic one) on the old retry prompt, to test whether the contradiction costs only small
  models.
