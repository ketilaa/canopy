# 2026-06-19 — Day 0: the first pivot

**source: reconstructed** — written 2026-07-14 from commit history only (5 commits: `32cf41e`,
`795676a`, `7884aea`, `c419d19`, `2805e03`). No first-hand session record exists for this day; do
not treat this as a first-hand account. Inferred entirely from commit messages and diffs — nothing
here is invented beyond what those artifacts support.

---

# What Changed

Canopy V1 shipped as an "idea exploration engine" (`32cf41e`): a four-crate Rust workspace that
turns a vague idea into structured YAML — vision, requirements, domain model, architecture — via
one LLM call per artifact. The architecture schema was rigid: `frontend`, `backend`, `database`,
`deployment` as plain typed string fields.

Three consecutive same-day commits (`795676a`, `7884aea`, `c419d19`) each widened one more field to
`serde_yaml::Value` because the model kept returning richer, nested structures than the schema
allowed — per-service configs, multi-field deployment blocks, reasoning text containing colons.

The same day ended with `2805e03`: the rigid `Requirements`/`Architecture` types were removed
entirely, replaced by `DeliveryIntents` and `ArchitecturePrinciples`/`StructuralCommitments`.
Architecture would now state only structural commitments that must hold across all delivery
intents — not a prescribed stack — and requirements would defer to intent-start time rather than
being captured upfront.

# What We Learned

The three field-widening commits were a symptom, not a fix. Loosening one field's type after a
parse failure just meant the next field would fail the same way on the next generation. The actual
problem wasn't the field types — it was that a single, fully-specified, upfront architecture
schema was the wrong shape of artifact to ask an LLM to produce in one shot, regardless of how
loosely each field was typed.

# What Surprised Us

Nothing in the commit record suggests this was anticipated going in — `32cf41e`'s own CLAUDE.md
addition states a broader ambition ("planning and implementation engine," "Planning Before Coding")
that the rigid V1 schema doesn't yet match. The pivot commit doesn't cite the three preceding parse
failures explicitly as its reason; the rationale given is architectural (defer requirements, state
commitments not frameworks), suggesting the widening commits may have been the proximate trigger
that surfaced a design problem the stated ambition had already implied.

# What We Believe Now

*(Reconstructed inference, not a verbatim belief statement from the commits — flagged as
interpretation.)* Based on what `2805e03` actually changed: state only structural commitments that
must hold across all future intents, not concrete framework choices, and defer detailed requirements
capture to when a specific intent is being worked rather than requiring them upfront. This is the
first visible instance of what later becomes the project's explicit "everything emerges, nothing is
decided upfront" design philosophy (formalized far later, but the shape is already present here).

# Possible Next Steps

*(What the evidence suggests was likely considered next, inferred from what actually shipped in the
following days — not stated directly in this day's commits.)* The new `DeliveryIntents`/
`ArchitecturePrinciples` types would need a real command surface to populate and consume them — the
following days' commits (planning phase, `canopy intent`, domain registry) suggest this was the
immediate follow-on, though that inference belongs to the next reconstructed period, not this one.
