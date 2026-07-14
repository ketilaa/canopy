# GitHub Readiness Checklist

Tracks execution of the backlog from `docs/github-readiness-assessment.md`. Updated after each
step. Status values: `not started` / `planned — awaiting approval` / `done`.

## Phase 1 — Private Cleanup

| # | Item | Status |
|---|---|---|
| 1 | Commit identity rewrite (→ `ketil.aasarod@gmail.com`) | done — verified across all 263 commits |
| 2 | License decision | done — Apache 2.0, `LICENSE` + per-crate metadata added |
| 3 | Fix `canopy-e-commerce` reference | done |
| 3 | Defensive `.gitignore` entries | done |

**Open item surfaced during execution, not in the original assessment:** global git config
(`~/.gitconfig`) still has the old `no.experis.com` email as `user.email` — not changed, per the
"never touch git config unasked" rule. Needs your decision.

## Phase 2 — Documentation Hardening

| # | Item | Status |
|---|---|---|
| 4 | README (structure + status + discoverability) | done — `README.md` added, all links verified |
| 5 | Project Status table | done — folded into README |
| 6 | Discoverability recommendations | done — folded into README |

## Phase 3 — Public GitHub Launch

| # | Item | Status |
|---|---|---|
| 8 | Push to GitHub | blocked — needs target repo details (see below) |

**Before this can execute:** which GitHub account/org, what repo name, and confirm visibility
should be public. Also recommend a final re-run of the secret/hygiene scan against the exact
commit about to be pushed, per the original assessment's Stage 3 guidance — not yet done.

## Phase 4-5 (not started)

Blog/Knowledge Site Launch, Community Onboarding — unchanged from the original assessment; not in
scope for this pass.
