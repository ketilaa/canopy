# 2026-07-02 to 2026-07-03 — Prompt guidance for humans, code enforcement for machines

**source: reconstructed** — written 2026-07-14 from commit history only (66 commits across the two
heaviest single days in the project's history to that point). No first-hand session record exists
for this period. Given the volume, this reconstruction prioritizes the recurring pattern over a
commit-by-commit account — nothing here is invented beyond what the cited commits support.

---

# What Changed

At the start of this period, the model's YAML output was treated as basically trustworthy — plans
were parsed close to directly, with the default fix for a bad response being a clearer prompt
(`c4e8788`: "restructure plan prompt rules into four labelled sections").

The `depends_on` field (a list of file paths a step depends on) went through the same failure four
times in one day: the model encoded it as a bare string instead of a real list (`8822716`, fixed by
tolerating the string), then as a quoted string-array (`8317acc`, fixed by tolerating that shape
too), and that very fix then over-matched and broke a *different*, already-working case — bracket
type annotations elsewhere in the same YAML (`5cc34fb`). A parallel field in a different prompt
(dependency proposals) needed the identical class of fix a day later: list-item indentation broken
(`8a2f623`), then a degenerate empty-bracket value causing an outright panic (`e9f6f2d`).

By the end of the period, prompt formatting itself had become a deliberate, systematized fix
target — not just wording, but visual structure: `b3c1ff0` and `5c21660` rolled out consistent
`##`/`###` section headers across every skill and plan prompt, and `3da65d8` physically relocated a
testing rule to sit immediately before the output template specifically because its distance from
the point of generation was itself causing it to be ignored.

# What We Learned

List/array-shaped YAML was the one consistent failure category across the whole period — scalars
were reliable, sequences were not, in every one of at least three unrelated prompts. A parallel
pattern showed up in dependency proposals specifically: the model invented content not grounded in
already-decided context (a wrong database driver, banned packages, already-installed packages
proposed again) — a distinct "under-constrained" failure alongside the YAML-shape one.

Neither failure class was fully solved by a prompt-only fix. `9281a2a` shows this most directly:
frontend step ordering was "fixed" by prompt wording that same morning, and the actual ordering
logic still needed a code-level fix an hour later. The `depends_on` chain shows the same lesson from
the opposite direction — each purely prompt/parsing patch closed one case and opened another,
because the underlying issue (the model can't reliably render list-shaped YAML) doesn't go away by
asking more clearly; it needs tolerant, idempotent parsing on the code side as well.

# What Surprised Us

That prompt *placement*, not just prompt *content*, measurably affected compliance. A rule that was
present, correctly worded, and unambiguous still got ignored when it sat far from the content it
governed — moving the identical rule closer to the point of generation (`3da65d8`) is presented as
the actual fix, not a rewrite of the rule's wording. This is the first clear evidence in the
project's history for what later becomes an explicit house-style rule about proximity.

# What We Believe Now

*(Reconstructed inference, closely paraphrasing `9281a2a`'s own commit message, not invented.)*
Prompt guidance is for communicating intent to a model that has some discretion; code enforcement
is for anything the system actually needs to be true. Ordering, test presence, scaffold exclusion,
package filtering, and multiple YAML shapes all moved from "ask nicely" to "parse defensively /
post-filter / gate" in this period — durable fixes came from prompt clarity and code-side tolerance
together, not from relying on either alone.

# Possible Next Steps

*(Inferred from what recurred even after this period's fixes, not stated directly.)* The `depends_on`
chain's pattern — a fix for one YAML shape edge case exposing or reintroducing a different one — is
exactly the kind of problem a later principle in this project's history (treat anything mechanically
computable as a fact injected into the prompt, rather than something parsed out of unpredictable
free-form model output after the fact) would address more durably; whether that later principle was
consciously connected back to this period's experience, or arrived at independently, isn't
determinable from the commit record alone.
