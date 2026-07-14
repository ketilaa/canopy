# 2026-06-29 to 2026-07-01 — Tech-stack skills and the frozen test file

**source: reconstructed** — written 2026-07-14 from commit history only (8 commits across these
three days). No first-hand session record exists for this period. Inferred entirely from commit
messages and diffs — nothing here is invented beyond what those artifacts support.

---

# What Changed

Before this period, implementation generation worked as one monolithic LLM call per delivery
intent: a single prompt produced every file, guided only by a generic instruction — "use the
technology stack from the component architecture... follow idiomatic conventions for the chosen
technology." No stack-specific rules, no expected file paths, no naming conventions, and no
test-driven discipline; tests were "included in a separate file when the plan has test tasks," with
no compile gate or pass/fail loop at all.

`e0aedf9` replaced this with per-service generation (one LLM call per service, merged and
layer-ordered afterward) and introduced tech-stack "skills" — authoritative, stack-specific rule
blocks for exact file paths, naming conventions, and forbidden patterns, dispatched by detected
technology. `8cfabf7` then added a Red/Green TDD loop: a test stub is generated and compile-checked
first (Red), and only once that's confirmed does implementation generation proceed (Green) — with
the test file itself frozen (`skip_files`) so the fix loop cannot edit it while making the
implementation pass.

# What We Learned

The generic "follow idiomatic conventions" instruction produced real, structural failures, not just
style inconsistencies. Java package paths were computed synthetically rather than detected from what
had actually been scaffolded, and a stale-cleanup step was found to be deleting real generated
files as a side effect (`e0aedf9`'s own commit message states this plainly). Validation annotations
were being applied to types they don't support (`@Positive`/`@Min`/`@Max` on non-numeric fields,
`8cfabf7`) — evidence the generic instruction gave the model no grounding in what's actually valid
for a given stack.

# What Surprised Us

The test-freezing detail is the sharper finding here. Once a TDD loop exists, a new risk appears
that a single-shot generator never had: the fix loop, faced with a failing test, could "fix" the
test to match a broken implementation instead of fixing the implementation to match the test. The
project's response wasn't a prompt instruction asking the model nicely not to do this — it was a
structural guarantee (`skip_files` protecting the test during Green phase) that makes the shortcut
unavailable regardless of what the model would prefer to do. This is a materially different kind of
fix than the tech-stack skills themselves (which are still prompt content, just more specific
prompt content) — it's the first clear instance in the project's history of protecting an already-
established artifact from being silently reinterpreted by a later step.

# What We Believe Now

*(Reconstructed inference from what `e0aedf9` and `8cfabf7` together establish, not a verbatim
belief statement.)* Tech-specific, narrow rule blocks reliably beat one generic "use idiomatic
conventions" instruction. And once a test is established as the specification for a piece of
behavior, that spec needs to be protected — structurally, not just by asking — from being redefined
by whatever comes after it.

# Possible Next Steps

*(Inferred from what the evidence suggests remained open, not stated directly.)* At this point,
skills exist as functions/constants inside a single large file (`canopy-llm/src/lib.rs`), not yet
as a dedicated module — the later `skills/` directory split (visible in subsequent history) isn't
evidenced yet in this window. The Red-phase compile-only check also doesn't yet address a test that
compiles but fails at runtime for a non-domain reason — a gap the project's own later documentation
(CLAUDE.md's TDD section) explicitly calls out as something addressed further downstream, not in
this period.
