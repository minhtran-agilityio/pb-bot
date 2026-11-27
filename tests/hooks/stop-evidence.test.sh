#!/bin/bash
# Contract tests for hooks/stop-evidence.sh (Stop hook: "Done needs real command output").
#
# Contract under test (design: specs/stop-hook-evidence/design.md, decisions D1-D4):
#   stdin   Stop JSON {transcript_path, cwd, stop_hook_active, ...}; CWD = the repo.
#   trigger a code diff (git status; docs and specs files ignored) AND no Bash tool_result in the transcript.
#   judge   `$STOP_EVIDENCE_JUDGE_CMD <model>` (prompt on stdin) prints one JSON line
#           {"severity":"pass|light|severe","reason":"..."}. The test sets it to a stub.
#           Main model from the transcript: sonnet -> judge model "opus"; opus -> no judge unless the risk floor matches.
#   floor   changed path under hooks/ or scripts/ (or a secret/auth word in the diff) = severe floor.
#   output  block = exit 0 + stdout {"decision":"block","reason":"..."}; pass = exit 0, empty stdout.
#   light   blocks once: passes when stop_hook_active is true.   severe: blocks even when active.
#   errors  judge fails or prints junk: light diff passes (fail open); floor diff blocks (fail closed).
#   guard   STOP_EVIDENCE_GUARD set = exit 0 at once (the judge subprocess must not recurse).
#   evidence only a Bash result, or a test-runner agent result, AFTER the last Edit/Write counts (#191 B5).
#   start   hooks/stop-evidence-baseline.sh (SessionStart) records the files dirty at session start;
#           the Stop hook counts only files that differ from that record (#191 B5).
#   floor   holds without a working python3 (#191 B6).
#   bound   at most STOP_EVIDENCE_MAX_BLOCKS (3) blocks per transcript; no judge wait without claude (#191 B7).
source "$(dirname "$0")/../lib.sh"

H=stop-evidence.sh
J=stop-evidence-judge.md
FX="$ROOT/tests/fixtures/stop-evidence"
STUBLOG=$(mktemp); _CLEANUP_DIRS+=("$STUBLOG")

# stub <name> <json-or-FAIL> -> path of a judge stub that logs its model argument
stub() {
  local s; s=$(mktemp); _CLEANUP_DIRS+=("$s")
  if [ "$2" = FAIL ]; then
    printf '#!/bin/bash\necho "$1" >> "%s"\ncat >/dev/null\nexit 1\n' "$STUBLOG" > "$s"
  else
    printf '#!/bin/bash\necho "$1" >> "%s"\ncat >/dev/null\necho '"'%s'"'\n' "$STUBLOG" "$2" > "$s"
  fi
  chmod +x "$s"; echo "$s"
}
# repo_with <relpath> -> repo with a base commit and an unstaged edit to that path
repo_with() {
  local r; r=$(new_repo $H $J)
  stage "$r" "$1" 'v1'; git -C "$r" commit -qm base
  printf 'v2\n' > "$r/$1"
  echo "$r"
}
stop_json() { # stop_json <transcript-file> <active true|false>
  printf '{"session_id":"s","transcript_path":"%s","cwd":".","hook_event_name":"Stop","stop_hook_active":%s}' "$1" "$2"
}
# The hook counts its blocks per transcript, so each run gets its own copy of a fixture; a case
# that needs the same transcript again (the block bound) passes a file it made itself.
fresh() { local f; f=$(mktemp -d)/t-$RANDOM$RANDOM.jsonl; _CLEANUP_DIRS+=("$(dirname "$f")"); cp "$1" "$f" 2>/dev/null; echo "$f"; }
run_stop() { # run_stop <repo> <transcript> <active> <stub> [VAR=val ...]
  local r="$1" tr="$2" act="$3" st="$4"; shift 4
  [ -f "$tr" ] && case "$tr" in "$FX"/*) tr=$(fresh "$tr") ;; esac
  : > "$STUBLOG"
  run_hook "$r" $H "$(stop_json "$tr" "$act")" STOP_EVIDENCE_JUDGE_CMD="$st" "$@"
}
judge_calls() { wc -l < "$STUBLOG" | tr -d ' '; }
assert_pass()  { if [ "$RC" = 0 ] && [ -z "$OUT" ]; then pass; else fail "want exit 0 and no output; rc=$RC out=$(echo "$OUT" | head -3 | tr '\n' ' ')"; fi; }
assert_block() { if [ "$RC" = 0 ] && printf '%s' "$OUT" | grep -qF '"decision":"block"'; then pass; else fail "want exit 0 and a block; rc=$RC out=$(echo "$OUT" | head -3 | tr '\n' ' ')"; fi; }
assert_pass_no_judge() { if [ "$RC" = 0 ] && [ -z "$OUT" ] && [ "$(judge_calls)" = 0 ]; then pass; else fail "rc=$RC out=$OUT judge calls=$(judge_calls)"; fi; }

LIGHT=$(stub light '{"severity":"light","reason":"no test output"}')
SEVERE=$(stub severe '{"severity":"severe","reason":"auth change, nothing ran"}')
PASSV=$(stub pass '{"severity":"pass","reason":"evidence fits"}')
BAD=$(stub bad FAIL)
JUNK=$(stub junk 'not json')

R=$(new_repo $H $J); stage "$R" src/app.py 'v1'; git -C "$R" commit -qm base
t "no diff: passes silently"
run_stop "$R" "$FX/sonnet-no-evidence.jsonl" false "$LIGHT"; assert_pass

R=$(repo_with README.md)
t "docs-only diff: passes silently, judge not called"
run_stop "$R" "$FX/sonnet-no-evidence.jsonl" false "$LIGHT"; assert_pass_no_judge

R=$(repo_with src/app.py)
t "code diff, no output, judge says light: blocks once"
run_stop "$R" "$FX/sonnet-no-evidence.jsonl" false "$LIGHT"; assert_block

t "sonnet main: the judge runs with model opus"
if [ "$(cat "$STUBLOG")" = opus ]; then pass; else fail "judge calls: $(cat "$STUBLOG")"; fi

t "light verdict with stop_hook_active true: passes (blocks only once)"
run_stop "$R" "$FX/sonnet-no-evidence.jsonl" true "$LIGHT"; assert_pass

t "severe verdict blocks even when stop_hook_active is true"
run_stop "$R" "$FX/sonnet-no-evidence.jsonl" true "$SEVERE"; assert_block

t "judge says pass: passes"
run_stop "$R" "$FX/sonnet-no-evidence.jsonl" false "$PASSV"; assert_pass

t "a Bash tool_result in the transcript counts as evidence: passes, judge not called"
run_stop "$R" "$FX/sonnet-with-evidence.jsonl" false "$SEVERE"; assert_pass_no_judge

t "judge exits non-zero on a light diff: fails open"
run_stop "$R" "$FX/sonnet-no-evidence.jsonl" false "$BAD"; assert_pass

t "judge prints junk on a light diff: fails open"
run_stop "$R" "$FX/sonnet-no-evidence.jsonl" false "$JUNK"; assert_pass

t "unreadable transcript on a light diff: fails open"
run_stop "$R" "/nonexistent/t.jsonl" false "$LIGHT"; assert_pass

t "guard env set: exits 0 at once, judge not called"
run_stop "$R" "$FX/sonnet-no-evidence.jsonl" false "$SEVERE" STOP_EVIDENCE_GUARD=1; assert_pass_no_judge

t "opus main, no risk floor: one light block, judge not called"
run_stop "$R" "$FX/opus-no-evidence.jsonl" false "$SEVERE"
if [ "$RC" = 0 ] && printf '%s' "$OUT" | grep -qF '"decision":"block"' && [ "$(judge_calls)" = 0 ]; then pass; else fail "rc=$RC out=$OUT calls=$(judge_calls)"; fi

t "opus main, light block already given (active true): passes"
run_stop "$R" "$FX/opus-no-evidence.jsonl" true "$SEVERE"; assert_pass

F=$(repo_with scripts/deploy.sh)
t "risk floor (scripts/), judge fails: fails closed (blocks)"
run_stop "$F" "$FX/sonnet-no-evidence.jsonl" false "$BAD"; assert_block

t "risk floor (scripts/), judge fails, stop_hook_active true: still blocks"
run_stop "$F" "$FX/sonnet-no-evidence.jsonl" true "$BAD"; assert_block

t "risk floor, opus main: the judge is called with model opus"
run_stop "$F" "$FX/opus-no-evidence.jsonl" false "$PASSV"
if [ "$RC" = 0 ] && [ "$(cat "$STUBLOG")" = opus ]; then pass; else fail "rc=$RC calls: $(cat "$STUBLOG")"; fi

A=$(repo_with src/login.py)
printf 'password = request.get("password")\n' > "$A/src/login.py"
t "auth word in the diff counts as the risk floor: judge fails, blocks"
run_stop "$A" "$FX/sonnet-no-evidence.jsonl" false "$BAD"; assert_block

# --- B5.1: only command output after the last edit counts ---------------------------------------
R=$(repo_with src/app.py)
t "a Bash result before the last edit is not evidence: the judge runs and blocks"
run_stop "$R" "$FX/bash-before-edit.jsonl" false "$LIGHT"
if [ "$RC" = 0 ] && printf '%s' "$OUT" | grep -qF '"decision":"block"' && [ "$(judge_calls)" = 1 ]; then pass; else fail "rc=$RC out=$OUT calls=$(judge_calls)"; fi

t "Bash result, then another edit: the result is stale, blocks"
run_stop "$R" "$FX/bash-then-edit.jsonl" false "$LIGHT"; assert_block

t "a Bash result after the last edit still counts (existing fixture): passes, no judge"
run_stop "$R" "$FX/sonnet-with-evidence.jsonl" false "$SEVERE"; assert_pass_no_judge

# --- B5.2: a delegated test run counts ----------------------------------------------------------
t "a test-runner agent result after the last edit counts as evidence: passes, no judge"
run_stop "$R" "$FX/subagent-test-runner.jsonl" false "$SEVERE"; assert_pass_no_judge

t "an Explore agent result after the edit is not a test run: blocks"
run_stop "$R" "$FX/subagent-explore.jsonl" false "$LIGHT"; assert_block

t "a test-runner agent result before the last edit is stale: blocks"
run_stop "$R" "$FX/subagent-before-edit.jsonl" false "$LIGHT"; assert_block

# --- B5.3: only files changed after session start count -----------------------------------------
B=stop-evidence-baseline.sh
SS_JSON() { printf '{"session_id":"%s","transcript_path":"%s","cwd":".","hook_event_name":"SessionStart","source":"startup"}' "$1" "$2"; }
# session_repo <relpath>: repo whose tree is dirty (tracked edit + untracked file) when the session starts
session_repo() {
  local r; r=$(new_repo $H $J $B)
  stage "$r" "$1" 'v1'; git -C "$r" commit -qm base
  printf 'dirty-before-session\n' > "$r/$1"
  printf 'scratch\n' > "$r/src/scratch.py"
  echo "$r"
}
SESS_T="$FX/sonnet-no-evidence.jsonl"
start_session() { run_hook "$1" $B "$(SS_JSON "$2" "$SESS_T")"; }
stop_session() { # stop_session <repo> <session-id> <stub> -> the transcript is named after the session id
  local d tr; d=$(mktemp -d); _CLEANUP_DIRS+=("$d"); tr="$d/$2.jsonl"; cp "$SESS_T" "$tr"
  : > "$STUBLOG"
  run_hook "$1" $H "$(printf '{"session_id":"%s","transcript_path":"%s","cwd":".","stop_hook_active":false}' "$2" "$tr")" STOP_EVIDENCE_JUDGE_CMD="$3"
}

S=$(session_repo src/app.py)
t "files dirty at session start and unchanged since: a read-only session passes, no judge"
start_session "$S" sess1; stop_session "$S" sess1 "$SEVERE"; assert_pass_no_judge

t "the session start record stays out of git status"
if [ -z "$(git -C "$S" status --porcelain | grep -v '^?? src/scratch.py$' | grep -v '^ M src/app.py$')" ]; then pass; else fail "status: $(git -C "$S" status --porcelain | tr '\n' ' ')"; fi

t "a dirty file edited again after session start counts: blocks"
printf 'edited-in-session\n' > "$S/src/app.py"
stop_session "$S" sess1 "$LIGHT"; assert_block

S=$(session_repo src/app.py)
t "a new untracked file created after session start counts: blocks"
start_session "$S" sess2; printf 'new\n' > "$S/src/new_module.py"
stop_session "$S" sess2 "$LIGHT"; assert_block

S=$(session_repo src/app.py)
t "no record for the session: falls back to git status (the dirty file counts)"
stop_session "$S" sess-unknown "$LIGHT"; assert_block

# Owner decision (#191): the baseline decides only whether code "changed". A dirty file on the risk floor
# still needs evidence, also when it was already dirty at session start and has not changed since.
S=$(session_repo src/app.py)
mkdir -p "$S/scripts"; printf 'echo hi\n' > "$S/scripts/old.sh"
start_session "$S" sess3
t "a hooks/scripts file dirty at session start and unchanged still needs evidence: blocks (judge fails)"
stop_session "$S" sess3 "$BAD"; assert_block

t "the same file with a Bash result after the last edit: passes"
d=$(mktemp -d); _CLEANUP_DIRS+=("$d"); cp "$FX/sonnet-with-evidence.jsonl" "$d/sess3.jsonl"
run_hook "$S" $H "$(printf '{"session_id":"sess3","transcript_path":"%s","cwd":".","stop_hook_active":false}' "$d/sess3.jsonl")" STOP_EVIDENCE_JUDGE_CMD="$BAD"; assert_pass

S=$(session_repo src/app.py)
printf 'password = request.get("password")\n' > "$S/src/app.py"
start_session "$S" sess4
t "an auth-word change dirty at session start and unchanged still needs evidence: blocks"
stop_session "$S" sess4 "$BAD"; assert_block

S=$(session_repo src/app.py)
start_session "$S" sess5
t "a dirty non-floor file at session start and unchanged: still passes without a judge"
stop_session "$S" sess5 "$SEVERE"; assert_pass_no_judge

# --- B6: the risk floor holds without a working python3 -----------------------------------------
# tools_dir [with-python|no-python|broken-python] [with-claude]: a PATH of its own, so a tool can be absent.
tools_dir() {
  local d tool p; d=$(mktemp -d); _CLEANUP_DIRS+=("$d")
  for tool in bash git sed grep awk cat mktemp rm sleep kill pkill wc tr head tail dirname basename cut sort uniq env date find mkdir xargs ls readlink; do
    p=$(command -v "$tool" 2>/dev/null) && ln -s "$p" "$d/$tool"
  done
  case "$1" in
    with-python)   ln -s "$(command -v python3)" "$d/python3" ;;
    broken-python) printf '#!/bin/sh\nexit 1\n' > "$d/python3"; chmod +x "$d/python3" ;;
  esac
  [ "${2:-}" = with-claude ] && printf '#!/bin/sh\ncat >/dev/null; echo %s\n' "'{\"severity\":\"pass\",\"reason\":\"x\"}'" > "$d/claude" && chmod +x "$d/claude"
  echo "$d"
}
F=$(repo_with scripts/deploy.sh); LT=$(repo_with src/app.py)
t "python3 missing, scripts/ changed, no evidence: still blocks"
run_stop "$F" "$FX/sonnet-no-evidence.jsonl" false "$PASSV" PATH="$(tools_dir no-python)"; assert_block

t "python3 missing: the block is valid JSON-ish with a reason"
if printf '%s' "$OUT" | grep -qE '^\{"decision":"block","reason":"[^"]+"\}$'; then pass; else fail "out=$OUT"; fi

t "python3 broken (exits 1), scripts/ changed, no evidence: still blocks"
run_stop "$F" "$FX/sonnet-no-evidence.jsonl" false "$PASSV" PATH="$(tools_dir broken-python)"; assert_block

t "python3 broken, opus main on the risk floor: still blocks"
run_stop "$F" "$FX/opus-no-evidence.jsonl" false "$PASSV" PATH="$(tools_dir broken-python)"; assert_block

t "python3 missing, no risk floor (src/app.py): fails open"
run_stop "$LT" "$FX/sonnet-no-evidence.jsonl" false "$LIGHT" PATH="$(tools_dir no-python)"; assert_pass

t "python3 broken, no risk floor: fails open"
run_stop "$LT" "$FX/sonnet-no-evidence.jsonl" false "$LIGHT" PATH="$(tools_dir broken-python)"; assert_pass

t "python3 missing, no changed files: passes"
R0=$(new_repo $H $J); stage "$R0" src/app.py v1; git -C "$R0" commit -qm base
run_stop "$R0" "$FX/sonnet-no-evidence.jsonl" false "$LIGHT" PATH="$(tools_dir no-python)"; assert_pass

# --- B7: a bounded number of blocks per transcript ----------------------------------------------
loop_run() { # loop_run <repo> <transcript> <stub> <active> [VAR=val ...] -> number of blocks in 8 stops of one turn
  # The first stop of a turn has stop_hook_active false; the stops after a block have it true.
  local r="$1" tr="$2" st="$3" act="$4" n=0 i; shift 4
  for i in 1 2 3 4 5 6 7 8; do
    [ "$i" = 1 ] && act=false || act=true
    run_hook "$r" $H "$(stop_json "$tr" "$act")" STOP_EVIDENCE_JUDGE_CMD="$st" "$@"
    printf '%s' "$OUT" | grep -qF '"decision":"block"' && n=$((n + 1))
  done
  echo "$n"
}
F=$(repo_with scripts/deploy.sh); TR=$(fresh "$FX/sonnet-no-evidence.jsonl")
t "judge keeps failing on the risk floor, stop_hook_active true: blocks at most 3 times"
N=$(loop_run "$F" "$TR" "$BAD" true)
if [ "$N" -ge 1 ] && [ "$N" -le 3 ]; then pass; else fail "blocks in 8 stops: $N"; fi

TR=$(fresh "$FX/sonnet-no-evidence.jsonl")
t "severe verdict every time: blocks at most 3 times"
N=$(loop_run "$F" "$TR" "$SEVERE" true)
if [ "$N" -ge 1 ] && [ "$N" -le 3 ]; then pass; else fail "blocks in 8 stops: $N"; fi

TR=$(fresh "$FX/sonnet-no-evidence.jsonl")
t "STOP_EVIDENCE_MAX_BLOCKS=1: blocks once"
N=$(loop_run "$F" "$TR" "$SEVERE" false STOP_EVIDENCE_MAX_BLOCKS=1)
if [ "$N" = 1 ]; then pass; else fail "blocks in 8 stops: $N"; fi

TR=$(fresh "$FX/sonnet-no-evidence.jsonl")
t "claude missing from PATH on the risk floor: blocks at most 3 times and never waits for a judge"
START=$SECONDS
N=$(loop_run "$F" "$TR" "" false PATH="$(tools_dir with-python)" STOP_EVIDENCE_JUDGE_CMD=)
if [ "$N" -ge 1 ] && [ "$N" -le 3 ] && [ $((SECONDS - START)) -lt 20 ]; then pass; else fail "blocks=$N seconds=$((SECONDS - START))"; fi

t "claude missing, no risk floor: fails open at once"
run_hook "$LT" $H "$(stop_json "$(fresh "$FX/sonnet-no-evidence.jsonl")" false)" PATH="$(tools_dir with-python)" STOP_EVIDENCE_JUDGE_CMD=; assert_pass

TR=$(fresh "$FX/sonnet-no-evidence.jsonl")
t "evidence after a block resets the count: the next missing-evidence stop blocks again"
loop_run "$F" "$TR" "$SEVERE" false >/dev/null
cp "$FX/sonnet-with-evidence.jsonl" "$TR"
run_hook "$F" $H "$(stop_json "$TR" false)" STOP_EVIDENCE_JUDGE_CMD="$SEVERE"
cp "$FX/sonnet-no-evidence.jsonl" "$TR"
run_hook "$F" $H "$(stop_json "$TR" false)" STOP_EVIDENCE_JUDGE_CMD="$SEVERE"; assert_block

# --- security review (#191): a file name must not switch off the auth/secret word scan ----------
X=$(repo_with lib/x.py)
printf 'password = 2\n' > "$X/lib/x.py"
printf 'x\n' > "$X/:(exclude)*.py"
t "an untracked file named like a pathspec magic does not hide a password change (no python3): blocks"
run_stop "$X" "$FX/sonnet-no-evidence.jsonl" false "$PASSV" PATH="$(tools_dir no-python)"; assert_block

t "a non-numeric STOP_EVIDENCE_JUDGE_TIMEOUT falls back to the default instead of hanging"
run_stop "$F" "$FX/sonnet-no-evidence.jsonl" false "$SEVERE" STOP_EVIDENCE_JUDGE_TIMEOUT=abc; assert_block

# --- review round 1 (#191): F1-F5 and the per-turn bound ------------------------------------------
t "F2: an agent that may have edited (implementer) after a Bash result makes it stale: blocks"
R=$(repo_with src/app.py)
run_stop "$R" "$FX/bash-then-implementer.jsonl" false "$LIGHT"; assert_block

t "F2: edit, implementer agent, then a test-runner result: that result counts"
run_stop "$R" "$FX/implementer-then-test-runner.jsonl" false "$SEVERE"; assert_pass_no_judge

t "F3: a background test-runner launch notice is not evidence: blocks"
run_stop "$R" "$FX/background-test-runner.jsonl" false "$LIGHT"; assert_block

t "F4: a counter file that cannot be written does not open the risk floor (no python3): blocks"
F=$(repo_with scripts/deploy.sh); TR=$(fresh "$FX/sonnet-no-evidence.jsonl")
CD=$(git -C "$F" rev-parse --git-path pd-bot/stop-evidence); case "$CD" in /*) ;; *) CD="$F/$CD" ;; esac
mkdir -p "$CD/$(basename "$TR" .jsonl).blocks"
run_hook "$F" $H "$(stop_json "$TR" false)" PATH="$(tools_dir no-python)"; assert_block

t "F4: unwritable counter in a continuation stop (active true): blocks nothing more"
run_hook "$F" $H "$(stop_json "$TR" true)" PATH="$(tools_dir no-python)"; assert_pass

t "F1: project in a subdirectory of the repository: a password change still blocks (python3 missing)"
SUB=$(new_repo $H $J $B); stage "$SUB" app/lib/a.py 'v1'; git -C "$SUB" commit -qm base
printf 'password = 2\n' > "$SUB/app/lib/a.py"
run_stop "$SUB" "$FX/sonnet-no-evidence.jsonl" false "$PASSV" PATH="$(tools_dir no-python)" CLAUDE_PROJECT_DIR="$SUB/app"; assert_block

t "F1: project in a subdirectory: the baseline keeps a dirty file unchanged since the start from counting"
SUB=$(new_repo $H $J $B); stage "$SUB" app/lib/a.py 'v1'; git -C "$SUB" commit -qm base
printf 'dirty-before\n' > "$SUB/app/lib/a.py"
run_hook "$SUB" $B "$(SS_JSON subsess "$SESS_T")" CLAUDE_PROJECT_DIR="$SUB/app"
d=$(mktemp -d); _CLEANUP_DIRS+=("$d"); cp "$SESS_T" "$d/subsess.jsonl"; : > "$STUBLOG"
run_hook "$SUB" $H "$(printf '{"session_id":"subsess","transcript_path":"%s","cwd":".","stop_hook_active":false}' "$d/subsess.jsonl")" STOP_EVIDENCE_JUDGE_CMD="$SEVERE" CLAUDE_PROJECT_DIR="$SUB/app"
assert_pass_no_judge

t "F1: project in a subdirectory: an edit after the start counts"
printf 'edited\n' > "$SUB/app/lib/a.py"
run_hook "$SUB" $H "$(printf '{"session_id":"subsess","transcript_path":"%s","cwd":".","stop_hook_active":false}' "$d/subsess.jsonl")" STOP_EVIDENCE_JUDGE_CMD="$LIGHT" CLAUDE_PROJECT_DIR="$SUB/app"
assert_block

t "per-turn bound: after 3 blocks a new user turn (stop_hook_active false) can block again"
F=$(repo_with scripts/deploy.sh); TR=$(fresh "$FX/sonnet-no-evidence.jsonl")
loop_run "$F" "$TR" "$SEVERE" false >/dev/null
run_hook "$F" $H "$(stop_json "$TR" false)" STOP_EVIDENCE_JUDGE_CMD="$SEVERE"; assert_block

finish
