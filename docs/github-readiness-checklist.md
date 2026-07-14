# GitHub Readiness Checklist

Tracks execution of the backlog from `docs/github-readiness-assessment.md`. Updated after each
step. Status values: `not started` / `planned — awaiting approval` / `done`.

## Phase 1 — Private Cleanup

| # | Item | Status |
|---|---|---|
| 1 | Commit identity rewrite (→ `ketil.aasarod@gmail.com`) | done — verified across all 263 commits |
| 2 | License decision | done — Apache 2.0, `LICENSE` + per-crate metadata added |
| 3 | Fix dogfooding-project-name reference | done |
| 3 | Defensive `.gitignore` entries | done |

**Open item surfaced during execution, resolved:** global git config was updated to
`ketil.aasarod@gmail.com` on explicit request. Caught during the final pre-push scan: 3 commits
made *after* the identity rewrite but *before* this config update still carried the old email, and
2 readiness docs re-introduced the dogfooding project's real name while documenting the earlier
fix. Both closed with a second, smaller filter-repo pass and a doc edit, re-verified before
pushing.

## Phase 2 — Documentation Hardening

| # | Item | Status |
|---|---|---|
| 4 | README (structure + status + discoverability) | done — `README.md` added, all links verified |
| 5 | Project Status table | done — folded into README |
| 6 | Discoverability recommendations | done — folded into README |

## Phase 3 — Public GitHub Launch

| # | Item | Status |
|---|---|---|
| 8 | Push to GitHub | **done** — https://github.com/ketilaa/canopy, public, 267 commits, single verified author identity, Apache-2.0 auto-detected |

Final pre-push scan (secrets, project-name leak, author identity, tree-content integrity, build)
re-run against the exact commit pushed — all clean. `canopy-backup-2/` kept on disk as a safety net
until the pushed repo has been spot-checked in a browser; safe to delete after that.

## Phase 4-5 (not started)

Blog/Knowledge Site Launch, Community Onboarding — unchanged from the original assessment; not in
scope for this pass.
