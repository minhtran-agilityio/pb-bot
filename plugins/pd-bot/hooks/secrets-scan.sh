#!/usr/bin/env bash
# PreToolUse(Bash): on `git commit`, block when the changes the commit will include, or their
# filenames, look like secrets. The hook runs BEFORE the command, so it works out what the commit
# will hold: the index, plus the `git add ...` earlier in the same command, plus `-a` / `-am` /
# `git commit <paths>`. It builds that in a temporary copy of the index; the real index is never
# touched, and the blobs it writes go to a temporary object directory, not into the repository.
# `git -C <dir>` and `cd <dir>` scan <dir>; a `cd` may not run as the hook assumes (subshell, `||`),
# so the current directory is scanned too.
# Fails open: no jq/git, a bad payload, or any other command passes. Exit 2 blocks.
command -v jq >/dev/null && command -v git >/dev/null || exit 0
cmd=$(jq -r '.tool_input.command // empty' 2>/dev/null) || exit 0
printf '%s' "$cmd" | grep -qE '(^|[;&|("'"'"']|[[:space:]])git[[:space:]]+([^;&|]*[[:space:]])?commit([[:space:]]|$)' || exit 0

export LC_ALL=C
US=$'\037'   # separates the fields of one recorded `git add`
OP=$'\001'   # marks an operator token (; & | ( ) newline) in the token list
RD=$'\002'   # marks a redirection (> < >> 2>&1 <<EOF); the word after it is its target

# Content patterns (case-insensitive). The first five are the retired commit-gate's patterns;
# the token set follows pd-tickets/scripts/publish-check.sh.
# A token must not start inside a longer name (`highs_...`, `task_test_...`): left boundary.
TOKENS='AKIA[0-9A-Z]{16}|(^|[^a-z0-9_])(sk-[a-z0-9_-]{20,}|[rsp]k_(live|test)_[a-z0-9]{16,}|whsec_[a-z0-9]{16,}|AIza[a-z0-9_-]{30,}|gh[pousr]_[a-z0-9]{20,}|github_pat_[a-z0-9_]{20,}|glpat-[a-z0-9_-]{16,}|xox[baprs]-[a-z0-9-]{10,}|eyJ[a-z0-9_-]{8,}\.[a-z0-9_-]{8,}\.)'
PEM='-----BEGIN [A-Z ]*PRIVATE KEY'
AWS='aws_secret_access_key["'"'"']?[[:space:]]*[=:][[:space:]]*["'"'"']?[^[:space:]"'"'"',$]'
OLD='password\s*=\s*["'"'"'][^"'"'"']+|passwd\s*=|api_key\s*=\s*["'"'"'][^"'"'"']+|apikey\s*=\s*["'"'"'][^"'"'"']+|client_secret\s*=\s*["'"'"'][^"'"'"']+|\bsecret\s*=\s*["'"'"'][^"'"'"']+'
PATTERN="($TOKENS|$PEM|$OLD)"
# A value that is a code expression (`os.environ["X"]`, `getenv(...)`) is not a secret.
CODEVAL='=[[:space:]]*[A-Za-z_][A-Za-z0-9_.]*[(\[]'
# Shell or .env style assignment at the start of an added line, with an unquoted value:
# `export API_KEY=abcdefghijkl`. Upper case only, so `client(api_key=x)` is not a match.
ENVPAT='^\+[[:space:]]*(export[[:space:]]+)?[A-Z][A-Z0-9_]*(KEY|SECRET|TOKEN|PASSWORD|PASSWD)=[^[:space:]"'"'"'$#]'

TMPD=
# shellcheck disable=SC2329  # runs from the EXIT trap
cleanup() { [ -n "$TMPD" ] && rm -rf "$TMPD"; }
trap cleanup EXIT

CWD0=$(pwd -P) || exit 0
CUR=$CWD0
ADDS=()          # one record per `git add`: dir US arg US arg ...
COMMIT_SEEN=0

block() { echo "BLOCKED: $1" >&2; exit 2; }

# resolve_dir <base> <arg>: print the physical path of <arg> relative to <base>; fail when unknown.
resolve_dir() {
  local d=$2
  case $d in
    *'$'*|*'`'*) return 2 ;;
    '') return 1 ;;
    \~) d=$HOME ;;
    \~/*) d=$HOME/${d#\~/} ;;
    /*) ;;
    *) d=$1/$d ;;
  esac
  [ -d "$d" ] || return 1
  (cd "$d" 2>/dev/null && pwd -P)
}

# tokenize <string>: shell-like word split into TOK. Quotes and backslashes are honoured;
# operators become their own tokens, prefixed with $OP. Nothing is expanded or run.
tokenize() {
  TOK=()
  local s=$1 i=0 n=${#1} c cur='' has=0 q=''
  while [ "$i" -lt "$n" ]; do
    c=${s:$i:1}
    if [ "$q" = "'" ]; then
      if [ "$c" = "'" ]; then q=''; else cur+=$c; fi
    elif [ "$q" = '"' ]; then
      if [ "$c" = '"' ]; then q=''
      elif [ "$c" = "\\" ] && [ $((i + 1)) -lt "$n" ]; then i=$((i + 1)); cur+=${s:$i:1}
      else cur+=$c; fi
    else
      case $c in
        "'"|'"') q=$c; has=1 ;;
        "\\") i=$((i + 1)); cur+=${s:$i:1}; has=1 ;;
        ' '|$'\t') if [ "$has" = 1 ]; then TOK+=("$cur"); cur=''; has=0; fi ;;
        '>'|'<')
          # a leading file descriptor number belongs to the redirection, not to a word
          if [ "$has" = 1 ]; then
            case $cur in *[!0-9]*) TOK+=("$cur") ;; esac
            cur=''; has=0
          fi
          while [ $((i + 1)) -lt "$n" ]; do
            case ${s:$((i + 1)):1} in '>'|'<'|'&'|'|'|'-') i=$((i + 1)) ;; *) break ;; esac
          done
          TOK+=("$RD") ;;
        $'\n'|';'|'|'|'&'|'('|')')
          if [ "$has" = 1 ]; then TOK+=("$cur"); cur=''; has=0; fi
          TOK+=("$OP$c") ;;
        *) cur+=$c; has=1 ;;
      esac
    fi
    i=$((i + 1))
  done
  if [ "$has" = 1 ]; then TOK+=("$cur"); fi
}

# gitx <dir> <git args>: git in <dir>, on the temporary index with a private object directory when
# there is one. Nothing the repository configures may run: no fsmonitor, no clean/process filter.
gitx() {
  local gd=$1 k n c=()
  shift
  c=(-c core.fsmonitor=false -c core.splitIndex=false)
  while IFS= read -r k; do
    n=${k#filter.}; n=${n%.*}
    c+=(-c "filter.$n.clean=cat" -c "filter.$n.process=cat" -c "filter.$n.required=false")
  done < <(git -C "$gd" config --name-only --get-regexp '^filter\.' 2>/dev/null | grep -E '\.(clean|process)$')
  if [ -n "$IDX" ]; then
    env GIT_INDEX_FILE="$IDX" GIT_OBJECT_DIRECTORY="$TMPD/objects" GIT_ALTERNATE_OBJECT_DIRECTORIES="$OBJS" git -C "$gd" "${c[@]}" "$@"
  else
    git -C "$gd" "${c[@]}" "$@"
  fi
}

# scan <dir> <all:0|1> <path>...: block when what a commit in <dir> would hold looks like a secret.
scan() {
  local dir=$1 all=$2 rec real a top need=0
  shift 2
  local paths=("$@")
  IDX=''; OBJS=''
  top=$(git -C "$dir" rev-parse --show-toplevel 2>/dev/null) || return 0
  [ "$all" = 1 ] && need=1
  [ "${#paths[@]}" -gt 0 ] && need=1
  for rec in "${ADDS[@]}"; do [ "${rec%%"$US"*}" = "$top" ] && need=1; done
  if [ "$need" = 1 ]; then
    [ -n "$TMPD" ] || TMPD=$(mktemp -d 2>/dev/null) || TMPD=''
    if [ -n "$TMPD" ]; then
      IDX=$TMPD/index.$RANDOM$RANDOM
      real=$(git -C "$dir" rev-parse --git-path index 2>/dev/null)
      case $real in /*) ;; *) real=$dir/$real ;; esac
      [ -f "$real" ] && cp "$real" "$IDX"
      OBJS=$(git -C "$dir" rev-parse --git-path objects 2>/dev/null)
      case $OBJS in /*) ;; *) OBJS=$dir/$OBJS ;; esac
      mkdir -p "$TMPD/objects"
      for rec in "${ADDS[@]}"; do
        [ "${rec%%"$US"*}" = "$top" ] || continue
        IFS=$US read -r -a a <<<"$rec"
        gitx "${a[1]}" add "${a[@]:2}" >/dev/null 2>&1 || gitx "${a[1]}" add -A >/dev/null 2>&1
      done
      if [ "$all" = 1 ]; then
        gitx "$dir" add -u >/dev/null 2>&1
      elif [ "${#paths[@]}" -gt 0 ]; then
        gitx "$dir" add -u -- "${paths[@]}" >/dev/null 2>&1 || gitx "$dir" add -u >/dev/null 2>&1
      fi
    fi
  fi
  local diff
  diff=$(gitx "$dir" diff --cached --no-textconv --no-ext-diff -U0 -- ':!*tests/*' ':!*__tests__/*' ':!*.example' ':!*.md' ':!docs/' 2>/dev/null)
  if printf '%s\n' "$diff" | grep -qEi -e "$PATTERN" ||
     printf '%s\n' "$diff" | grep -Ei -e "$AWS" | grep -qvEi -e "$CODEVAL" ||
     printf '%s\n' "$diff" | grep -E -e "$ENVPAT" | grep -qvE -e "$CODEVAL"; then
    block "potential secret in the changes this commit would include. Run 'git diff HEAD', move the value to an env var, re-stage. If it is a fake test value, ask the owner."
  fi
  local envfile
  envfile=$(gitx "$dir" -c core.quotepath=off diff --cached --no-textconv --no-ext-diff --name-only 2>/dev/null | grep -E '(^|/)([^/]*\.env|\.env\.[^/]*)$' | grep -vE '(^|/)[^/]*\.env\.example$' | head -n 1)
  if [ -n "$envfile" ]; then
    block "a .env file is staged ($envfile). Unstage it (git restore --staged <file>), gitignore it, and commit a .env.example instead."
  fi
  return 0
}

# handle_add <dir> <arg>...: record a `git add`. A flag outside the known set (-p, -i, ...) means
# "stage everything" for the scan, which can only over-block.
handle_add() {
  local dir=$1 rec a rest ok=1 endopts=0 j ch top
  shift
  top=$(git -C "$dir" rev-parse --show-toplevel 2>/dev/null) || return 0
  rec="$top$US$dir"
  for a in "$@"; do
    if [ "$endopts" = 0 ]; then
      case $a in
        --) endopts=1 ;;
        --all|--update|--force|--intent-to-add|--verbose|--dry-run) ;;
        --*) ok=0 ;;
        -?*) rest=${a#-}
             for ((j = 0; j < ${#rest}; j++)); do
               ch=${rest:$j:1}
               case $ch in A|u|f|N|v|n) ;; *) ok=0 ;; esac
             done ;;
      esac
    fi
    rec+="$US$a"
  done
  [ "$ok" = 1 ] || rec="$top$US$dir${US}-A"
  ADDS+=("$rec")
}

# handle_commit <dir> <arg>...: work out -a and the pathspecs, then scan.
handle_commit() {
  local dir=$1 all=0 a rest j ch skip=0 endopts=0 paths=()
  shift
  for a in "$@"; do
    if [ "$skip" = 1 ]; then skip=0; continue; fi
    if [ "$endopts" = 1 ]; then paths+=("$a"); continue; fi
    case $a in
      --) endopts=1 ;;
      --all) all=1 ;;
      --pathspec-from-file) all=1; skip=1 ;;
      --pathspec-from-file=*) all=1 ;;
      --message|--file|--author|--date|--reuse-message|--reedit-message|--template|--fixup|--squash|--cleanup|--trailer) skip=1 ;;
      --*) ;;
      -?*) rest=${a#-}
           for ((j = 0; j < ${#rest}; j++)); do
             ch=${rest:$j:1}
             case $ch in
               a) all=1 ;;
               m|F|C|c|t) [ $((j + 1)) -ge ${#rest} ] && skip=1; break ;;
             esac
           done ;;
      *) paths+=("$a") ;;
    esac
  done
  COMMIT_SEEN=1
  scan "$dir" "$all" "${paths[@]}"
}

# handle_segment: SEG holds the words of one simple command.
handle_segment() {
  local n=${#SEG[@]} i=0 dir=$CUR j sub d rc explicit=0
  # Skip what can stand before the command: VAR=x, `env`, `command`, `sudo`, `time`, `nohup`, `exec`,
  # `nice`, `!`, and the shell keywords `{ if then elif else do while until`.
  while [ "$i" -lt "$n" ]; do
    case ${SEG[$i]} in
      env|command|sudo|time|nohup|exec|nice|'!'|'{'|'}'|if|then|elif|else|do|while|until)
        i=$((i + 1))
        while [ "$i" -lt "$n" ] && { [[ ${SEG[$i]} =~ ^- ]] || [[ ${SEG[$i]} =~ ^[A-Za-z_][A-Za-z0-9_]*= ]]; }; do i=$((i + 1)); done ;;
      *=*) if [[ ${SEG[$i]} =~ ^[A-Za-z_][A-Za-z0-9_]*= ]]; then i=$((i + 1)); else break; fi ;;
      *) break ;;
    esac
  done
  [ "$i" -lt "$n" ] || return 0
  case ${SEG[$i]} in
    xargs)
      # `... | xargs git add`: the paths are unknown, so scan as if everything were added
      for ((j = i + 1; j + 1 < n; j++)); do
        if [ "${SEG[$j]}" = git ]; then
          case ${SEG[$((j + 1))]} in add|stage) handle_add "$dir" -A ;; esac
        fi
      done ;;
    cd)
      if [ $((i + 2)) -eq "$n" ] && d=$(resolve_dir "$CUR" "${SEG[$((i + 1))]}"); then CUR=$d; fi ;;
    git)
      j=$((i + 1))
      while [ "$j" -lt "$n" ]; do
        case ${SEG[$j]} in
          -C) j=$((j + 1)); explicit=1
              d=$(resolve_dir "$dir" "${SEG[$j]:-}"); rc=$?
              if [ "$rc" = 0 ]; then dir=$d; elif [ "$rc" = 2 ]; then dir=$CWD0; else dir=''; fi ;;
          -c|--git-dir|--work-tree|--namespace|--exec-path|--super-prefix|--config-env) j=$((j + 1)) ;;
          -*) ;;
          *) break ;;
        esac
        j=$((j + 1))
      done
      sub=${SEG[$j]:-}
      if [ -z "$dir" ]; then   # -C names a directory that does not exist: git itself fails
        [ "$sub" = commit ] && COMMIT_SEEN=1
        return 0
      fi
      case $sub in
        add|stage) handle_add "$dir" "${SEG[@]:$((j + 1))}" ;;
        commit)
          handle_commit "$dir" "${SEG[@]:$((j + 1))}"
          # the cd tracking is a guess (subshell, ||, cd -): also scan where the hook started
          [ "$explicit" = 0 ] && [ "$dir" != "$CWD0" ] && handle_commit "$CWD0" "${SEG[@]:$((j + 1))}" ;;
      esac ;;
  esac
  return 0
}

tokenize "$cmd"
SEG=()
skipnext=0
for tok in "${TOK[@]}" "${OP};"; do
  if [ "$skipnext" = 1 ]; then
    skipnext=0
    [ "${tok#"$OP"}" = "$tok" ] && [ "$tok" != "$RD" ] && continue
  fi
  if [ "$tok" = "$RD" ]; then
    skipnext=1
  elif [ "${tok#"$OP"}" != "$tok" ]; then
    [ "${#SEG[@]}" -gt 0 ] && handle_segment
    SEG=()
  else
    SEG+=("$tok")
  fi
done

# The line above matched a commit that the parser did not see (a wrapper, a form it does not read):
# scan the index of the current directory, as before.
if [ "$COMMIT_SEEN" != 1 ]; then
  # `-a` anywhere in that text stages tracked changes: scan them too (over-blocks at worst)
  if printf '%s' "$cmd" | grep -qE '(^|[[:space:]])(-[a-zA-Z]*a[a-zA-Z]*|--all)([[:space:]]|$)'; then scan "$CWD0" 1; else scan "$CWD0" 0; fi
fi
exit 0
