#!/bin/bash
# Stop-event auto-install — rebuilds and installs canopy/roots whenever the actual
# source files under the relevant crates differ from what was installed last time.
#
# Deliberately content-hashed, NOT git-status-based: the previous inline version gated
# on `git status --porcelain` being non-empty (a dirty tree). That misses the exact case
# this repo's own Commit Discipline creates on purpose — commit promptly at each
# checkpoint — because the tree is already clean by the time this hook runs. Three
# checkpoint commits shipped in a row against a real dogfooding session before anyone
# noticed the installed binary was still the pre-fix build; see CLAUDE.md's "Diagnosing
# Dogfooding Runs" section and the git history around 2026-07-10 for the incident.
#
# Hashing the files on disk instead of asking git about commit/dirty state sidesteps the
# distinction entirely: committed or not, staged or not, this only cares whether the
# source actually changed since the last successful install.
set -uo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" || exit 0
cd "$REPO_ROOT" || exit 0

CRATES="canopy-cli roots-cli roots-parser roots-core roots-storage roots-context canopy-llm canopy-core"

CURRENT_HASH=$(find $CRATES -type f \( -name '*.rs' -o -name 'Cargo.toml' \) 2>/dev/null \
  | sort | xargs shasum 2>/dev/null | shasum | cut -d' ' -f1)

# No source files found (e.g. run from an unexpected cwd) — nothing to install.
[ -z "$CURRENT_HASH" ] && exit 0

MARKER="$REPO_ROOT/.claude/.last-install-hash"

if [ -f "$MARKER" ] && [ "$(cat "$MARKER")" = "$CURRENT_HASH" ]; then
  exit 0
fi

if cargo install --path canopy-cli --quiet && cargo install --path roots-cli --quiet; then
  echo "$CURRENT_HASH" > "$MARKER"
fi

exit 0
