#!/bin/bash
# SessionStart hook: tell the Pod Bay session of the main workspace that it is the orchestrator.
# Every other session gets nothing: a worker, a reviewer and an advisor take their role from
# their brief. Design: docs/adr/0001 and 0002.
#
# Signal  Pod Bay sets tmux options on each session it starts. @tn_context is "<project>/<repo>"
#         in the main workspace (src) and "<project>/<repo>@NN" in a parallel workspace (src-NN).
#         These options are not documented by Pod Bay: when they are missing, the hook is silent.
# Order   1. Only an interactive session (CLAUDE_CODE_ENTRYPOINT=cli): a nested `claude -p` (the
#            stop-evidence judge is one) inherits TMUX_PANE and PD_BOT_ROLE and must not get the role.
#         2. PD_BOT_ROLE: "orchestrator" gives the role, any other value gives none.
#         3. Inside tmux, the session is Pod Bay's (@tn_kind=claude), it runs in this project
#            (@claude_dir), and its pane was started as `claude ...` (not a shell the user split off).
#         4. @tn_context has no @NN suffix.
#         5. <project>/.claude/pd-bot.json does not say {"orchestrator": "off"}.
# Output  The SessionStart JSON envelope with hooks/orchestrator-role.md as additionalContext.
#         With --role: the word "orchestrator" or nothing (pr-opened.sh asks this way).
# Fails open: any doubt or error prints nothing and exits 0. No jq, no timeout.
MODE="${1:-}"
HERE="$(cd "$(dirname "$0")" && pwd)"
TEXT="$HERE/orchestrator-role.md"
PROJECT="${CLAUDE_PROJECT_DIR:-$PWD}"
CONTEXT=""

# opt <format>: one tmux value of this pane. One call per value: a separator byte in a single
# call is not safe, because a tmux version may print a control byte as escaped text.
opt() { tmux display-message -p -t "$TMUX_PANE" "$1" 2>/dev/null; }
# canon <dir>: the physical path, so a symlinked parent does not break the comparison.
canon() { (cd "$1" 2>/dev/null && pwd -P) || printf '%s' "${1%/}"; }

# is_orchestrator: exit 0 when this session has the role. Sets CONTEXT when tmux gave a plain one.
is_orchestrator() {
  [ "${CLAUDE_CODE_ENTRYPOINT:-}" = "cli" ] || return 1
  [ -z "${STOP_EVIDENCE_GUARD:-}" ] || return 1
  if [ -n "${PD_BOT_ROLE:-}" ]; then
    [ "$PD_BOT_ROLE" = "orchestrator" ]; return
  fi
  [ -n "${TMUX_PANE:-}" ] || return 1
  command -v tmux >/dev/null 2>&1 || return 1
  local kind context dir start
  kind=$(opt '#{@tn_kind}') || return 1
  [ "$kind" = "claude" ] || return 1
  dir=$(opt '#{@claude_dir}') || return 1
  [ -n "$dir" ] || return 1
  [ "$(canon "$dir")" = "$(canon "$PROJECT")" ] || return 1
  # Pod Bay starts the pane as `claude --session-id ...` or `claude --resume ...`.
  start=$(opt '#{pane_start_command}') || return 1
  start="${start#\"}"
  case "${start%%[[:space:]]*}" in claude|*/claude) ;; *) return 1 ;; esac
  context=$(opt '#{@tn_context}') || return 1
  case "$context" in
    ''|*@*) return 1 ;;
    */*) ;;
    *) return 1 ;;
  esac
  config_allows || return 1
  # The context goes into a sed expression and a JSON string: name it only when it is plain.
  case "$context" in *[!A-Za-z0-9._/-]*) ;; *) CONTEXT="$context" ;; esac
  return 0
}

# config_allows: exit 1 when the project config switches the role off, or cannot be read.
# One rule for every tool: the values "off" and false switch the role off, and a file that is
# not a JSON object switches it off too (no surprise from a broken file).
config_allows() {
  local file="$PROJECT/.claude/pd-bot.json" value=""
  [ -f "$file" ] || return 0
  if command -v python3 >/dev/null 2>&1; then
    value=$(python3 -I -c '
import json, sys
try:
    d = json.load(open(sys.argv[1], encoding="utf-8"))
    v = d.get("orchestrator") if isinstance(d, dict) else "invalid"
except Exception:
    v = "invalid"
print("off" if (v == "off" or v is False) else "invalid" if v == "invalid" else "on")
' "$file" 2>/dev/null) || value=""
    # A python3 that cannot run (the macOS stub) gives no value: try the next tool.
    case "$value" in on) return 0 ;; off|invalid) return 1 ;; esac
  fi
  if command -v jq >/dev/null 2>&1; then
    value=$(jq -r 'if type != "object" then "invalid" elif (.orchestrator == "off" or .orchestrator == false) then "off" else "on" end' "$file" 2>/dev/null) || value="invalid"
    [ "$value" = "on" ]; return
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
CR=$(printf '\r')
# CONTEXT holds only [A-Za-z0-9._/-] (checked above), so it is safe inside the sed expression
# with a | delimiter and inside the JSON string. sed gets a literal tab and carriage return:
# BSD sed does not read \t or \r as escapes.
BODY=$(sed -e "s|{{context}}|${CONTEXT:-this repository}|g" \
           -e 's/\\/\\\\/g' -e 's/"/\\"/g' -e "s/$TAB/\\\\t/g" -e "s/$CR\$//" "$TEXT" \
  | awk '{ printf "%s%s", $0, "\\n" }')
[ -n "$BODY" ] || exit 0
printf '{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"%s"}}\n' "$BODY"
exit 0
