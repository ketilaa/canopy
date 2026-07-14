# Canopy

Canopy is an AI-assisted software planning system. It generates specifications, extracts atomic
behaviors, surfaces business decisions that need a human answer instead of letting a model guess
at them, and produces per-component "contracts" meant to bound what an implementation step is
allowed to touch — rather than generating architecture and code in one large pass.

Canopy is also, deliberately, a record of its own development process. Every reliability fix in
this repository is backed by a reproducibility sweep — the same input run multiple times to
separate a real bug from model sampling noise — and the reasoning behind major design pivots is
preserved rather than discarded once the pivot is made. See [Where to Read Next](#where-to-read-next)
below; this is one of the more unusual things about this repository, and worth knowing before you
judge it by the code alone.

This is an active research-and-engineering project, not a finished product. See
[Project Status](#project-status) for what's real today.

## What Problem This Solves

Language models can generate plausible-looking code and plausible-looking architecture decisions.
They're less reliable at *knowing when a decision isn't theirs to make* — a business rule with no
stated basis, an architecture choice that should have required a human's sign-off, a duplicate of
something already decided. Canopy's specific bet, tested repeatedly against its own history (see
[`docs/principles/compute-facts-mechanically.md`](docs/principles/compute-facts-mechanically.md)
and
[`docs/narratives/from-prompt-engineering-to-mechanical-facts.md`](docs/narratives/from-prompt-engineering-to-mechanical-facts.md)),
is that this gap closes less by writing better prompts and more by mechanically computing whatever
the system can already determine on its own, reserving model judgment for genuine ambiguity, and
building deterministic checks that fail loudly rather than silently drift.

## Project Status

| Status | Item | Evidence |
|---|---|---|
| **Implemented** | Behavior-first planning pipeline: Story → Behaviors → Decisions → Clusters → Contracts (5 gated stages) | [`docs/design/behavior-first-planning.md`](docs/design/behavior-first-planning.md) — live-verified against real dogfooding stories |
| **Implemented** | Entity/Event Continuity gates (mechanical checks that a generated artifact matches already-established project vocabulary) | [`docs/principles/cross-artifact-consistency-audits-prevent-drift.md`](docs/principles/cross-artifact-consistency-audits-prevent-drift.md) |
| **Implemented** | Evidence-grounded Policy Discovery (business rules must cite a source or be marked unresolved, not guessed) | [`docs/blog-drafts/policy-discovery-vs-policy-invention.md`](docs/blog-drafts/policy-discovery-vs-policy-invention.md) — confirmed via controlled before/after reproducibility sweep |
| **Implemented** | Scenario Coverage Enumeration (test scenarios generated from a computed checklist, not holistic review) | [`docs/principles/coverage-should-be-generated-not-discovered.md`](docs/principles/coverage-should-be-generated-not-discovered.md) |
| **Designed, not yet wired up** | Contract-driven implementation — `canopy implement` still runs on the older ADR/architecture-skill-driven planner, not the new `contracts.yaml` | [`docs/narratives/the-road-to-contracts.md`](docs/narratives/the-road-to-contracts.md) |
| **Designed, not yet wired up** | Re-materializing a behavior once its blocking Decision Point is resolved | Noted as an open gap in the design doc's own Status section |
| **Experimental** | The reproducibility-sweep methodology itself | Proven valuable, but tested on a small number of stories so far — not a broad corpus |
| **Experimental** | Several individual principles, explicitly marked `maturity: emerging` in their own front matter | e.g. [`docs/principles/freeze-the-established-spec.md`](docs/principles/freeze-the-established-spec.md) |

## How It Works, Briefly

```
Story → Behaviors → Decisions → Clusters → Contracts → Files → Tests → Code
```

A story is a plain behavioral statement. Behaviors are atomic, taggable units extracted from it.
Unresolved business questions become explicit, gated Decision Points instead of being silently
answered. Behaviors are mechanically clustered by subject and kind. Each cluster becomes a
contract — an explicit, non-overlapping declaration of what a piece of code owns and depends on.
The full reasoning behind this design, including the specific failure that motivated it, is in
[`docs/design/behavior-first-planning.md`](docs/design/behavior-first-planning.md) and
[`docs/narratives/from-stories-to-behaviors.md`](docs/narratives/from-stories-to-behaviors.md).

## Getting Started

Requires Rust (edition 2021) and either a local OpenAI-compatible LLM server (llama.cpp's
`llama-server`, or Ollama) or an Anthropic API key.

```
git clone <this repo>
cd canopy
cargo build --workspace
cargo install --path canopy-cli
canopy
```

Canopy runs as an interactive REPL — commands are typed at the `canopy>` prompt, not passed as
shell arguments. Provider and model are configured per-agent in `.canopy/config.yaml`; see
`CLAUDE.md`'s "LLM Providers" section for the exact config shape and a local `llama-server` setup
example.

## Where to Read Next

This repository keeps four kinds of knowledge-capture documents, each answering a different
question:

- **[`docs/narratives/`](docs/narratives/)** — how the project's own thinking changed over weeks
  or months, not single incidents. Start with
  [`the-evolution-of-canopys-stated-purpose.md`](docs/narratives/the-evolution-of-canopys-stated-purpose.md)
  for framing, then
  [`from-prompt-engineering-to-mechanical-facts.md`](docs/narratives/from-prompt-engineering-to-mechanical-facts.md)
  — the project independently rediscovered the same fix four times across three weeks before
  naming it as a rule, which is arguably the most generalizable lesson in this repository. See
  [`docs/narrative-analysis.md`](docs/narrative-analysis.md) for a full reading order and a
  ranking of which narratives are strongest by evidence.
- **[`docs/principles/`](docs/principles/)** — evidence-graded engineering lessons, each with an
  explicit counter-evidence section stating where the lesson's own limits were found, not just
  where it worked.
- **[`docs/reports/`](docs/reports/)** — live dogfooding and reproducibility-sweep data: the
  closest thing in this repository to "does this actually work," with real incidence numbers
  before and after a fix, not just a claim that it does.
- **[`docs/retrospectives/`](docs/retrospectives/)** — the rawest record, day by day (or, for
  periods before the daily habit existed, reconstructed from commit history and marked as such).

## Limitations and Open Questions

- Contract-driven implementation is designed but not yet wired into `canopy implement` — see
  [`docs/narratives/the-road-to-contracts.md`](docs/narratives/the-road-to-contracts.md).
- The reproducibility-sweep methodology has been applied to a small number of stories so far, not
  validated at scale.
- Several principles are explicitly `maturity: emerging` rather than `validated` — check a given
  principle's own front matter before treating it as settled.
- This project does not yet have a broad test corpus across many project types, languages, or
  frameworks — dogfooding so far has focused on a small number of representative stories.

## License

Apache License 2.0 — see [`LICENSE`](LICENSE).
