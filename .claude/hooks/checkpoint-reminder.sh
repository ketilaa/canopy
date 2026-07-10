#!/bin/bash
# Stop-event checkpoint REMINDER — informational only. Makes no git commits, no installs,
# invokes no nested agent: it only reads repo state and, at most once per distinct diff,
# emits a "block" decision whose reason surfaces back to the agent so a commit doesn't get
# silently skipped. See CLAUDE.md's Commit Discipline section for when to actually commit.
#
# Hash-gated so it can NEVER loop forever: once a given diff has been reminded about, it
# stays silent for that exact diff even if the agent chooses not to commit yet — it only
# fires again once the diff actually changes (new work) or after a commit clears it.
set -uo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" || exit 0
cd "$REPO_ROOT" || exit 0

CRATES="canopy-cli roots-cli roots-parser roots-core roots-storage roots-context canopy-llm canopy-core"

DIFF=$(git diff -- $CRATES 2>/dev/null)
UNTRACKED=$(git status --porcelain -- $CRATES 2>/dev/null | grep '^??' || true)

if [ -z "$DIFF" ] && [ -z "$UNTRACKED" ]; then
  exit 0
fi

MARKER="$REPO_ROOT/.claude/.checkpoint-reminder-hash"
CURRENT_HASH=$(git diff -- $CRATES 2>/dev/null | shasum | cut -d' ' -f1)-$(echo "$UNTRACKED" | shasum | cut -d' ' -f1)

if [ -f "$MARKER" ] && [ "$(cat "$MARKER")" = "$CURRENT_HASH" ]; then
  exit 0
fi

echo "$CURRENT_HASH" > "$MARKER"

CHANGED_FILES=$(git status --porcelain -- $CRATES 2>/dev/null | awk '{print $2}' | tr '\n' ' ')

cat <<EOF
{"decision": "block", "reason": "Checkpoint reminder: uncommitted changes in $CHANGED_FILES. Per CLAUDE.md's Commit Discipline section, if build/test are green and this is a natural checkpoint, commit it now with a real message before ending the turn."}
EOF
