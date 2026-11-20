#!/bin/bash
# Contract tests for hooks/session-role.sh: which session gets the orchestrator role text.
# The table is "signals -> role". The signals are PD_BOT_ROLE, the entry point, TMUX_PANE, the
# tmux options Pod Bay sets on a session (read through a fake tmux on PATH), and the project
# config .claude/pd-bot.json. Every case that is not the Pod Bay session of the main workspace
# must print nothing and exit 0 (fail open).
source "$(dirname "$0")/../lib.sh"

HOOK="$ROOT/plugins/pd-bot/hooks/session-role.sh"
SEP=$'\037'

BIN=$(mktemp -d); _CLEANUP_DIRS+=("$BIN")
cat > "$BIN/tmux" <<'EOF'
#!/bin/bash
# Fake tmux: prints FAKE_TMUX_OUT and exits FAKE_TMUX_RC, whatever the arguments.
[ "${FAKE_TMUX_RC:-0}" -eq 0 ] || exit "$FAKE_TMUX_RC"
printf '%s\n' "${FAKE_TMUX_OUT:-}"
EOF
chmod +x "$BIN/tmux"
# A PATH with the tools the hook needs but no tmux, for the "tmux is not installed" case.
NOTMUX=$(mktemp -d); _CLEANUP_DIRS+=("$NOTMUX")
for tool in bash sed awk grep cat dirname printf python3 jq tr; do
  p=$(command -v "$tool" 2>/dev/null) && [ -x "$p" ] && ln -s "$p" "$NOTMUX/$tool"
done

PROJ=$(mktemp -d); _CLEANUP_DIRS+=("$PROJ")
SID="claude --session-id 6ba55c9b-0000-4000-8000-000000000000 --remote-control \"Pb Bot #UA-00\" -n \"Pb Bot #UA-00\""

# Defaults of one case; each test overrides what it needs.
reset() {
  ROLE_ENV="-"; ENTRY="cli"; PANE="%3"; KIND="claude"; CONTEXT="pb-bot/pb-bot"; CDIR="$PROJ"; START="$SID"
  TMUX_RC=0; USE_PATH="$BIN:$PATH"; MODE=""; SOURCE="startup"
  rm -rf "$PROJ/.claude"
}

# run: sets OUT (stdout and stderr) and RC.
run() {
  local args=(-u PD_BOT_ROLE -u TMUX_PANE -u CLAUDE_CODE_ENTRYPOINT -u CLAUDE_PLUGIN_ROOT
    "PATH=$USE_PATH" "CLAUDE_PROJECT_DIR=$PROJ" "FAKE_TMUX_RC=$TMUX_RC"
    "FAKE_TMUX_OUT=$KIND$SEP$CONTEXT$SEP$CDIR$SEP$START")
  [ "$ROLE_ENV" = "-" ] || args+=("PD_BOT_ROLE=$ROLE_ENV")
  [ "$ENTRY" = "-" ] || args+=("CLAUDE_CODE_ENTRYPOINT=$ENTRY")
  [ "$PANE" = "-" ] || args+=("TMUX_PANE=$PANE")
  # shellcheck disable=SC2086
  OUT=$(cd "$PROJ" && printf '{"session_id":"s","cwd":"%s","hook_event_name":"SessionStart","source":"%s"}' "$PROJ" "$SOURCE" \
    | env "${args[@]}" bash "$HOOK" $MODE 2>&1); RC=$?
}

is_orchestrator() {
  if [ "$RC" -eq 0 ] && printf '%s' "$OUT" | jq -e '
      .hookSpecificOutput.hookEventName == "SessionStart"
      and (.hookSpecificOutput.additionalContext | test("orchestrator"))' >/dev/null 2>&1; then pass
  else fail "want the role text — rc=$RC out: $(printf '%s' "$OUT" | head -c 200)"; fi
}
is_none() {
  if [ "$RC" -eq 0 ] && [ -z "$OUT" ]; then pass
  else fail "want no output and rc 0 — rc=$RC out: $(printf '%s' "$OUT" | head -c 200)"; fi
}

t "1: the Pod Bay session of the main workspace gets the role"
reset; run; is_orchestrator

t "2: a session in a parallel workspace (context ends in @NN) gets nothing"
reset; CONTEXT="pb-bot/pb-bot@05"; run; is_none

t "3: a resumed Pod Bay session (claude --resume) gets the role"
reset; START="claude --resume 9471ccb7-0000-4000-8000-000000000000 --remote-control \"Mnemo #UA-01\""; run; is_orchestrator

t "4: a project whose repo has another name gets the role by its context, not by the tmux name"
reset; CONTEXT="tokscope/claude-code-tokscope"; run; is_orchestrator

t "5: no TMUX_PANE (plain claude outside tmux) gets nothing"
reset; PANE="-"; run; is_none

t "6: a tmux session Pod Bay did not start (no @tn_kind) gets nothing"
reset; KIND=""; CONTEXT=""; CDIR=""; START="zsh"; run; is_none

t "7: plain claude in a split pane of the Pod Bay session (pane started as a shell) gets nothing"
reset; START="zsh"; run; is_none

t "8: a nested claude -p in the same pane (entry point sdk-cli) gets nothing"
reset; ENTRY="sdk-cli"; run; is_none

t "8b: an unset entry point gets nothing"
reset; ENTRY="-"; run; is_none

t "9: the session directory Pod Bay recorded differs from this project: nothing"
reset; CDIR="/somewhere/else"; run; is_none

t "10: the project config switches the role off"
reset; mkdir -p "$PROJ/.claude"; printf '{"orchestrator": "off"}\n' > "$PROJ/.claude/pd-bot.json"; run; is_none

t "10b: a project config without the switch keeps the default (on)"
reset; mkdir -p "$PROJ/.claude"; printf '{"other": 1}\n' > "$PROJ/.claude/pd-bot.json"; run; is_orchestrator

t "11: PD_BOT_ROLE=orchestrator gives the role with no tmux and a config that says off"
reset; ROLE_ENV="orchestrator"; PANE="-"; mkdir -p "$PROJ/.claude"; printf '{"orchestrator": "off"}\n' > "$PROJ/.claude/pd-bot.json"; run; is_orchestrator

t "12: PD_BOT_ROLE=none switches the role off for one session"
reset; ROLE_ENV="none"; run; is_none

t "13: tmux exits non-zero: nothing, exit 0"
reset; TMUX_RC=1; run; is_none

t "13b: tmux is not installed: nothing, exit 0"
reset; USE_PATH="$NOTMUX"; run; is_none

t "14: a project config that is not valid JSON: nothing, exit 0"
reset; mkdir -p "$PROJ/.claude"; printf '{"orchestrator": \n' > "$PROJ/.claude/pd-bot.json"; run; is_none

for src in resume clear compact; do
  t "15: source=$src gives the same role as startup"
  reset; SOURCE="$src"; run; is_orchestrator
  t "15: source=$src in a parallel workspace still gives nothing"
  reset; SOURCE="$src"; CONTEXT="pb-bot/pb-bot@02"; run; is_none
done

t "16: the output is valid JSON and the role text keeps its quotes, backslashes and tabs"
if ! command -v python3 >/dev/null 2>&1; then skip "python3 is needed to compare the JSON text"
else
  d=$(mktemp -d); _CLEANUP_DIRS+=("$d"); mkdir -p "$d/hooks"
  cp "$HOOK" "$d/hooks/"
  printf 'say "hi" \\ back\ttab for {{context}}\nline two\n' > "$d/hooks/orchestrator-role.md"
  reset
  OUT=$(cd "$PROJ" && env -u PD_BOT_ROLE "PATH=$BIN:$PATH" "CLAUDE_PROJECT_DIR=$PROJ" CLAUDE_CODE_ENTRYPOINT=cli TMUX_PANE=%3 \
    "FAKE_TMUX_OUT=$KIND$SEP$CONTEXT$SEP$CDIR$SEP$START" bash "$d/hooks/session-role.sh" 2>&1 </dev/null)
  if printf '%s' "$OUT" | python3 -c '
import json, sys
c = json.load(sys.stdin)["hookSpecificOutput"]["additionalContext"]
assert c == "say \"hi\" \\ back\ttab for pb-bot/pb-bot\nline two\n", repr(c)
'; then pass; else fail "out: $(printf '%s' "$OUT" | head -c 200)"; fi
fi

t "the role text names the context of the session"
reset; run
if printf '%s' "$OUT" | jq -e '.hookSpecificOutput.additionalContext | test("pb-bot/pb-bot")' >/dev/null 2>&1; then pass
else fail "out: $(printf '%s' "$OUT" | head -c 200)"; fi

t "a context with characters outside the expected set gets nothing"
reset; CONTEXT='pb-bot/pb"bot; rm'; run; is_none

t "the role text is missing: nothing, exit 0"
d=$(mktemp -d); _CLEANUP_DIRS+=("$d"); mkdir -p "$d/hooks"; cp "$HOOK" "$d/hooks/"
reset
OUT=$(cd "$PROJ" && env -u PD_BOT_ROLE "PATH=$BIN:$PATH" "CLAUDE_PROJECT_DIR=$PROJ" CLAUDE_CODE_ENTRYPOINT=cli TMUX_PANE=%3 \
  "FAKE_TMUX_OUT=$KIND$SEP$CONTEXT$SEP$CDIR$SEP$START" bash "$d/hooks/session-role.sh" 2>&1 </dev/null); RC=$?
is_none

t "--role prints the word orchestrator for the orchestrator session"
reset; MODE="--role"; run
if [ "$RC" -eq 0 ] && [ "$OUT" = "orchestrator" ]; then pass; else fail "rc=$RC out=$OUT"; fi

t "--role prints nothing for another session"
reset; MODE="--role"; CONTEXT="pb-bot/pb-bot@05"; run; is_none

t "the role text states the limits the owner set"
TEXT="$ROOT/plugins/pd-bot/hooks/orchestrator-role.md"
ok=1
for want in 'check, coordinate and decide' 'worker' 'pd-bot:pd-orchestrate' 'push' 'pull request' 'merge' 'PD_BOT_ROLE' '.claude/pd-bot.json'; do
  grep -qF -- "$want" "$TEXT" 2>/dev/null || { ok=0; echo "        missing: $want"; }
done
[ "$ok" -eq 1 ] && pass || fail "orchestrator-role.md lacks a required statement"

t "the role text fits the 10,000 character cap of a hook context with room to spare"
n=$(wc -c < "$TEXT" 2>/dev/null || echo 99999)
if [ "$n" -lt 4000 ]; then pass; else fail "orchestrator-role.md is $n bytes"; fi

t "hooks.json registers session-role.sh on SessionStart once, with no matcher"
if jq -e '[.hooks.SessionStart[] | select(has("matcher") | not) | .hooks[] | .args[]? | select(endswith("hooks/session-role.sh"))] | length == 1' \
  "$ROOT/plugins/pd-bot/hooks/hooks.json" >/dev/null; then pass; else fail "session-role.sh is not registered once without a matcher"; fi

finish
