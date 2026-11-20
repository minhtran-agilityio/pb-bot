#!/bin/bash
# SessionStart hook: tell the Pod Bay session of the main workspace that it is the orchestrator.
# Every other session gets nothing: a worker, a reviewer and an advisor take their role from
# their brief. Design: docs/adr/0001 and 0002.
#
# Signal  Pod Bay sets tmux options on each session it starts. @tn_context is "<project>/<repo>"
#         in the main workspace (src) and "<project>/<repo>@NN" in a parallel workspace (src-NN).
#         These options are not documented by Pod Bay: when they are missing, the hook is silent.
# Order   1. PD_BOT_ROLE: "orchestrator" gives the role, any other value gives none.
#         2. Only an interactive session (CLAUDE_CODE_ENTRYPOINT=cli) inside tmux: a nested
#            `claude -p` in the same pane inherits TMUX_PANE and must not get the role.
#         3. The session is Pod Bay's (@tn_kind=claude), it runs in this project (@claude_dir),
#            and its pane was started as `claude ...` (not a shell the user split off).
#         4. @tn_context has no @NN suffix.
#         5. <project>/.claude/pd-bot.json does not say {"orchestrator": "off"}.
# Output  The SessionStart JSON envelope with hooks/orchestrator-role.md as additionalContext.
#         With --role: the word "orchestrator" or nothing (pr-opened.sh asks this way).
# Fails open: any doubt or error prints nothing and exits 0. No jq, no timeout.
MODE="${1:-}"
HERE="$(cd "$(dirname "$0")" && pwd)"
TEXT="$HERE/orchestrator-role.md"
PROJECT="${CLAUDE_PROJECT_DIR:-$PWD}"
SEP=$(printf '\037')
CONTEXT=""

# is_orchestrator: exit 0 when this session has the role. Sets CONTEXT when tmux gave one.
is_orchestrator() {
  if [ -n "${PD_BOT_ROLE:-}" ]; then
    [ "$PD_BOT_ROLE" = "orchestrator" ]; return
  fi
  [ "${CLAUDE_CODE_ENTRYPOINT:-}" = "cli" ] || return 1
  [ -n "${TMUX_PANE:-}" ] || return 1
  command -v tmux >/dev/null 2>&1 || return 1
  local raw kind context dir start
  raw=$(tmux display-message -p -t "$TMUX_PANE" \
    "#{@tn_kind}${SEP}#{@tn_context}${SEP}#{@claude_dir}${SEP}#{pane_start_command}" 2>/dev/null) || return 1
  IFS="$SEP" read -r kind context dir start <<EOF
$raw
EOF
  [ "$kind" = "claude" ] || return 1
  [ -n "$dir" ] && [ "${dir%/}" = "${PROJECT%/}" ] || return 1
  # Pod Bay starts the pane as `claude --session-id ...` or `claude --resume ...`.
  start="${start#\"}"
  case "${start%%[[:space:]]*}" in claude|*/claude) ;; *) return 1 ;; esac
  case "$context" in
    ''|*@*|*[!A-Za-z0-9._/-]*) return 1 ;;
    */*) ;;
    *) return 1 ;;
  esac
  config_allows || return 1
  CONTEXT="$context"
}

# config_allows: exit 1 when the project config switches the role off, or cannot be read.
config_allows() {
  local file="$PROJECT/.claude/pd-bot.json" value
  [ -f "$file" ] || return 0
  if command -v python3 >/dev/null 2>&1; then
    value=$(python3 -I -c '
import json, sys
try:
    d = json.load(open(sys.argv[1], encoding="utf-8"))
    v = d.get("orchestrator") if isinstance(d, dict) else "invalid"
except Exception:
    v = "invalid"
print("off" if v in ("off", False) else "invalid" if v == "invalid" else "on")
' "$file" 2>/dev/null) || return 1
    [ "$value" = "on" ]; return
  fi
  if command -v jq >/dev/null 2>&1; then
    value=$(jq -r 'if type == "object" then (.orchestrator | tostring) else "invalid" end' "$file" 2>/dev/null) || return 1
    case "$value" in off|false|invalid) return 1 ;; *) return 0 ;; esac
  fi
  ! grep -Eq '"orchestrator"[[:space:]]*:[[:space:]]*("off"|false)' "$file"
}

is_orchestrator || exit 0
if [ "$MODE" = "--role" ]; then
  echo orchestrator
  exit 0
fi

[ -f "$TEXT" ] || exit 0
TAB=$(printf '\t')
# CONTEXT holds only [A-Za-z0-9._/-] (checked above), so it is safe inside the sed expression
# with a | delimiter and inside the JSON string.
BODY=$(sed -e "s|{{context}}|${CONTEXT:-this repository}|g" \
           -e 's/\\/\\\\/g' -e 's/"/\\"/g' -e "s/$TAB/\\\\t/g" -e 's/\r$//' "$TEXT" \
  | awk '{ printf "%s%s", $0, "\\n" }')
[ -n "$BODY" ] || exit 0
printf '{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"%s"}}\n' "$BODY"
exit 0
