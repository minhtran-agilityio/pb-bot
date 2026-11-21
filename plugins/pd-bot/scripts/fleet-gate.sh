#!/bin/bash
# Decides whether one ticket pull request of a fleet run may go into its feature branch without the
# owner. It decides only; it never changes the pull request. On a pass it prints the one command the
# orchestrator then runs as its own Bash call, so hooks/merge-guard.sh still sees and judges it.
#
# Three checks, all for the same head commit:
#   evidence  <evidence>/fleet-check.json is from a block-mode run on --head with no mismatch, and
#             its suite check ran and passed (a skipped suite is not enough to go in unattended)
#   ci        every check of the pull request passed (skipped checks do not count as a pass)
#   pr        the pull request is open, not a draft, targets exactly --base (a feat/* branch), its
#             head is --head, it has no label pd-bot:owner-merge, and the base fleet-check ran from
#             is in the history of the feature branch, so the check covered every commit the pull
#             request brings in
# The orchestrator puts the label pd-bot:owner-merge on a pull request the owner must merge, for
# example a fix for a requirement that was not clear; the gate then never passes it.
#
# Trust: --evidence, --head, --pr and --gh-repo come from the orchestrator. fleet-check.json was
# written by fleet-check.sh into the orchestrator's evidence dir; a symlink there is refused. Text
# that comes back from gh or from the JSON file is compared and counted, never executed, and it is
# stripped to printable ASCII and cut to 200 characters before it is printed or written.
#
# Usage:
#   fleet-gate.sh --evidence <dir> --head <40|64 hex> --pr <number> --gh-repo <owner/name>
#                 --base <feat/...> [--wait <seconds>]
# --wait: how long to wait for pending CI checks (default 0: one look). The poll interval is 30 s
# (FLEET_GATE_INTERVAL overrides it, for tests).
# Output: one line per check (`ok|MISMATCH <name>: <detail>`), <evidence>/fleet-gate.json, on a pass
# the line `run: gh pr merge ...`, and the last line `fleet-gate: pass` or
# `fleet-gate: <n> mismatch(es)`.
# Exit: 0 on a pass, 1 on a mismatch, 2 on a usage error. Needs gh and jq.
set -u

usage_text() { echo "usage: fleet-gate.sh --evidence <dir> --head <sha> --pr <number> --gh-repo <owner/name> --base <feat/...> [--wait <seconds>]"; }
usage() { echo "fleet-gate: $*" >&2; usage_text >&2; exit 2; }

lower() { printf '%s' "$1" | LC_ALL=C tr '[:upper:]' '[:lower:]'; }
is_full_hex() {
  case "$1" in *[!0-9a-fA-F]*|'') return 1 ;; esac
  [ "${#1}" -eq 40 ] || [ "${#1}" -eq 64 ]
}
san() { printf '%s' "$1" | LC_ALL=C tr -cd '\040-\176' | cut -c1-200; }

evid=""; head=""; pr=""; gh_repo=""; base_ref_want=""; wait_s=0
while [ $# -gt 0 ]; do
  case "$1" in
    --help|-h) usage_text; exit 0 ;;
    --evidence|--head|--pr|--gh-repo|--base|--wait) [ $# -ge 2 ] || usage "$1 needs a value" ;;
    *) usage "unknown argument: $1" ;;
  esac
  case "$1" in
    --evidence) evid=$2 ;;
    --head) head=$2 ;;
    --pr) pr=$2 ;;
    --gh-repo) gh_repo=$2 ;;
    --base) base_ref_want=$2 ;;
    --wait) wait_s=$2 ;;
  esac
  shift 2
done

[ -n "$evid" ] || usage "--evidence is required"
[ -n "$head" ] || usage "--head is required"
[ -n "$pr" ] || usage "--pr is required"
[ -n "$gh_repo" ] || usage "--gh-repo is required"
[ -n "$base_ref_want" ] || usage "--base is required"
case "$base_ref_want" in
  feat/*[!A-Za-z0-9._/-]*|feat/) usage "--base must be a feat/<name> branch" ;;
  feat/?*) ;;
  *) usage "--base must be a feat/<name> branch: the owner merges every other base" ;;
esac
is_full_hex "$head" || usage "--head must be a full 40 or 64 hex object name"
head=$(lower "$head")
case "$pr" in ''|*[!0-9]*|0*) usage "--pr must be a positive number" ;; esac
case "$wait_s" in ''|*[!0-9]*) usage "--wait must be a number of seconds" ;; esac
case "$gh_repo" in
  */*/*|/*|*/|*[!A-Za-z0-9._/-]*|-*) usage "--gh-repo must be owner/name" ;;
  */*) ;;
  *) usage "--gh-repo must be owner/name" ;;
esac
interval=${FLEET_GATE_INTERVAL:-30}
case "$interval" in ''|*[!0-9]*|0*) interval=30 ;; esac
evid=$(CDPATH='' cd -P -- "$evid" 2>/dev/null && pwd -P) || usage "--evidence is not a directory"

tmpout=""
# shellcheck disable=SC2317,SC2329  # called by the trap
cleanup() { [ -n "$tmpout" ] && rm -f -- "$tmpout"; }
trap cleanup EXIT

mismatches=0
checks_json=""
json_str() {
  local s=$1
  s=${s//\\/\\\\}
  s=${s//\"/\\\"}
  printf '"%s"' "$s"
}
record() { # record <name> <ok|mismatch> <detail>
  local name=$1 res=$2 detail label
  detail=$(san "$3")
  if [ "$res" = ok ]; then label=ok; else label=MISMATCH; mismatches=$((mismatches + 1)); fi
  printf '%s %s: %s\n' "$label" "$name" "$detail"
  [ -n "$checks_json" ] && checks_json="$checks_json,"
  checks_json="$checks_json{\"name\":$(json_str "$name"),\"result\":\"$res\",\"detail\":$(json_str "$detail")}"
}

have_jq=1; command -v jq >/dev/null 2>&1 || have_jq=0
have_gh=1; command -v gh >/dev/null 2>&1 || have_gh=0

FC_BASE=""   # the --base fleet-check ran from; check_pr compares it with the feature branch
# --- 1. evidence --------------------------------------------------------------------------------
check_evidence() {
  local f="$evid/fleet-check.json" out
  if [ -L "$f" ]; then record evidence mismatch "fleet-check.json is a symlink"; return; fi
  if [ ! -f "$f" ]; then record evidence mismatch "fleet-check.json is missing: run fleet-check.sh --mode block first"; return; fi
  if [ "$have_jq" = 0 ]; then record evidence mismatch "jq is not installed, cannot read fleet-check.json"; return; fi
  # One line: <head> <mode> <mismatches> <checks> <checks that are a mismatch or have no known result>
  # <suite checks with result ok> <base>
  out=$(jq -r -s 'if length == 1 and (.[0] | type) == "object" and (.[0].checks | type) == "array" then .[0]
      | "\(.head | if type == "string" then . else "-" end) \(.mode | if type == "string" then . else "-" end) \(.mismatches | if type == "number" then . else "-" end) \(.checks | length) \([.checks[] | select((type != "object") or ((.result == "ok" or .result == "skip") | not))] | length) \([.checks[] | objects | select(.name == "suite" and .result == "ok")] | length) \(.base | if type == "string" and length > 0 then . else "-" end)"
    else "bad" end' "$f" 2>/dev/null) || out=bad
  case "$out" in bad|''|*$'\n'*) record evidence mismatch "fleet-check.json does not parse as one result object"; return ;; esac
  # shellcheck disable=SC2086  # seven space-separated words
  set -- $out
  [ $# -eq 7 ] || { record evidence mismatch "fleet-check.json has an unexpected shape"; return; }
  local probs=""
  [ "$(lower "$1")" = "$head" ] || probs="$probs head '$(san "$1")' is not --head;"
  [ "$2" = block ] || probs="$probs mode is '$(san "$2")', not block;"
  [ "$3" = 0 ] || probs="$probs mismatches is '$(san "$3")', not 0;"
  case "$4" in ''|*[!0-9]*|0) probs="$probs no check was recorded;" ;; esac
  [ "$5" = 0 ] || probs="$probs $(san "$5") check(s) are not ok or skip;"
  [ "$6" = 1 ] || probs="$probs the suite did not run and pass (suite ok $(san "$6") time(s), want 1);"
  if is_full_hex "$7"; then FC_BASE=$(lower "$7"); else probs="$probs base '$(san "$7")' is not a full commit name;"; fi
  if [ -n "$probs" ]; then record evidence mismatch "${probs# }"; else record evidence ok "block mode, head matches, $4 check(s), no mismatch"; fi
}

# --- 2. ci --------------------------------------------------------------------------------------
# ci_look: sets CI_STATE to green, pending, red, none or error, and CI_TEXT.
CI_STATE=""; CI_TEXT=""
ci_look() {
  local out rc counts errf
  errf=$(mktemp "${TMPDIR:-/tmp}/fleet-gate.XXXXXX" 2>/dev/null) || errf=/dev/null
  out=$(gh pr checks "$pr" --repo "$gh_repo" --json name,bucket 2>"$errf"); rc=$?
  # A pull request with no checks yet: gh prints nothing and says so on stderr, or prints [].
  if [ -z "$out" ] && [ "$errf" != /dev/null ] && grep -qi 'no checks reported' "$errf" 2>/dev/null; then
    rm -f -- "$errf"; CI_STATE=none; CI_TEXT="no checks reported yet"; return
  fi
  # An error that a retry cannot fix (wrong repository, no login, an old gh) ends the wait at once.
  if [ -z "$out" ] && [ "$errf" != /dev/null ] && grep -qiE 'could not resolve|not found|HTTP 40[134]|auth login|authentication|unknown flag|unknown command' "$errf" 2>/dev/null; then
    CI_TEXT="gh pr checks failed: $(san "$(head -1 "$errf")")"; rm -f -- "$errf"; CI_STATE=fatal; return
  fi
  [ "$errf" != /dev/null ] && rm -f -- "$errf"
  # gh exits 8 while a check is pending and 1 when one failed; both still print the list.
  counts=$(printf '%s' "$out" | jq -r 'if type == "array" then
      "\([.[] | select(.bucket == "pass")] | length) \([.[] | select(.bucket == "skipping")] | length) \([.[] | select(.bucket == "pending")] | length) \(length)"
    else "bad" end' 2>/dev/null) || counts=bad
  case "$counts" in bad|''|*[!0-9\ ]*) CI_STATE=error; CI_TEXT="gh pr checks gave no readable list (exit $rc)"; return ;; esac
  # shellcheck disable=SC2086  # four space-separated integers
  set -- $counts
  local n_pass=$1 n_skip=$2 n_pend=$3 n_all=$4
  local n_bad=$((n_all - n_pass - n_skip - n_pend))
  CI_TEXT="$n_pass pass, $n_skip skipped, $n_pend pending, $n_bad failed or cancelled"
  if [ "$n_all" = 0 ]; then CI_STATE=none
  elif [ "$n_bad" -gt 0 ]; then CI_STATE=red
  elif [ "$n_pend" -gt 0 ]; then CI_STATE=pending
  elif [ "$n_pass" -gt 0 ]; then CI_STATE=green
  else CI_STATE=none; CI_TEXT="every check was skipped"; fi
}
check_ci() {
  if [ "$have_gh" = 0 ]; then record ci mismatch "gh is not installed"; return; fi
  if [ "$have_jq" = 0 ]; then record ci mismatch "jq is not installed, cannot read the checks"; return; fi
  local waited=0
  ci_look
  # No checks yet counts as pending while there is time left: CI may not have started.
  # A gh error may be a network blip: it is retried too while time is left. A failed check is final.
  while { [ "$CI_STATE" = pending ] || [ "$CI_STATE" = none ] || [ "$CI_STATE" = error ]; } && [ "$waited" -lt "$wait_s" ]; do
    sleep "$interval"
    waited=$((waited + interval))
    ci_look
  done
  case "$CI_STATE" in
    green) record ci ok "$CI_TEXT" ;;
    pending) record ci mismatch "still pending after ${waited}s: $CI_TEXT" ;;
    fatal) record ci mismatch "$CI_TEXT" ;;
    none) record ci mismatch "no check passed: $CI_TEXT" ;;
    *) record ci mismatch "$CI_TEXT" ;;
  esac
}

# --- 3. pr --------------------------------------------------------------------------------------
# Runs last, so a push during the CI wait shows up as a head that is not --head.
check_pr() {
  if [ "$have_gh" = 0 ]; then record pr mismatch "gh is not installed"; return; fi
  if [ "$have_jq" = 0 ]; then record pr mismatch "jq is not installed, cannot read the pull request"; return; fi
  local out line probs=""
  out=$(gh pr view "$pr" --repo "$gh_repo" --json state,isDraft,baseRefName,baseRefOid,headRefOid,labels 2>/dev/null) || { record pr mismatch "gh pr view failed"; return; }
  line=$(printf '%s' "$out" | jq -r 'if type == "object" then
      "\(.state | if type == "string" then . else "-" end)\t\(.isDraft | if type == "boolean" then . else "-" end)\t\(.baseRefName | if type == "string" then . else "-" end)\t\(.baseRefOid | if type == "string" then . else "-" end)\t\(.headRefOid | if type == "string" then . else "-" end)\t\(if (.labels | type) == "array" then ([.labels[] | objects | select(.name == "pd-bot:owner-merge")] | length) else "-" end)"
    else "bad" end' 2>/dev/null) || line=bad
  case "$line" in bad|''|*$'\n'*) record pr mismatch "gh pr view gave no readable object"; return ;; esac
  local state draft base_ref base_oid head_oid owner_label
  IFS=$'\t' read -r state draft base_ref base_oid head_oid owner_label <<EOF
$line
EOF
  [ "$state" = OPEN ] || probs="$probs state is '$(san "$state")', not OPEN;"
  [ "$draft" = false ] || probs="$probs the pull request is a draft;"
  [ "$base_ref" = "$base_ref_want" ] || probs="$probs base '$(san "$base_ref")' is not $(san "$base_ref_want"): the owner merges it;"
  case "$owner_label" in
    0) ;;
    ''|*[!0-9]*) probs="$probs the labels could not be read;" ;;
    *) probs="$probs it has the label pd-bot:owner-merge: the owner merges it;" ;;
  esac
  # The commits fleet-check covered are FC_BASE..head. The pull request brings in every commit of head
  # that the feature branch lacks. Both sets are the same only when FC_BASE is in the feature branch.
  if [ -z "$FC_BASE" ]; then probs="$probs the evidence names no base commit;"
  elif ! is_full_hex "$base_oid"; then probs="$probs the feature branch commit could not be read;"
  else
    local rel
    rel=$(gh api "repos/$gh_repo/compare/$FC_BASE...$(lower "$base_oid")" --jq .status 2>/dev/null) || rel="error"
    case "$rel" in
      identical|ahead) ;;
      *) probs="$probs the checked base $(san "${FC_BASE:0:12}") is not in the history of $(san "$base_ref") (compare: $(san "$rel")): commits outside the checked range would go in;" ;;
    esac
  fi
  [ "$(lower "$head_oid")" = "$head" ] || probs="$probs head '$(san "$head_oid")' is not --head;"
  if [ -n "$probs" ]; then record pr mismatch "${probs# }"; else record pr ok "open, base $(san "$base_ref"), head matches"; fi
}

check_evidence
check_ci
check_pr

write_failed=0
target="$evid/fleet-gate.json"
if [ -L "$target" ] || [ -d "$target" ]; then
  echo "fleet-gate: refusing to write $target (it is a symlink or a directory)" >&2
  write_failed=1
elif tmpout=$(mktemp "$evid/.fleet-gate.XXXXXX" 2>/dev/null) \
  && printf '{"v":1,"head":%s,"pr":%s,"repo":%s,"checks":[%s],"mismatches":%d}\n' \
    "$(json_str "$head")" "$pr" "$(json_str "$gh_repo")" "$checks_json" "$mismatches" > "$tmpout" \
  && chmod 644 "$tmpout" && mv -f -- "$tmpout" "$target"; then
  tmpout=""
else
  echo "fleet-gate: cannot write $target" >&2
  write_failed=1
fi

if [ "$mismatches" -gt 0 ]; then
  echo "fleet-gate: $mismatches mismatch(es)"
  exit 1
fi
if [ "$write_failed" = 1 ]; then
  echo "fleet-gate: cannot write the result file, so the gate does not pass"
  exit 1
fi
echo "run: gh pr merge $pr --repo $gh_repo --merge --match-head-commit $head"
echo "fleet-gate: pass"
exit 0
