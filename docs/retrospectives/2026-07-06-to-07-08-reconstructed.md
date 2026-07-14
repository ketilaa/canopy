# 2026-07-06 to 2026-07-08 — Paying down two god modules

**source: reconstructed** — written 2026-07-14 from commit history only (8 commits, 5 of which are
automated `wip: auto-install checkpoint` placeholders confirmed via `git show --stat` to be
mechanical snapshots with no authored content — excluded from this account). No first-hand session
record exists for this period. Inferred entirely from the 3 real commits' messages and diffs —
nothing here is invented beyond what those artifacts support.

---

# What Changed

Through the preceding weeks, `canopy-llm/src/lib.rs` and `canopy-cli/src/main.rs` had grown into two
large single files — the automated checkpoint commits leading up to this period show both files
still gaining hundreds of lines each as feature work continued. On 2026-07-08, `2872c18` split both
apart: `canopy-llm/src/lib.rs` (roughly 3,741 lines before) became a thin module-declaration file,
with prompt builders, skills, and repair logic moved into their own modules
(`client.rs`, `repair.rs`, `tech.rs`, `skills/{mod,tech_stack,architecture,build_system,testing}.rs`,
`prompts/{mod,intent,spec,plan,step,fix,dependencies,scaffold,summary}.rs`); `canopy-cli/src/main.rs`
(roughly 2,800 lines before) shrank to about 40, with command handling, the fix loop, and REPL
plumbing split into dedicated files. Business logic that had been sitting in the wrong crate layer —
tech-family classification, ADR merging, domain-entity/event/role types — moved down into
`canopy-core`. Separately, `bf01dc8` added a Rust language parser to the Roots repository-
intelligence tool, mirroring the existing Java/Kotlin/TypeScript extractors.

# What We Learned

The refactor commit's own message diagnoses the problem precisely, not vaguely: prompt builders
entangled with skill definitions, planning logic mixed with step-execution logic, repeated
dialoguer/subprocess/storage boilerplate, and domain types living in the wrong layer. This wasn't
"the files got long" — it was specific coupling and misplaced responsibility that had accumulated
across many feature commits, each individually reasonable, without anyone stopping to reorganize
in between.

# What Surprised Us

The refactor was pure reorganization — 41 new files, over 8,000 line insertions, and by the commit's
own account, zero net new logic. It also folded in several smaller accumulated behavioral
refinements (TDD Red/Green parity fixes, prompt-bloat trims, a fix-loop no-op detector, stale-
artifact cleanup) that the commit message says were "consolidated rather than replayed stepwise" —
meaning real incremental work had been happening between commits without being captured as separate,
reviewable units. That's a process observation as much as a technical one: valuable intermediate
decisions can go unrecorded if refactoring and feature work aren't kept in visibly separate commits.

# What We Believe Now

*(Reconstructed inference from the shape of this refactor, not a verbatim belief statement.)* Let a
module grow while its natural boundaries are still unclear, rather than prematurely splitting it —
but once those boundaries become visible from real usage (here, from actually driving `implement`
runs against real projects), stop adding to the monolith and consolidate deliberately, as its own
clearly-labeled commit rather than mixed into ongoing feature work.

# Possible Next Steps

*(Inferred from what this period's evidence leaves open, not stated directly.)* The commit message's
own admission — that several days' worth of real refinements were folded into one commit rather than
captured incrementally — is itself evidence for a discipline the project appears to formalize only
later (see the Commit Discipline section that exists in CLAUDE.md by the retrospective-covered
period beginning 2026-07-09): commit at natural checkpoints as work happens, rather than
accumulating and reconstructing afterward.
