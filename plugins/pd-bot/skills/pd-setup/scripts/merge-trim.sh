#!/bin/bash
# Merge the token trim (fleet-trim.json: no claude.ai connectors, unused plugins off) into a
# project settings.json, so each request of every session there carries fewer tool and skill lists.
# Usage: merge-trim.sh [--dry-run] [--compact-window N] <trim.json> <settings.json>
#   --compact-window N writes autoCompactWindow N (at least 100000) into this project only. Without it
#   the script writes no autoCompactWindow and keeps the one the project already has, so the user's
#   ~/.claude/settings.json value (or Claude Code's own window) applies.
#   A trim file's own autoCompactWindow is never copied: only this option sets a cap.
#   Sets every top-level key and each enabledPlugins entry of the trim file; keeps every
#   other key. Writes nothing when nothing changes (idempotent). A missing file is created as {}.
#   --dry-run prints the diff and changes nothing.
# Exit: 0 done or already merged, 1 usage, 2 a file is not a valid JSON object or enabledPlugins
#       is not an object (file untouched), 3 jq is not installed. Bash 3.2 safe.
set -eu

DRY=0
WINDOW=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --dry-run) DRY=1; shift ;;
    --compact-window)
      [ "$#" -ge 2 ] || { echo "merge-trim: --compact-window needs a number" >&2; exit 1; }
      case "$2" in ''|*[!0-9]*|??????????*) echo "merge-trim: --compact-window needs a number of at most 9 digits" >&2; exit 1 ;; esac
      [ "$2" -ge 100000 ] || { echo "merge-trim: --compact-window must be at least 100000" >&2; exit 1; }
      WINDOW=$2; shift 2 ;;
    *) break ;;
  esac
done
[ "$#" -eq 2 ] || { echo "usage: merge-trim.sh [--dry-run] [--compact-window N] <trim.json> <settings.json>" >&2; exit 1; }
TRIM=$1
FILE=$2
command -v jq >/dev/null 2>&1 || { echo "merge-trim: jq is required (install jq, then re-run)" >&2; exit 3; }

jq -e 'type == "object" and ((.enabledPlugins // {}) | type == "object")' "$TRIM" >/dev/null 2>&1 \
  || { echo "merge-trim: $TRIM is not a valid trim file" >&2; exit 2; }
if [ -f "$FILE" ] && [ -n "$(tr -d ' \t\r\n' < "$FILE")" ]; then
  jq -e 'type == "object" and ((.enabledPlugins // {}) | type == "object")' "$FILE" >/dev/null 2>&1 \
    || { echo "merge-trim: $FILE is not a valid JSON object or its enabledPlugins is not an object; not touching it" >&2; exit 2; }
  CUR=$(cat "$FILE")
else
  CUR='{}'
fi

NEW=$(printf '%s' "$CUR" | jq --slurpfile t "$TRIM" --arg w "$WINDOW" '
  . * ($t[0] | del(.enabledPlugins, .disableClaudeAiConnectors, .autoCompactWindow))
  | .disableClaudeAiConnectors = (if ($t[0] | has("disableClaudeAiConnectors"))
                                then $t[0].disableClaudeAiConnectors else true end)
  | .enabledPlugins = ((.enabledPlugins // {}) + ($t[0].enabledPlugins // {}))
  | if .enabledPlugins == {} then del(.enabledPlugins) else . end
  | if $w != "" then .autoCompactWindow = ($w | tonumber) else . end')

SAME=0
if [ -f "$FILE" ] && [ "$(printf '%s' "$CUR" | jq -S .)" = "$(printf '%s' "$NEW" | jq -S .)" ]; then SAME=1; fi

if [ "$DRY" -eq 1 ]; then
  if [ "$SAME" -eq 1 ]; then echo "token trim already present in $FILE"; exit 0; fi
  TMP=$(mktemp)
  printf '%s\n' "$CUR" | jq . > "$TMP"
  printf '%s\n' "$NEW" | diff -u "$TMP" - || true
  rm -f "$TMP"
  exit 0
fi

if [ "$SAME" -eq 1 ]; then echo "token trim already present in $FILE"; exit 0; fi
mkdir -p "$(dirname "$FILE")"
TMP=$(mktemp "$FILE.XXXXXX")
printf '%s\n' "$NEW" > "$TMP"
mv "$TMP" "$FILE"
echo "merged the token trim into $FILE"
