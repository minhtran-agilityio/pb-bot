#!/bin/bash
# Contract tests for hooks/merge-guard.sh (ADR 0004: doubt means ask). A fake `gh` and `glab` first on
# PATH answer `pr view` / `mr view` from files and log every call. Outcomes: pass (exit 0, no output),
# ask (exit 0, permissionDecision "ask"), deny (exit 2, reason on stderr).
source "$(dirname "$0")/../lib.sh"

R=$(new_repo merge-guard.sh)
D=$(mktemp -d); _CLEANUP_DIRS+=("$D")
STUB="$D/bin"; mkdir -p "$STUB"; LOG="$D/calls.log"
cat > "$STUB/gh" <<'EOF'
#!/bin/sh
d=$(dirname "$0")/..; echo "gh $*" >> "$d/calls.log"
[ -f "$d/slow" ] && sleep 30
[ "$1 $2" = "pr view" ] && [ -f "$d/base.$3" ] && cat "$d/base.$3" && exit 0
exit 1
EOF
cat > "$STUB/glab" <<'EOF'
#!/bin/sh
d=$(dirname "$0")/..; echo "glab $*" >> "$d/calls.log"
[ "$1 $2" = "mr view" ] && [ -f "$d/base.$3" ] && printf '{"target_branch":"%s"}\n' "$(cat "$d/base.$3")" && exit 0
exit 1
EOF
chmod +x "$STUB/gh" "$STUB/glab"
NOJQ="$D/nojq"; mkdir -p "$NOJQ"
for x in cat grep tr cut head mktemp sleep rm; do ln -s "$(command -v $x)" "$NOJQ/$x"; done
BASH_BIN=$(command -v bash)
NL=$'\n'
printf 'feat/x\n' > "$D/base.2"; printf 'main\n' > "$D/base.1"   # PR 3 has no readable base

# run <command> [tool_name] : run the hook; AGENT=<id> sets agent_id (a subagent); P=<dir> sets PATH
run() {
  local json; json=$(jq -n --arg c "$1" --arg t "${2:-Bash}" --arg a "${AGENT-}" \
    '{tool_name:$t, tool_input:{command:$c}, permission_mode:"default"} + (if $a == "" then {} else {agent_id:$a} end)')
  rm -f "$LOG"
  OUT=$(cd "$R" && printf '%s' "$json" | env PATH="${P:-$STUB:$PATH}" "$BASH_BIN" hooks/merge-guard.sh 2>&1); RC=$?
}
# expect <pass|ask|deny> <command> [tool_name]
expect() {
  t "$1: ${2//$NL/<NL>}${3:+ ($3)}${AGENT:+ (subagent)}"; run "$2" "${3:-}"
  local got=pass
  if [ "$RC" = 2 ]; then got=deny
  elif [ "$RC" = 0 ] && [ -n "$OUT" ] && printf '%s' "$OUT" | jq -e '.hookSpecificOutput.permissionDecision == "ask"' >/dev/null 2>&1; then got=ask
  elif [ "$RC" != 0 ] || [ -n "$OUT" ]; then got="rc=$RC out=$OUT"; fi
  if [ "$got" = "$1" ]; then pass; else fail "want $1, got $got: $OUT"; fi
}

echo "== the artifact table (23 cases) =="
expect pass 'bash scripts/preflight.sh ~/myprojects/x; echo "exit=$?"'
expect pass 'git merge feat/x'
expect pass 'gh pr view 1 --json mergeable'
expect pass 'gh pr list --state merged'
expect pass 'gh pr merge 2 --squash --repo o/r'
expect ask  'gh pr merge 1 --squash --repo o/r'
expect ask  'gh pr merge 3 --repo o/r'
expect ask  'cd x && gh pr merge 1 --repo o/r'
expect ask  'gh api -X PUT repos/o/r/pulls/1/merge'
expect ask  'bash -c "gh pr merge 1"'
expect ask  'GH pr merge 1'
expect ask  'gh pr comment 5 --body "ready to merge"'
expect pass 'glab mr merge 2 --squash'
expect ask  'glab mr merge 1'
expect ask  'glab api -X PUT projects/5/merge_requests/1/merge'
expect pass 'git push -u origin feat/x'
expect pass 'git push origin fix/maintenance'
expect ask  'git push origin HEAD:main'
expect ask  'git push origin fix/1:refs/heads/main'
expect ask  '' mcp__github__merge_pull_request
expect deny 'gh pr merge 1 --auto --squash'
expect deny 'gh pr merge 2 --admin'
AGENT=a1 expect deny 'gh pr merge 1 --repo o/r'

echo "== the merges that ship.js and the fleet run into feat/* (also from a subagent) =="
SHA=0123456789abcdef0123456789abcdef01234567
for a in "" a1; do
  AGENT=$a expect pass 'gh pr merge 2 --repo o/r --merge'
  AGENT=$a expect pass "gh pr merge 2 --repo o/r --merge --match-head-commit $SHA"
  AGENT=$a expect pass 'gh pr merge 2 -R o/r --squash --delete-branch'
done
expect ask  "gh pr merge 1 --repo o/r --merge --match-head-commit $SHA"
t "the base is read for the PR number and repo of the command"; run 'gh pr merge 2 -R o/r --merge'
if grep -q 'gh pr view 2 --repo o/r --json baseRefName' "$LOG"; then pass; else fail "log: $(cat "$LOG" 2>/dev/null)"; fi
t "glab reads the base with the repo of the command"; run 'glab mr merge 2 --repo g/p'
if grep -q 'glab mr view 2 --repo g/p --output json' "$LOG"; then pass; else fail "log: $(cat "$LOG" 2>/dev/null)"; fi

echo "== doubt means ask; a subagent cannot ask, so it is denied =="
expect ask  'gh pr merge 1'
expect ask  'gh pr merge 2  --repo o/r'
expect ask  'gh pr merge 2 --repo=o/r'
expect ask  "gh pr merge 2 --repo o/r${NL}gh pr merge 1 --repo o/r"
expect ask  'gh pr merge https://github.com/o/r/pull/2'
expect ask  'gh api graphql -f query="mutation { mergePullRequest(input: {}) { clientMutationId } }"'
expect ask  'git commit -m "docs: say when gh pr merge asks"'
expect ask  'git push origin main'
expect ask  'git push origin +main'
t "a repeated repo flag reads the base in the last repo, the one gh uses"
run 'gh pr merge 2 --repo o/r --merge -R x/main-repo'
if grep -q 'gh pr view 2 --repo x/main-repo' "$LOG"; then pass; else fail "log: $(cat "$LOG" 2>/dev/null)"; fi
expect ask  'git -C ../x push origin master; echo done'
expect ask  'git push origin main && gh pr create'
expect ask  '' mcp__gitlab__merge_merge_request
expect ask  '' mcp__github__enable_pull_request_auto_merge
AGENT=a1 expect deny 'git commit -m "docs: say when gh pr merge asks"'
AGENT=a1 expect deny 'git push origin HEAD:main'
AGENT=a1 expect deny '' mcp__github__merge_pull_request
AGENT=a1 expect deny 'gh pr merge 3 --repo o/r'

echo "== no false question on ordinary commands =="
for a in "" a1; do  # ship.js commands (ADR 0004): the slug, title and body stay in files, so merge in them is not seen
  AGENT=$a expect pass 'gh pr create --repo o/r --base main --head "$(cat /tmp/h)" --title "$(cat /tmp/t)" --body-file /tmp/b'
  AGENT=$a expect pass 'gh issue comment 5 --repo o/r --body-file /tmp/c'
done
expect pass 'git commit -F /tmp/msg.txt'
expect pass 'git commit -m "push the fix to main"'
expect pass 'git push origin feat/main-x'
expect pass 'git push origin maintenance'
expect pass 'gh pr view 5 --json state,mergeStateStatus,mergedAt'
expect pass 'gh pr create --base main --title "fix: guard" --body-file /tmp/b.md'
expect pass 'git log --oneline --merges -3'
expect pass 'echo merge'

echo "== a stalled or missing tool =="
t "a gh that stalls is cut after 15 s and the merge asks"; touch "$D/slow"; s=$SECONDS
run 'gh pr merge 2 --repo o/r'; rm -f "$D/slow"
if [ "$RC" = 0 ] && printf '%s' "$OUT" | grep -q '"ask"' && [ $((SECONDS - s)) -lt 25 ]; then pass; else fail "rc=$RC $((SECONDS - s))s out=$OUT"; fi
P=$NOJQ expect deny 'gh pr merge 2 --repo o/r'
P=$NOJQ expect deny 'git push origin feat/x'
P=$NOJQ expect pass 'ls -la'

# Probe (opt-in: it starts real `claude -p` sessions, needs a login and costs tokens): what Claude Code does
# with permissionDecision "ask" in each mode. Run with RUN_ASK_PROBE=1. Result (ADR 0004): headless,
# nobody can answer the prompt, so in each mode the Bash call is NOT run.
ask_probe() {
  local mode="$1" pd
  pd=$(mktemp -d); _CLEANUP_DIRS+=("$pd"); mkdir -p "$pd/.claude"
  printf '#!/bin/sh\ncat >/dev/null\necho %s\n' \
    "'{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"ask\",\"permissionDecisionReason\":\"probe\"}}'" > "$pd/hook.sh"
  chmod +x "$pd/hook.sh"
  printf '{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"%s/hook.sh"}]}]}}' "$pd" > "$pd/.claude/settings.json"
  (cd "$pd" && claude -p --model haiku --permission-mode "$mode" --setting-sources project \
    "Run exactly this Bash command and nothing else: touch $pd/ran. Then say DONE." </dev/null >/dev/null 2>&1)
  if [ -f "$pd/ran" ]; then fail "ask was turned into an allow under $mode"; else pass; fi
}
for mode in default auto dontAsk bypassPermissions; do
  t "probe: ask is not turned into an allow under $mode (headless claude -p)"
  if [ "${RUN_ASK_PROBE:-}" != 1 ]; then skip "set RUN_ASK_PROBE=1 to run (starts a real claude session)"
  elif ! command -v claude >/dev/null 2>&1; then skip "claude is not installed"
  else ask_probe "$mode"; fi
done

finish
