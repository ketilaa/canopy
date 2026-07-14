#!/bin/bash
# Stop-event PUBLICATION reminder — informational only, mirrors checkpoint-reminder.sh's safety
# shape (no mutating git commands, no network calls, hash-gated once per HEAD). Distinct in intent
# from checkpoint-reminder.sh: that one flags "you have uncommitted work" — a local checkpoint.
# This one flags "you may have reached a publication checkpoint" — a fundamentally different
# question, since a push is a publication event, not just another local checkpoint, and shouldn't
# be nagged about at the same frequency ordinary commits are.
#
# Trigger is deliberately NOT "branch is ahead of upstream" alone — during normal development it's
# expected to sit on many local commits, and firing on every one would just be noise that gets the
# hook ignored. Instead: fire only when (a) upstream is behind AND at least one changed file
# touches a publication-relevant path, or (b) the branch has diverged far enough (>=15 commits)
# that it's worth surfacing regardless of what changed — a mechanical fallback against the path
# list going stale or missing something, not a substitute for the path check.
#
# The path list is a MECHANICAL FACT the hook can check (did this diff touch docs/principles/?),
# not a heuristic judgment about whether the change is "important" — the same "compute facts
# mechanically, don't ask a heuristic question" principle this project keeps converging on
# elsewhere, applied here to hook design itself. "Significant architecture changes" isn't its own
# path check — this project's own convention is that architecture decisions get written down in
# docs/design/ or CLAUDE.md, so checking those paths is the mechanical stand-in for "architecture
# changed" rather than trying to infer significance from an arbitrary source diff.
#
# docs/blog-drafts/ is deliberately excluded: those are pre-publication drafts by design (see the
# GitHub readiness assessment's knowledge-capture section), not yet meant to be treated as
# publication-ready just because they exist.
#
# Hash-gated so it can NEVER loop forever: once a given HEAD has been reminded about, it stays
# silent for that exact HEAD even if the user chooses not to push yet — "not pushing yet" is a
# completely valid outcome, not something this hook overrides. It only fires again once HEAD
# actually changes (a new local commit) or a push clears it.
set -uo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" || exit 0
cd "$REPO_ROOT" || exit 0

BRANCH="$(git rev-parse --abbrev-ref HEAD 2>/dev/null)" || exit 0
[ "$BRANCH" = "HEAD" ] && exit 0   # detached HEAD — nothing meaningful to compare

UPSTREAM="$(git rev-parse --abbrev-ref --symbolic-full-name '@{u}' 2>/dev/null)" || exit 0
# No upstream configured for this branch at all — not this hook's job to suggest setting one up.

AHEAD="$(git rev-list --count '@{u}..HEAD' 2>/dev/null)" || exit 0
[ "$AHEAD" -eq 0 ] 2>/dev/null && exit 0

CHANGED_FILES="$(git diff --name-only '@{u}..HEAD' 2>/dev/null)" || exit 0

# Mechanical path-category check — each category is a fixed, listed path prefix, not a heuristic.
MATCHED_CATEGORIES=()

check_category() {
  local label="$1"
  local pattern="$2"
  if echo "$CHANGED_FILES" | grep -qE "$pattern"; then
    MATCHED_CATEGORIES+=("$label")
  fi
}

check_category "docs/principles/"        '^docs/principles/'
check_category "docs/narratives/"        '^docs/narratives/'
check_category "docs/retrospectives/"    '^docs/retrospectives/'
check_category "docs/reports/"           '^docs/reports/'
check_category "docs/design/"            '^docs/design/'
check_category "README.md"               '^README\.md$'
check_category "CLAUDE.md"               '^CLAUDE\.md$'
check_category "canopy-llm/src/prompts/" '^canopy-llm/src/prompts/'
check_category "canopy-llm/src/skills/"  '^canopy-llm/src/skills/'
check_category "docs/READING_ORDER.md"   '^docs/READING_ORDER\.md$'

PATH_MATCH=0
[ "${#MATCHED_CATEGORIES[@]}" -gt 0 ] && PATH_MATCH=1

SIZE_FALLBACK=0
[ "$AHEAD" -ge 15 ] 2>/dev/null && SIZE_FALLBACK=1

if [ "$PATH_MATCH" -eq 0 ] && [ "$SIZE_FALLBACK" -eq 0 ]; then
  exit 0
fi

CURRENT_HEAD="$(git rev-parse HEAD)"
MARKER="$REPO_ROOT/.claude/.push-reminder-hash"

if [ -f "$MARKER" ] && [ "$(cat "$MARKER")" = "$CURRENT_HEAD" ]; then
  exit 0
fi

echo "$CURRENT_HEAD" > "$MARKER"

REASON="Publication reminder: $AHEAD commit(s) ahead of '$UPSTREAM' on '$BRANCH'."
if [ "$PATH_MATCH" -eq 1 ]; then
  CATS=$(printf ', %s' "${MATCHED_CATEGORIES[@]}")
  CATS="${CATS:2}"
  REASON="$REASON Touches publication-relevant paths: $CATS."
fi
if [ "$SIZE_FALLBACK" -eq 1 ] && [ "$PATH_MATCH" -eq 0 ]; then
  REASON="$REASON No specific publication-relevant path matched, but this is a large amount of unpushed work worth a look regardless."
fi
REASON="$REASON If this is a natural publication checkpoint, ask the user before running git push. This is a reminder, not a gate -- choosing not to push yet is a completely valid outcome."

REASON_ESCAPED=$(echo "$REASON" | sed 's/\\/\\\\/g; s/"/\\"/g')

cat <<EOF
{"decision": "block", "reason": "$REASON_ESCAPED"}
EOF
