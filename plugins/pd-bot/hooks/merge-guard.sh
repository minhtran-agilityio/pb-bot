#!/usr/bin/env bash
# PreToolUse guard (Bash, and MCP tools that merge): no automatic merge into a protected base.
# Goal: an agent never merges into main (or another base that is not feat/*) on its own. The owner is
# asked each time and the merge runs only on the owner's yes. A merge into feat/* needs no question.
# Threat model: an agent that merges out of habit with a normal command. NOT an agent that hides the
# command (escapes, scripts, interpreters, curl with a token); see docs/adr/0004-*.
# Rule: doubt means ask. Anything that looks like a merge and is not a plain `gh pr merge <n>` or
# `glab mr merge <n>` into feat/* is put to the owner. In a subagent nobody can answer, so ask = deny.
# The hook reads bash only as far as git push / checkout / switch words, never a merge command. A false ask on prose (`git commit -m "... gh pr merge ..."`): put the
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

# 2. A git push that reaches main or master: ask. Quotes are dropped first ("main", 'main').
#    a. Named: origin main, HEAD:main, refs/heads/main (one word of the push, so text in another command is not read).   b. --all and --mirror push every branch.
#    c. No branch named (`git push`, `git push origin`, `git push origin HEAD`): the current branch and its
#       upstream decide, read from the repository of the command (payload cwd, or git -C). A checkout of
#       main earlier in the same command counts. Nothing to resolve (no repository, detached HEAD): pass.
q=$(printf '%s' "$cmd" | tr -d "\"'")  # case kept: a path after -C is case-sensitive
is_main() { case "$1" in [Mm][Aa][Ii][Nn]|[Mm][Aa][Ss][Tt][Ee][Rr]) return 0 ;; esac; return 1; }
abs_dir() {  # <dir or ""> : the directory of a git -C or cd, as a path ("~" expanded, relative to the command's cwd)
  case "$1" in "") echo "${cwd:-$PWD}" ;; \~|\~/*) echo "$HOME${1#\~}" ;; /*) echo "$1" ;; *) echo "${cwd:-$PWD}/$1" ;; esac
}
resolves_to_main() {  # <dir> <push.default> <cur-branch or ""> <remote or ""> <refspecs from -c>; 0 = a push without a branch reaches main
  local d up r pd="$2" cur="$3" remote="$4" cfgr="$5"; d=$(abs_dir "$1")
  [ -n "$cur" ] || cur=$(git -C "$d" symbolic-ref --short -q HEAD 2>/dev/null) || return 1
  is_main "$cur" && return 0
  [ -n "$pd" ] || pd=$(git -C "$d" config push.default 2>/dev/null)
  [ "$pd" != matching ] || return 0  # every branch that exists on both sides, main included
  up=$(git -C "$d" config "branch.$cur.merge" 2>/dev/null); is_main "${up#refs/heads/}" && return 0
  [ -n "$remote" ] || remote=$(git -C "$d" config "branch.$cur.pushRemote" 2>/dev/null) \
    || remote=$(git -C "$d" config remote.pushDefault 2>/dev/null) || remote=$(git -C "$d" config "branch.$cur.remote" 2>/dev/null)
  while IFS= read -r r; do  # a configured push refspec (config file, or -c on the command)
    case "$r" in "") continue ;; :|*\**) return 0 ;; esac; is_main "${r##*[:/+]}" && return 0
  done <<<"$cfgr"$'\n'"$(git -C "$d" config --get-all "remote.${remote:-origin}.push" 2>/dev/null)"
  return 1
}
cwd=$(field .cwd); moved=""; tomain=0; nl=$'\n'; sep='[;&|()`]'  # one command per line
while IFS= read -r seg; do
  read -ra tk <<<"$seg"; i=0; n=${#tk[@]}
  case "${tk[0]:-}" in cd|pushd) [ -z "${tk[1]:-}" ] || [ "${tk[1]}" = - ] || cwd=$(abs_dir "${tk[1]}"); continue ;; esac  # a later push runs there
  while [ "$i" -lt "$n" ] && ! [[ ${tk[i]##*/} =~ ^[Gg][Ii][Tt]$ ]]; do i=$((i+1)); done
  [ "$i" -lt "$n" ] || continue
  i=$((i+1)); dir=""; pd=""; cfgr=""
  while [ "$i" -lt "$n" ]; do case "${tk[i]}" in
    -C) dir="${tk[i+1]:-}"; i=$((i+2)) ;;
    -c) case "${tk[i+1]:-}" in push.default=*) pd="${tk[i+1]#push.default=}" ;; remote.*.push=*) cfgr="$cfgr${tk[i+1]#*=}"$'\n' ;; esac; i=$((i+2)) ;;
    --git-dir|--work-tree|--namespace|--exec-path) i=$((i+2)) ;;
    -*) i=$((i+1)) ;;
    *) break ;; esac
  done
  case "${tk[i]:-}" in
    checkout|switch)  # which branch is current after it: main counts as a push to main
      br=""; track=0; new=0; j=$((i+1))
      while [ "$j" -lt "$n" ]; do case "${tk[j]}" in
        --) break ;;  # what follows is a file, not a branch
        -b|-B|-c|-C|--orphan) br="${tk[j+1]:-}"; new=1; break ;;
        -t|--track|--track=*) track=1 ;;
        -*) ;;
        *) br="${tk[j]}"; break ;; esac; j=$((j+1))
      done
      [ "$track" = 0 ] || br="${br##*/}"  # --track origin/main makes a local main
      if [ -z "$br" ]; then :
      elif is_main "$br"; then tomain=1
      elif [ "$new" = 1 ] || git -C "$(abs_dir "$dir")" show-ref --verify -q "refs/heads/$br" 2>/dev/null; then moved="$br"; tomain=0
      fi ;;  # anything else (a file, a tag, a commit) leaves the current branch as it is
    push)
      pos=0; remote=""; head=0; tags=0; j=$((i+1))
      while [ "$j" -lt "$n" ]; do a="${tk[j]#+}"
        case "$a" in "#"*) break ;; esac  # a comment ends the command
        case "$a" in *[\<\>]*)  # a redirection is no argument: 2>&1 (split at the &), > file, >file, main>file
          a="${a%%[<>]*}"; case "$a" in *[!0-9]*) ;; *) case "${tk[j]}" in *[\<\>]) j=$((j+1)) ;; esac; j=$((j+1)); continue ;; esac ;;
        esac
        case "$a" in
        --all|--mirror|--branches) ask "git push $a pushes every branch, main included." ;;
        --tags) tags=1 ;;
        -o|--push-option|--repo|--receive-pack|--exec) j=$((j+1)) ;;
        --recurse-submodules) case "${tk[j+1]:-}" in check|on-demand|only|no) j=$((j+1)) ;; esac ;;
        -*) ;;
        :|*\**) ask "git push $a pushes every matching branch, main included." ;;
        *) is_main "$a" && ask "This command pushes to main or master."
           is_main "${a##*[:/]}" && ask "This command pushes to main or master."
           pos=$((pos+1)); [ "$pos" -ne 1 ] || remote="$a"; case "$a" in [Hh][Ee][Aa][Dd]|@) [ "$pos" -lt 2 ] || head=1 ;; esac ;;
        esac; j=$((j+1))
      done
      [ "$pos" -ge 2 ] && [ "$head" = 0 ] && continue  # a branch is named, and it is not main (2a)
      [ "$pos" -ge 2 ] || [ "$tags" = 0 ] || continue   # --tags alone pushes no branch
      [ "$tomain" = 0 ] || ask "This command checks out main or master, then pushes."
      if [ "$pos" -ge 2 ]; then  # HEAD: the current branch itself; no refspec: its upstream and the config too
        cur="$moved"; [ -n "$cur" ] || cur=$(git -C "$(abs_dir "$dir")" symbolic-ref --short -q HEAD 2>/dev/null)
        is_main "$cur" && ask "This command pushes the current branch, '$cur'."
      else
        resolves_to_main "$dir" "$pd" "$moved" "$remote" "$cfgr" && ask "This push names no branch, and the current branch or its upstream is main or master."
      fi ;;
  esac
done <<<"${q//$sep/$nl}"

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
