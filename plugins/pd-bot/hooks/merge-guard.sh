#!/usr/bin/env bash
# PreToolUse guard (Bash, and MCP tools that merge): no automatic merge into a protected base.
# Goal: an agent never merges into main (or another base that is not feat/*) on its own. The owner is
# asked each time and the merge runs only on the owner's yes. A merge into feat/* needs no question.
# Threat model: an agent that merges out of habit with a normal command. NOT an agent that hides the
# command (escapes, scripts, interpreters, curl with a token); see docs/adr/0004-*.
# Rule: doubt means ask. Anything that looks like a merge and is not a plain `gh pr merge <n>` or
# `glab mr merge <n>` into feat/* is put to the owner. In a subagent nobody can answer, so ask = deny.
# The hook never parses bash. A false ask on prose (`git commit -m "... gh pr merge ..."`): put the
# text in a file (`git commit -F`, `--body-file`).
set -u
payload=$(cat)
command -v jq >/dev/null 2>&1 || case "$payload" in
  *merge*|*push*) echo "merge-guard: BLOCKED. jq is missing, so merges cannot be judged. Install jq." >&2; exit 2 ;;
  *) exit 0 ;; esac
field() { printf '%s' "$payload" | jq -r "$1 // empty" 2>/dev/null; }

deny() { echo "merge-guard: BLOCKED. $1" >&2; exit 2; }
ask() {  # the owner decides; where nobody can answer (a subagent), it is a denial
  [ -z "$(field .agent_id)" ] || deny "$1 A subagent cannot ask the owner; the main session or the owner must run it."
  jq -n --arg r "merge-guard: $1" \
    '{hookSpecificOutput:{hookEventName:"PreToolUse",permissionDecision:"ask",permissionDecisionReason:$r}}'
  exit 0
}

# 1. An MCP merge tool (hooks.json sends only those here): the base is not read, so always ask.
tool=$(field .tool_name)
case "$tool" in mcp__*) ask "The MCP tool '$tool' merges a pull or merge request." ;; esac

cmd=$(field .tool_input.command)
low=$(printf '%s' "$cmd" | tr '[:upper:]' '[:lower:]')

# 2. A git push that names main or master (origin main, HEAD:main, refs/heads/main): ask.
if printf '%s' "$low" | grep -qE '(^|[^a-z0-9_-])git([[:space:]]+-[^[:space:]]+([[:space:]]+[^-[:space:]][^[:space:]]*)?)*[[:space:]]+push[[:space:]].*[[:space:]:/+](main|master)([[:space:];&|)]|$)'; then
  ask "This command pushes to main or master."
fi

# 3. Not a merge: leave at once. `merge` must be a whole word, so mergeable and merged do not count.
printf '%s' "$low" | grep -qE '(^|[^a-z0-9_-])(gh|glab)[[:space:]].*[^a-z]merge([^a-z]|$)|(pulls|merge_requests)/[0-9]+/merge|mergepullrequest' || exit 0

# 4. Never automatic.
case " $low " in *" --auto "*|*" --admin "*) deny "--auto and --admin are not allowed; the owner decides each merge." ;; esac

# 5. A plain `gh pr merge <n> [flags]` or `glab mr merge <n> [flags]`: read the base (at most 15 s).
#    feat/* passes; every other base, and one that cannot be read, is the owner's call.
plain='^(gh pr|glab mr) merge [0-9]+( -[-a-zA-Z]+( [A-Za-z0-9._/-]+)?)*$'
if [[ $cmd =~ $plain ]]; then
  n=$(printf '%s' "$cmd" | cut -d' ' -f4)
  repo=$(printf '%s' "$cmd" | grep -oE -- '(--repo|-R) [^ ]+' | tail -n 1 | cut -d' ' -f2)
  read_base() { case "$cmd" in
    gh*)   gh pr view "$n" ${repo:+--repo "$repo"} --json baseRefName -q .baseRefName ;;
    glab*) glab mr view "$n" ${repo:+--repo "$repo"} --output json | jq -r '.target_branch // empty' ;;
  esac; }
  f=$(mktemp 2>/dev/null) || ask "Merge #$n? (no temp file, so the base was not read)"
  read_base >"$f" 2>/dev/null & p=$!
  (sleep 15 & s=$!; trap 'kill $s' TERM; wait $s; pkill -P "$p"; kill "$p") >/dev/null 2>&1 & w=$!
  wait "$p" 2>/dev/null; kill "$w" 2>/dev/null; base=$(head -n 1 "$f"); rm -f "$f"
  case "$base" in feat/?*) exit 0 ;; esac
  ask "Merge #$n into '${base:-a base that could not be read}'?"
fi

# 6. Any other form (a chain, a pipe, quotes, gh api, glab api, GraphQL, merge in a text): ask.
ask "This command looks like a merge but is not a plain 'gh pr merge <n>' or 'glab mr merge <n>'. Approve only if you mean it."
