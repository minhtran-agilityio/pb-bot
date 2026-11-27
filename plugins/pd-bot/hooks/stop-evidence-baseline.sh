#!/bin/bash
# SessionStart hook: the other half of stop-evidence.sh (#191, B5).
# Records which files are already dirty when a session starts, with a hash of each file's content,
# so the Stop hook counts only files changed after the start. A tree that was dirty before the
# session, or an untracked scratch file, then does not make a read-only session look like it
# changed code.
#
# Input   SessionStart JSON on stdin: session_id.
# Output  none. The record is <git dir>/pd-bot/stop-evidence/<session_id>.baseline, one
#         "<content hash><TAB><path>" line per dirty path (hash "gone" for a deleted path).
#         It lives in the git directory, so it is never committed and never shows in git status.
# Rules   First record wins: a resume or a compaction keeps the record of the session start.
#         Fails open: any problem means no record, and the Stop hook falls back to git status.
set -u

# The stop-evidence judge runs claude -p, which loads this hook again. The guard ends that run.
[ -n "${STOP_EVIDENCE_GUARD:-}" ] && exit 0

INPUT=$(cat)
REPO_DIR="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null)}"
[ -n "$REPO_DIR" ] || exit 0

SID=$(printf '%s' "$INPUT" | grep -o '"session_id"[[:space:]]*:[[:space:]]*"[^"]*"' | head -n 1 \
  | sed -e 's/^.*:[[:space:]]*"//' -e 's/"$//' | tr -c 'A-Za-z0-9._\n-' '_')
[ -n "$SID" ] || exit 0

DIR=$(git -C "$REPO_DIR" rev-parse --git-path pd-bot/stop-evidence 2>/dev/null) || exit 0
case "$DIR" in /*) ;; *) DIR="$REPO_DIR/$DIR" ;; esac
mkdir -p "$DIR" 2>/dev/null || exit 0
FILE="$DIR/$SID.baseline"
[ -e "$FILE" ] && exit 0

# fingerprint: paths on stdin -> "<hash><TAB><path>" lines. One hash-object call for all files.
fingerprint() {
  local paths exist gone
  paths=$(cat)
  exist=$(printf '%s\n' "$paths" | while IFS= read -r p; do [ -n "$p" ] && [ -f "$REPO_DIR/$p" ] && printf '%s\n' "$p"; done)
  gone=$(printf '%s\n' "$paths" | while IFS= read -r p; do [ -n "$p" ] && [ ! -f "$REPO_DIR/$p" ] && printf '%s\n' "$p"; done)
  if [ -n "$exist" ]; then
    paste -d '\t' <(printf '%s\n' "$exist" | git -C "$REPO_DIR" hash-object --no-filters --stdin-paths 2>/dev/null) <(printf '%s\n' "$exist")
  fi
  if [ -n "$gone" ]; then printf '%s\n' "$gone" | awk '{print "gone\t" $0}'; fi
}

TMP=$(mktemp "$DIR/.tmp.XXXXXX" 2>/dev/null) || exit 0
git -C "$REPO_DIR" -c core.quotepath=off status --porcelain -uall 2>/dev/null \
  | sed -E 's/^.{3}//; s/^.* -> //; s/^"(.*)"$/\1/' | fingerprint > "$TMP" 2>/dev/null
mv -f "$TMP" "$FILE" 2>/dev/null || rm -f "$TMP"

# Records of finished sessions are useless after a while.
find "$DIR" -type f -mtime +14 -delete 2>/dev/null || true
exit 0
