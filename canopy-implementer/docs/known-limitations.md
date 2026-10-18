# Known limitations of the Implementer on this benchmark

One entry per limit that was measured and not fixed. Each says what was tried, so the next session does not repeat it, and how a
result that hits the limit should be read. The numbers are in `prompt-and-plan-changelog.md`.

Model and setup for every entry: Qwen3-14B Q4_K_M behind llama-server, plain Java 21 / Maven / JUnit 5, the `lean` system prompt, the
one-answer protocol, three attempts per step, harness test as the only judge of attempts.

## 1. The boundary of an "expires after N" comparison follows the model's idiom, not the plan (recorded 2026-10-18)

- **What happens:** for a rule like "a ticket is valid for 90 minutes" the model writes `at.isAfter(issuedAt.plusMinutes(90))`, which keeps
  exactly 90 minutes valid. When the intended behaviour is the opposite (exactly 90 minutes is expired), it writes the same comparison anyway.
  The same plan text passes 9 of 10 against a hidden test that wants the inclusive boundary (T4) and 0 of 10 against one that wants the
  exclusive boundary (X1).
- **Tried against the exclusive boundary, all n=10, all 0/10 (X1 to X6, about 100 attempts):** the rule left open (X1); the boundary stated
  in the rule sentence (X2); the boundary as a case in the step's test sentence (X3, which moves the failure from the hidden test to the
  step's own gate); a diagnostic fact about reading "expected exception not thrown" (X4); "the tests are the specification" (X5); and the
  two-call test-first protocol with the test shown read-only (`--protocol test-first`, X6). In 100 production answers the comparison changed
  in 1 run, and that run then failed on a wrong test of its own that fixed tests cannot repair.
- **Why the gate does not help:** the model writes the test and the code in one answer from the same case and idiom, and the retry starts from
  its previous answer: 44% of retries over this session's batches are identical answers (0 of 96 recovered), and 9% of all retries recover.
- **How to read a result that hits it:** a failure of this kind is not a plan-detail failure and a better plan does not recover it. A Planner
  scored on whether the Implementer passes will lose on such a step whatever it writes; mark the story or step as hitting this limit rather
  than changing the Planner, and do not count it against plan quality.
- **What has not been tried:** sampling several production answers against a fixed test and keeping one that passes (best of N), see
  `docs/open-questions/search-strategy-best-of-n.md`; a different or larger model; fine-tuning. Nothing here shows the limit for other
  models, other rules (the same shape with a different default idiom) or other languages.
- **Where the evidence is:** the change log entries T4, X1 to X6, "Repair behaviour, measured over this session's batches", and
  `docs/repair_behaviour.py`.
