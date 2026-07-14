# 2026-06-23 to 2026-06-25 — From upfront questions to emergent behavior

**source: reconstructed** — written 2026-07-14 from commit history only (43 commits across these
three days). No first-hand session record exists for this period. Inferred entirely from commit
messages and diffs — nothing here is invented beyond what those artifacts support.

---

# What Changed

At the start of this period, planning still assumed upfront elicitation: `canopy explore` asked up
to three questions (who/what/why) before producing a vision document, and roles/boundaries were
meant to be captured through those questions.

By the end of the period, nearly every one of those upfront elements had been removed or replaced
with something derived from actual behavioral statements: user roles now emerge from `as_a` fields
in stories, not from explore questions (`0e3fa34`); boundaries were dropped from explore entirely,
with explore now allowed to ask zero questions (`2661da1`); clarifying questions were removed from
explore altogether, stated as adding "friction without value" (`85a7f1b`); and finally explore was
renamed to `init` and the vision document was dropped completely — `init` now does one thing (save
the idea), with no LLM call at all (`470ca03`).

In parallel, domain vocabulary extraction was automated from intent statements (`ba26b83`), and
CLAUDE.md itself was rewritten around this shift — "Redesign CLAUDE.md: accurate, concise,
emergent-first" (`603364d`) — turning what had been an implicit direction into explicit doctrine.

A real bug cluster also surfaced and got fixed mid-period (`03e6ac0`): duplicate ADRs being proposed
for categories already decided, infrastructure ADRs (database, event broker) overwriting the owning
service's own technology field instead of adding a separate entry, and event names coming out
kebab-case because a blanket naming rule was misapplied to events.

# What We Learned

Upfront questions produced premature, speculative, or leaked answers. Domain extraction was
observed pulling from the `so_that` (beneficiary) field rather than the actual action, and stories
were naming implementation details ("in the catalog", "via the API") before any architecture had
been decided (`30b656b`). Event extraction was found adding speculative events alongside the one
actually described — an `Updated` event proposed next to a `Created` event with no textual basis for
it (`7f5efe6`).

The pattern across roles, boundaries, vocabulary, and tech stack was the same each time: information
solicited abstractly and early tended to be wrong, premature, or over-generated, while the same
information derived from a specific described behavior was more accurate and better-scoped.

# What Surprised Us

The shift wasn't total. A bootstrap step was reintroduced two days after boundaries and questions
were being stripped out — `init` gained LLM-suggested candidate entities/roles as a pre-selected,
editable multi-select (`3eaebed`, `5d664be`). Pure emergence-from-behavior-only wasn't the final
answer; a seeded-but-correctable starting point survived alongside incremental accumulation. This
reads as the project discovering the limits of "ask nothing upfront," not just committing further to
it.

The bug cluster in `03e6ac0` also showed a specific and non-obvious failure mode: infrastructure
ADRs (database, broker) and structural ADRs both wrote into the same `technology`/`component_type`
fields on a service record, so whichever proposal landed second could silently overwrite the first
rather than adding independent information — several more field-ownership races surfaced in the days
following (`2167060`, `2782f23`, `82c6b20`, `ef66214`).

# What We Believe Now

*(Reconstructed inference from the pattern across this period's commits, not a verbatim belief
statement.)* Elicit information late, from concrete described behavior, rather than early through
abstract categorical questions — and when emergent extraction is the default, generation prompts
need explicit "don't anticipate, don't decompose, don't duplicate" guardrails, because the failure
mode shifts from "missing information" to "the model over-generating or corrupting accumulated
state."

# Possible Next Steps

*(Inferred from what the evidence suggests remained open, not stated directly.)* The field-ownership
races (multiple ADR categories writing into the same service record fields) suggest the underlying
services registry needed a more careful merge/ownership model than "later proposal overwrites
earlier one" — later commits in this same window (`2167060`, `2782f23`) look like incremental patches
to this, not a structural fix; whether a more principled reconciliation approach was needed is a
natural open question from this period's evidence.
