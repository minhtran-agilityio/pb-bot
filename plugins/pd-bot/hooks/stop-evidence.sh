#!/bin/bash
# Stop hook: "Done needs real command output".
# Blocks the end of a main session when code changed and no command output came after the last edit.
# A model judge picks the severity; a small bash risk floor decides when the judge fails.
# Design: specs/stop-hook-evidence (decisions D1-D4), reworked in #191 (B5, B6, B7).
# Fails open, except on the risk floor.
#
# Input   Stop JSON on stdin: transcript_path, session_id, stop_hook_active, last_assistant_message.
# Output  block = {"decision":"block","reason":"..."} on stdout, exit 0. Pass = no output, exit 0.
# Code    "Code changed" = a file in `git status` (docs, specs and .md/.txt files left out) that differs
#         from the record stop-evidence-baseline.sh made at session start (file dirty at the start and
#         unchanged since: not counted). With no record for the session: every file in `git status`.
# Evidence  a Bash tool result, or the result of a test-runner or qa-verifier agent call, that came AFTER
#         the last successful Edit/Write/MultiEdit/NotebookEdit or the last agent call that may have edited
#         files (any agent but a read-only or test one). A call that runs in the background gives only a
#         launch notice, which is not evidence. Older output does not count.
# Floor   a dirty path under hooks/ or scripts/ (also one dirty before the session started), or an auth or secret word in the added lines. It holds
#         when python3 is missing or broken: the block and the check need only bash, git, sed and grep.
# Bound   at most STOP_EVIDENCE_MAX_BLOCKS blocks (default 3) per transcript and turn, counted in
#         <git dir>/pd-bot/stop-evidence/<transcript name>.blocks; evidence, no code change or a new user
#         turn (stop_hook_active false) resets it. A counter that cannot be written blocks once per turn.
# Judge   $STOP_EVIDENCE_JUDGE_CMD <model> (prompt on stdin) prints {"severity":"pass|light|severe","reason":"..."}.
#         Default: claude -p --model <model>; with no claude on PATH the judge is skipped (no wait).
#         STOP_EVIDENCE_GUARD stops the judge from re-entering this hook.
# Tuning  STOP_EVIDENCE_JUDGE_TIMEOUT seconds (default 90).
set -u

# The judge runs claude -p, which loads this hook again. The guard ends that second run at once.
[ -n "${STOP_EVIDENCE_GUARD:-}" ] && exit 0

INPUT=$(cat)
HOOK_DIR="$(cd "$(dirname "$0")" && pwd)"
PROMPT_FILE="$HOOK_DIR/stop-evidence-judge.md"

REPO_DIR="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null)}"
[ -n "$REPO_DIR" ] || exit 0
# git status and git diff print paths relative to the top of the repository, which is not REPO_DIR
# when the project is a subdirectory of the repository. Every path below is resolved from TOP.
TOP=$(cd "$REPO_DIR" 2>/dev/null && git rev-parse --show-toplevel 2>/dev/null) || TOP="$REPO_DIR"

# python3 reads the transcript and parses the judge's answer. Missing or broken python3 must not open
# the risk floor, so nothing else here needs it.
HAVE_PY=0
python3 -I -c 'import json, sys' >/dev/null 2>&1 && HAVE_PY=1

# --- Stop input -------------------------------------------------------------------------------
# json_str <key>: the first string value of <key> in the input (bash, grep and sed only).
json_str() {
  printf '%s' "$INPUT" | grep -o "\"$1\"[[:space:]]*:[[:space:]]*\"[^\"]*\"" | head -n 1 \
    | sed -e 's/^.*:[[:space:]]*"//' -e 's/"$//'
}
TRANSCRIPT=""; ACTIVE=false; LAST_MSG=""
if [ "$HAVE_PY" = 1 ]; then
  PARSED_IN=$(printf '%s' "$INPUT" | python3 -I -c '
import json, sys
try:
    d = json.load(sys.stdin)
except Exception:
    d = {}
print((d.get("transcript_path") or "") + "\t" + ("true" if d.get("stop_hook_active") else "false"))
' 2>/dev/null) || PARSED_IN=""
  if [ -n "$PARSED_IN" ]; then
    TRANSCRIPT="${PARSED_IN%%$'\t'*}"; ACTIVE="${PARSED_IN##*$'\t'}"
    LAST_MSG=$(printf '%s' "$INPUT" | python3 -I -c '
import json, sys
try:
    print((json.load(sys.stdin).get("last_assistant_message") or "")[:2000])
except Exception:
    pass
' 2>/dev/null) || LAST_MSG=""
  fi
fi
if [ -z "$TRANSCRIPT" ]; then
  TRANSCRIPT=$(json_str transcript_path)
  printf '%s' "$INPUT" | grep -q '"stop_hook_active"[[:space:]]*:[[:space:]]*true' && ACTIVE=true
fi
[ -n "$TRANSCRIPT" ] || TRANSCRIPT="-"
SID=$(json_str session_id | tr -c 'A-Za-z0-9._\n-' '_')

# --- State: block counter and session-start record, both in the git directory --------------------
STATE_DIR=$(git -C "$REPO_DIR" rev-parse --git-path pd-bot/stop-evidence 2>/dev/null) || STATE_DIR=""
case "$STATE_DIR" in "") ;; /*) ;; *) STATE_DIR="$REPO_DIR/$STATE_DIR" ;; esac
BLOCKS_KEY=$(basename "$TRANSCRIPT" .jsonl | tr -c 'A-Za-z0-9._\n-' '_')
[ "$BLOCKS_KEY" = "-" ] && BLOCKS_KEY="${SID:-unknown}"
COUNT_FILE=""
if [ -n "$STATE_DIR" ] && mkdir -p "$STATE_DIR" 2>/dev/null; then
  COUNT_FILE="$STATE_DIR/$BLOCKS_KEY.blocks"
else # no git directory to write to: keep the bound in the temp directory
  _T="${TMPDIR:-/tmp}/pd-bot-stop-evidence-$(id -u 2>/dev/null || echo 0)"
  mkdir -p "$_T" 2>/dev/null && COUNT_FILE="$_T/$(printf '%s' "$REPO_DIR" | cksum | cut -d' ' -f1)-$BLOCKS_KEY.blocks"
fi
MAX_BLOCKS="${STOP_EVIDENCE_MAX_BLOCKS:-3}"
case "$MAX_BLOCKS" in ''|*[!0-9]*) MAX_BLOCKS=3 ;; esac
reset_blocks() { [ -n "$COUNT_FILE" ] && rm -f "$COUNT_FILE" 2>/dev/null; return 0; }
# A stop without stop_hook_active is a new turn of the user: the bound counts the blocks of one turn.
[ "$ACTIVE" = true ] || reset_blocks

# --- Code diff: files changed since session start, minus docs and specs --------------------------
# fingerprint: paths on stdin -> "<hash><TAB><path>" lines (same format as stop-evidence-baseline.sh).
fingerprint() {
  local paths exist gone
  paths=$(cat)
  exist=$(printf '%s\n' "$paths" | while IFS= read -r p; do [ -n "$p" ] && [ -f "$TOP/$p" ] && printf '%s\n' "$p"; done)
  gone=$(printf '%s\n' "$paths" | while IFS= read -r p; do [ -n "$p" ] && [ ! -f "$TOP/$p" ] && printf '%s\n' "$p"; done)
  if [ -n "$exist" ]; then
    paste -d '\t' <(printf '%s\n' "$exist" | git -C "$TOP" hash-object --no-filters --stdin-paths 2>/dev/null) <(printf '%s\n' "$exist")
  fi
  if [ -n "$gone" ]; then printf '%s\n' "$gone" | awk '{print "gone\t" $0}'; fi
}
DIRTY=$(git -C "$TOP" -c core.quotepath=off status --porcelain -uall 2>/dev/null \
  | sed -E 's/^.{3}//; s/^.* -> //; s/^"(.*)"$/\1/')
BASELINE=""
[ -n "$STATE_DIR" ] && [ -n "$SID" ] && [ -f "$STATE_DIR/$SID.baseline" ] && BASELINE="$STATE_DIR/$SID.baseline"
if [ -n "$BASELINE" ] && [ -n "$DIRTY" ]; then
  # keep the paths whose "<hash><TAB><path>" line is not in the record: new, or changed since
  CHANGED=$(printf '%s\n' "$DIRTY" | fingerprint | grep -vxFf "$BASELINE" | sed 's/^[^	]*	//' || true)
else
  CHANGED="$DIRTY"
fi
NOT_CODE='^$|(^|/)(docs|specs)/|\.(md|txt)$'
CODE_FILES=$(printf '%s\n' "$CHANGED" | grep -vE "$NOT_CODE" || true)
ALL_CODE=$(printf '%s\n' "$DIRTY" | grep -vE "$NOT_CODE" || true)
# Risk floor: hooks/ and scripts/ paths, or auth/secret words in added lines. It looks at every dirty code
# file, also one that was dirty before the session started and has not changed since (owner decision, #191):
# the baseline only decides whether code "changed"; a risky file still needs evidence.
compute_floor() {
  FLOOR=0
  if printf '%s\n' "$ALL_CODE" | grep -qE '(^|/)(hooks|scripts)/'; then FLOOR=1; fi
  PATHSPEC=()
  while IFS= read -r f; do PATHSPEC+=("$f"); done <<EOF
$ALL_CODE
EOF
  if [ "$FLOOR" = 0 ]; then
    ADDED=$(GIT_LITERAL_PATHSPECS=1 git -C "$TOP" diff HEAD -- "${PATHSPEC[@]}" 2>/dev/null | grep -E '^\+[^+]' || true)
    if printf '%s\n%s\n' "$ALL_CODE" "$ADDED" | grep -qiE '\b(password|passwd|secret|api[_-]?key|credential|jwt|oauth|auth[a-z]*|login|sign[ -]?in)\b'; then FLOOR=1; fi
  fi
}
if [ -z "$CODE_FILES" ]; then
  # nothing changed since the start; a dirty file on the risk floor still needs evidence
  compute_floor
  if [ "$FLOOR" = 0 ]; then reset_blocks; exit 0; fi
  CODE_FILES="$ALL_CODE"
fi

# --- Transcript: main model and command output after the last edit -------------------------------
# The transcript format is not documented by Claude Code; read it defensively.
MODEL=unknown; EVIDENCE=0; ANALYZED=0
if [ "$HAVE_PY" = 1 ] && [ -r "$TRANSCRIPT" ]; then
  if ANALYSIS=$(python3 -I - "$TRANSCRIPT" <<'PY'
import json, re, sys
EDIT_TOOLS = ("Edit", "Write", "MultiEdit", "NotebookEdit")
AGENT_TOOLS = ("Agent", "Task")
TEST_AGENT = re.compile(r"test-runner|qa-verifier")
# agents that cannot change files; any other agent (implementer, general-purpose, ...) may have edited
READ_ONLY = re.compile(r"(^|:)(test-runner|qa-verifier|Explore|Plan|researcher|intent-reviewer|security-reviewer|task-reviewer|advisor|claude-code-guide|web-fetch)$")
model, seq = "unknown", 0
starts = {}      # tool_use id -> (kind, sequence number of the call)
last_edit = -1   # sequence number of the last edit that succeeded
last_evidence = -1  # sequence number of the latest call whose output counts as evidence
try:
    with open(sys.argv[1], encoding="utf-8", errors="replace") as f:
        for line in f:
            try:
                o = json.loads(line)
            except Exception:
                continue
            m = o.get("message")
            if not isinstance(m, dict):
                continue
            if o.get("type") == "assistant" and m.get("model"):
                model = m["model"]
            c = m.get("content")
            if not isinstance(c, list):
                continue
            for b in c:
                if not isinstance(b, dict):
                    continue
                seq += 1
                if b.get("type") == "tool_use":
                    name, inp = b.get("name"), b.get("input")
                    if name in EDIT_TOOLS:
                        starts[b.get("id")] = ("edit", seq)
                    elif name == "Bash":
                        starts[b.get("id")] = ("bash", seq)
                    elif name in AGENT_TOOLS and isinstance(inp, dict):
                        kind_t = str(inp.get("subagent_type") or "")
                        if inp.get("run_in_background"):
                            kind = "bg"   # the result is a launch notice
                        elif TEST_AGENT.search(kind_t):
                            kind = "agent"
                        elif READ_ONLY.search(kind_t):
                            kind = "readonly"
                        else:
                            kind = "editor"
                        starts[b.get("id")] = (kind, seq)
                    if name == "Bash" and isinstance(inp, dict) and inp.get("run_in_background"):
                        starts[b.get("id")] = ("bg", seq)
                elif b.get("type") == "tool_result":
                    s = starts.get(b.get("tool_use_id"))
                    if not s:
                        continue
                    kind, at = s
                    failed = bool(b.get("is_error"))
                    if kind == "edit" and not failed:
                        last_edit = max(last_edit, at)
                    elif kind == "bash":
                        last_evidence = max(last_evidence, at)
                    elif kind == "agent" and not failed:
                        last_evidence = max(last_evidence, at)
                    elif kind == "editor" and not failed:
                        last_edit = max(last_edit, seq)   # the agent may have edited until its result
except Exception:
    pass
print((model.replace(" ", "_").replace("\t", "_") or "unknown") + "\t" + ("1" if last_evidence > last_edit else "0"))
PY
  ); then
    MODEL="${ANALYSIS%%$'\t'*}"; EVIDENCE="${ANALYSIS##*$'\t'}"; ANALYZED=1
  fi
fi
if [ "$EVIDENCE" = 1 ]; then reset_blocks; exit 0; fi

# --- Risk floor: hooks/ and scripts/ paths, or auth/secret words in added lines ----------------------
compute_floor

# block <reason>: print the block (pure bash, no python3) and stop. At most MAX_BLOCKS per transcript.
block() {
  local n=0 esc
  if [ -n "$COUNT_FILE" ]; then
    n=$(cat "$COUNT_FILE" 2>/dev/null || echo 0)
    case "$n" in ''|*[!0-9]*) n=0 ;; esac
    [ "$n" -ge "$MAX_BLOCKS" ] && exit 0
    # a counter that cannot be written is as good as none: block once, not forever
    if ! { echo $((n + 1)) > "$COUNT_FILE"; } 2>/dev/null && [ "$ACTIVE" = true ]; then exit 0; fi
  elif [ "$ACTIVE" = true ]; then
    exit 0 # nowhere to count: block at most once, so a failing hook cannot loop
  fi
  esc=$(printf '%s' "$1" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g' | tr '\000-\037' ' ')
  printf '{"decision":"block","reason":"%s"}\n' "$esc"
  exit 0
}
LIGHT_REASON="Code changed but no command output came after the last edit. Run the check that proves it works (tests, build or the app) and show its output, or say plainly that you could not run it and why. \"Done\" needs real command output."
FLOOR_REASON="Code changed under hooks/, scripts/ or an auth/secret area, and no command output came after the last edit."

# Transcript not readable or not parsed: the model and the evidence are unknown. Only the risk floor goes on.
if [ "$ANALYZED" = 0 ] && [ "$FLOOR" = 0 ]; then exit 0; fi

# No working python3: nothing can read the transcript or the judge's answer. The risk floor blocks.
if [ "$HAVE_PY" = 0 ]; then
  block "$FLOOR_REASON python3 is missing or broken, so this gate could not read the transcript and blocks. Run the check that proves it works and show its output."
fi

# Opus main without the risk floor: no judge. One light block, then let it end.
case "$MODEL" in
  *opus*)
    if [ "$FLOOR" = 0 ]; then
      [ "$ACTIVE" = true ] && exit 0
      block "$LIGHT_REASON"
    fi ;;
esac

# --- Judge ---------------------------------------------------------------------------------------
JUDGE_MODEL=opus
BUILD_PROMPT() {
  cat "$PROMPT_FILE" 2>/dev/null
  printf '\n\n## Session facts\n- Main model: %s\n- Risk floor matched: %s\n- Command output (Bash tool results, test-runner or qa-verifier agent results) after the last edit: none\n' "$MODEL" "$([ "$FLOOR" = 1 ] && echo yes || echo no)"
  printf -- '- Changed code files:\n%s\n' "$(printf '%s\n' "$CODE_FILES" | sed 's/^/  - /')"
  printf '\n## Diff summary\n%s\n' "$(GIT_LITERAL_PATHSPECS=1 git -C "$TOP" diff HEAD --stat -- "${PATHSPEC[@]}" 2>/dev/null | tail -20)"
  printf '\n## Last assistant message\n%s\n' "$LAST_MSG"
}
default_judge() { STOP_EVIDENCE_GUARD=1 claude -p --model "$1"; }

SEVERITY=""; REASON=""
# No judge command and no claude on PATH: skip the judge. Waiting for a judge that cannot run only delays the stop.
if [ -n "${STOP_EVIDENCE_JUDGE_CMD:-}" ] || command -v claude >/dev/null 2>&1; then
  OUTFILE=$(mktemp 2>/dev/null) || OUTFILE=""
fi
if [ -n "${OUTFILE:-}" ]; then
  trap 'rm -f "$OUTFILE"' EXIT
  (
    if [ -n "${STOP_EVIDENCE_JUDGE_CMD:-}" ]; then
      BUILD_PROMPT | STOP_EVIDENCE_GUARD=1 "$STOP_EVIDENCE_JUDGE_CMD" "$JUDGE_MODEL"
    else
      BUILD_PROMPT | default_judge "$JUDGE_MODEL"
    fi
  ) >"$OUTFILE" 2>/dev/null &
  JPID=$!
  WAITED=0; LIMIT="${STOP_EVIDENCE_JUDGE_TIMEOUT:-90}"
  case "$LIMIT" in ''|*[!0-9]*) LIMIT=90 ;; esac
  while kill -0 "$JPID" 2>/dev/null; do
    if [ "$WAITED" -ge "$LIMIT" ]; then pkill -P "$JPID" 2>/dev/null; kill "$JPID" 2>/dev/null; break; fi
    sleep 1; WAITED=$((WAITED + 1))
  done
  wait "$JPID" 2>/dev/null
  JRC=$?

  if [ "$JRC" = 0 ] && [ "$WAITED" -lt "$LIMIT" ]; then
    PARSED=$(python3 -I - "$OUTFILE" <<'PY'
import json, sys
sev = reason = ""
try:
    lines = [l for l in open(sys.argv[1], encoding="utf-8", errors="replace").read().splitlines() if l.strip()]
    o = json.loads(lines[-1])
    if o.get("severity") in ("pass", "light", "severe"):
        sev, reason = o["severity"], str(o.get("reason", ""))[:500]
except Exception:
    pass
print(sev + "\t" + reason.replace("\t", " ").replace("\n", " "))
PY
) || PARSED=""
    if [ -n "$PARSED" ]; then SEVERITY="${PARSED%%$'\t'*}"; REASON="${PARSED#*$'\t'}"; fi
  fi
fi

case "$SEVERITY" in
  pass)   exit 0 ;;
  light)  [ "$ACTIVE" = true ] && exit 0
          block "$LIGHT_REASON Judge: ${REASON:-no reason given}" ;;
  severe) block "Code changed on a risky area and no command output came after the last edit. Run the check that proves it works and show its output before you finish. Judge: ${REASON:-no reason given}" ;;
  *)      # no judge, judge failed or printed junk: fail closed on the risk floor, otherwise open
          [ "$FLOOR" = 1 ] && block "$FLOOR_REASON No severity judge answered, so this gate blocks. Run the check that proves it works and show its output."
          exit 0 ;;
esac
