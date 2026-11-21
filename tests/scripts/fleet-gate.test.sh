#!/bin/bash
# plugins/pd-bot/scripts/fleet-gate.sh decides whether one ticket pull request may go into its
# feat/* branch without the owner: block-mode evidence for the head, green CI, an open pull request
# on feat/* at that head. Every case uses its own evidence dir and a fake gh on PATH, so no case
# touches the network or a real pull request.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
FG="$ROOT/plugins/pd-bot/scripts/fleet-gate.sh"

if ! command -v jq >/dev/null 2>&1; then
  t "fleet-gate needs jq"; skip "jq not installed"; finish
fi

HEAD=0123456789abcdef0123456789abcdef01234567
OTHER=89abcdef0123456789abcdef0123456789abcdef
FBASE=fedcba9876543210fedcba9876543210fedcba98
TIP=1111111111111111111111111111111111111111

# The fake gh answers `pr view` from $FAKE/view.json and `pr checks` from $FAKE/checks.<n>.json,
# where <n> is the number of the call (the last file is used again), and logs every call.
new_case() {
  EV=$(mktemp -d); FAKE=$(mktemp -d); BIN=$(mktemp -d)
  _CLEANUP_DIRS+=("$EV" "$FAKE" "$BIN")
  cat > "$BIN/gh" <<'EOF'
#!/bin/bash
printf '%s\n' "$*" >> "$FAKE/calls"
case "$1 $2" in
  "pr view") [ -f "$FAKE/view.json" ] || exit 1; cat "$FAKE/view.json" ;;
  "api repos/o/r/compare/"*) cat "$FAKE/compare.txt" 2>/dev/null || echo ahead ;;
  "pr checks")
    n=$(( $(cat "$FAKE/n" 2>/dev/null || echo 0) + 1 )); printf '%s' "$n" > "$FAKE/n"
    f="$FAKE/checks.$n.json"
    while [ ! -f "$f" ] && [ "$n" -gt 1 ]; do n=$((n - 1)); f="$FAKE/checks.$n.json"; done
    [ -f "$f" ] || exit 1
    if grep -q NOCHECKS "$f"; then echo "no checks reported on the 'x' branch" >&2; exit 1; fi
    if grep -q NETERR "$f"; then echo "connection refused" >&2; exit 1; fi
    if grep -q NOREPO "$f"; then echo "GraphQL: Could not resolve to a Repository with the name 'o/r'." >&2; exit 1; fi
    cat "$f"
    if grep -q '"pending"' "$f"; then exit 8; fi
    if grep -q '"fail"' "$f"; then exit 1; fi ;;
  *) exit 1 ;;
esac
EOF
  chmod +x "$BIN/gh"
  # sleep only logs its argument, so the --wait cases run fast and the interval is visible.
  printf '#!/bin/bash\nprintf "%%s\\n" "$1" >> "$FAKE/sleeps"\n' > "$BIN/sleep"; chmod +x "$BIN/sleep"
  write_check "$HEAD"; write_view; write_checks 1 pass pass
}
TWO_CHECKS='[{"name":"receipt","result":"ok","detail":"x"},{"name":"acceptance","result":"skip","detail":"x"},{"name":"suite","result":"ok","detail":"exit 0"}]'
write_check() { # write_check <head> [mode] [mismatches] [checks-json]
  printf '{"v":1,"head":"%s","base":"%s","mode":"%s","checks":%s,"mismatches":%s}\n' "$1" "${5-$FBASE}" "${2:-block}" "${4:-$TWO_CHECKS}" "${3:-0}" > "$EV/fleet-check.json"
}
write_view() { # write_view [state] [base] [head] [isDraft] [labels-json]
  printf '{"state":"%s","isDraft":%s,"baseRefName":"%s","baseRefOid":"%s","headRefOid":"%s","labels":%s}\n' "${1:-OPEN}" "${4:-false}" "${2:-feat/big-thing}" "$TIP" "${3:-$HEAD}" "${5:-[]}" > "$FAKE/view.json"
}
write_checks() { # write_checks <call number> <bucket>...
  local n=$1 out="" b; shift
  for b in "$@"; do out="$out${out:+,}{\"name\":\"c\",\"bucket\":\"$b\"}"; done
  printf '[%s]\n' "$out" > "$FAKE/checks.$n.json"
}
# fg <extra args...>: run the script; sets OUT and RC.
fg() {
  OUT=$(FAKE="$FAKE" PATH="$BIN:$PATH" FLEET_GATE_INTERVAL=1 bash "$FG" --evidence "$EV" --head "$HEAD" --pr 12 --gh-repo o/r --base feat/big-thing "$@" 2>&1); RC=$?
}
has() { if printf '%s\n' "$OUT" | grep -qF -- "$1"; then pass; else fail "missing '$1' in: $(printf '%s' "$OUT" | tr '\n' '|')"; fi; }
hasnt() { if printf '%s\n' "$OUT" | grep -qF -- "$1"; then fail "unexpected '$1' in: $(printf '%s' "$OUT" | tr '\n' '|')"; else pass; fi; }
rc_is() { if [ "$RC" -eq "$1" ]; then pass; else fail "rc=$RC, want $1 — $(printf '%s' "$OUT" | tr '\n' '|')"; fi; }

# --- pass ---------------------------------------------------------------------------------------
new_case
t "a pass: exit 0, three ok lines, the command and the last line"; fg
if [ "$RC" = 0 ] && ! printf '%s\n' "$OUT" | grep -q '^MISMATCH' \
  && printf '%s\n' "$OUT" | grep -qF 'ok evidence: block mode, head matches, 3 check(s), no mismatch' \
  && printf '%s\n' "$OUT" | grep -qF 'ok ci: 2 pass, 0 skipped, 0 pending, 0 failed or cancelled' \
  && printf '%s\n' "$OUT" | grep -qF 'ok pr: open, base feat/big-thing, head matches' \
  && printf '%s\n' "$OUT" | grep -qFx "run: gh pr merge 12 --repo o/r --merge --match-head-commit $HEAD" \
  && [ "$(printf '%s\n' "$OUT" | tail -1)" = 'fleet-gate: pass' ]; then pass
else fail "rc=$RC out: $(printf '%s' "$OUT" | tr '\n' '|')"; fi
t "a pass writes fleet-gate.json"
if [ "$(jq -r '.mismatches' "$EV/fleet-gate.json")" = 0 ] && [ "$(jq -r '.head' "$EV/fleet-gate.json")" = "$HEAD" ] \
  && [ "$(jq -r '.pr' "$EV/fleet-gate.json")" = 12 ] && [ "$(jq -r '.repo' "$EV/fleet-gate.json")" = o/r ] \
  && [ "$(jq -r '.checks | length' "$EV/fleet-gate.json")" = 3 ]; then pass
else fail "json: $(cat "$EV/fleet-gate.json" 2>&1)"; fi
t "the script only reads the pull request: it never calls gh pr merge"
if grep -q 'merge' "$FAKE/calls"; then fail "calls: $(tr '\n' '|' < "$FAKE/calls")"; else pass; fi
t "an uppercase --head and an uppercase head from gh still match"
write_view OPEN feat/big-thing "$(printf '%s' "$HEAD" | tr 'a-f' 'A-F')"
OUT=$(FAKE="$FAKE" PATH="$BIN:$PATH" bash "$FG" --evidence "$EV" --head "$(printf '%s' "$HEAD" | tr 'a-f' 'A-F')" --pr 12 --gh-repo o/r --base feat/big-thing 2>&1); RC=$?; rc_is 0
t "skipped checks next to a pass still pass"
new_case; write_checks 1 pass skipping; fg; rc_is 0

# --- evidence -----------------------------------------------------------------------------------
new_case
t "no fleet-check.json is a MISMATCH and exit 1"; rm -f "$EV/fleet-check.json"; fg
has 'MISMATCH evidence: fleet-check.json is missing'
t "... and no command is printed"; hasnt 'run: gh'
t "... and the exit code is 1"; rc_is 1
t "... and the last line counts the mismatches"
if [ "$(printf '%s\n' "$OUT" | tail -1)" = 'fleet-gate: 2 mismatch(es)' ]; then pass; else fail "$(printf '%s\n' "$OUT" | tail -1)"; fi
t "a warn-mode result is a MISMATCH"; write_check "$HEAD" warn; fg; has "MISMATCH evidence: mode is 'warn', not block"
t "a result for another head is a MISMATCH"; write_check "$OTHER"; fg; has "is not --head"
t "a result with mismatches is a MISMATCH"; write_check "$HEAD" block 2; fg; has "mismatches is '2', not 0"
t "a mismatch check with a mismatches count of 0 is still a MISMATCH"
write_check "$HEAD" block 0 '[{"name":"suite","result":"mismatch","detail":"x"}]'; fg; has '1 check(s) are not ok or skip'
t "a check with an unknown result is a MISMATCH"
write_check "$HEAD" block 0 '[{"name":"suite","result":"fine","detail":"x"}]'; fg; has '1 check(s) are not ok or skip'
t "a result with no base is a MISMATCH"; write_check "$HEAD" block 0 "$TWO_CHECKS" ""; fg; has "base '-' is not a full commit name"
t "... and the pr check names the missing base"; has 'the evidence names no base commit'
t "a checked base outside the feature branch is a MISMATCH"; write_check "$HEAD"; printf 'diverged\n' > "$FAKE/compare.txt"; fg
has "the checked base fedcba987654 is not in the history of feat/big-thing (compare: diverged)"
t "... with no command"; hasnt 'run: gh'
t "the gate asks GitHub to compare the checked base with the feature branch tip"
if grep -qFx "api repos/o/r/compare/$FBASE...$TIP --jq .status" "$FAKE/calls"; then pass; else fail "calls: $(tr '\n' '|' < "$FAKE/calls")"; fi
t "a checked base equal to the feature branch tip passes"; printf 'identical\n' > "$FAKE/compare.txt"; fg; rc_is 0
rm -f "$FAKE/compare.txt"
t "an empty checks list is a MISMATCH"; write_check "$HEAD" block 0 '[]'; fg; has 'no check was recorded'
t "a skipped suite is a MISMATCH"
write_check "$HEAD" block 0 '[{"name":"receipt","result":"ok","detail":"x"},{"name":"suite","result":"skip","detail":"--no-suite"}]'; fg
has 'the suite did not run and pass'
t "no suite check at all is a MISMATCH"
write_check "$HEAD" block 0 '[{"name":"receipt","result":"ok","detail":"x"}]'; fg; has 'the suite did not run and pass'
t "... and no command is printed for it"; hasnt 'run: gh'
t "a file that is not JSON is a MISMATCH"; printf 'nope\n' > "$EV/fleet-check.json"; fg; has 'MISMATCH evidence: fleet-check.json does not parse'
t "two JSON documents in the file are a MISMATCH"
write_check "$HEAD"; cat "$EV/fleet-check.json" "$EV/fleet-check.json" > "$EV/two"; mv "$EV/two" "$EV/fleet-check.json"; fg
has 'MISMATCH evidence: fleet-check.json does not parse'
t "a symlinked fleet-check.json is refused"
write_check "$HEAD"; mv "$EV/fleet-check.json" "$FAKE/real.json"; ln -s "$FAKE/real.json" "$EV/fleet-check.json"; fg
has 'MISMATCH evidence: fleet-check.json is a symlink'

# --- ci -----------------------------------------------------------------------------------------
new_case
t "a failed check is a MISMATCH"; write_checks 1 pass fail; fg; has 'MISMATCH ci: 1 pass, 0 skipped, 0 pending, 1 failed or cancelled'
t "a cancelled check is a MISMATCH"; write_checks 1 pass cancel; fg; has '1 failed or cancelled'
t "a pending check with no --wait is a MISMATCH"; write_checks 1 pass pending; fg; has 'MISMATCH ci: still pending after 0s'
t "no checks at all is a MISMATCH"; printf '[]\n' > "$FAKE/checks.1.json"; fg; has 'MISMATCH ci: no check passed'
t "only skipped checks is a MISMATCH"; write_checks 1 skipping skipping; fg; has 'MISMATCH ci: no check passed: every check was skipped'
t "an unreadable answer from gh is a MISMATCH"; printf 'oops\n' > "$FAKE/checks.1.json"; fg; has 'MISMATCH ci: gh pr checks gave no readable list'
new_case
t "--wait polls until the pending check passes"; write_checks 1 pass pending; write_checks 2 pass pending; write_checks 3 pass pass; fg --wait 10
if [ "$RC" = 0 ] && [ "$(cat "$FAKE/n")" = 3 ]; then pass; else fail "rc=$RC calls=$(cat "$FAKE/n") out: $(printf '%s' "$OUT" | tr '\n' '|')"; fi
new_case
t "--wait also waits while no check has started"; printf '[]\n' > "$FAKE/checks.1.json"; write_checks 2 pass; fg --wait 10; rc_is 0
new_case
t "gh saying no checks yet is waited on, not an error"; printf 'NOCHECKS\n' > "$FAKE/checks.1.json"; write_checks 2 pass; fg --wait 10
if [ "$RC" = 0 ] && [ "$(cat "$FAKE/n")" = 2 ]; then pass; else fail "rc=$RC calls=$(cat "$FAKE/n") out: $(printf '%s' "$OUT" | tr '\n' '|')"; fi
new_case
t "with no --wait, no checks yet is a MISMATCH"; printf 'NOCHECKS\n' > "$FAKE/checks.1.json"; fg; has 'MISMATCH ci: no check passed: no checks reported yet'
new_case
t "a gh error is retried while time is left"; printf 'NETERR\n' > "$FAKE/checks.1.json"; write_checks 2 pass; fg --wait 10; rc_is 0
new_case
t "a gh error with no time left is a MISMATCH"; printf 'NETERR\n' > "$FAKE/checks.1.json"; fg; has 'MISMATCH ci: gh pr checks gave no readable list'
new_case
t "FLEET_GATE_INTERVAL=0 falls back to 30s instead of a tight loop"; write_checks 1 pending
OUT=$(FAKE="$FAKE" PATH="$BIN:$PATH" FLEET_GATE_INTERVAL=0 bash "$FG" --evidence "$EV" --head "$HEAD" --pr 12 --gh-repo o/r --base feat/big-thing --wait 1 2>&1); RC=$?
if [ "$RC" = 1 ] && [ "$(cat "$FAKE/n")" = 2 ] && [ "$(cat "$FAKE/sleeps")" = 30 ]; then pass; else fail "rc=$RC calls=$(cat "$FAKE/n") sleeps=$(cat "$FAKE/sleeps" 2>&1)"; fi
new_case
t "an error a retry cannot fix ends the wait at once"; printf 'NOREPO\n' > "$FAKE/checks.1.json"; fg --wait 600
if [ "$RC" = 1 ] && [ "$(cat "$FAKE/n")" = 1 ] && printf '%s' "$OUT" | grep -qF 'MISMATCH ci: gh pr checks failed: GraphQL: Could not resolve'; then pass
else fail "rc=$RC calls=$(cat "$FAKE/n") out: $(printf '%s' "$OUT" | tr '\n' '|')"; fi
new_case
t "--wait stops at once when a check failed"; write_checks 1 pending fail; fg --wait 10
if [ "$RC" = 1 ] && [ "$(cat "$FAKE/n")" = 1 ]; then pass; else fail "rc=$RC calls=$(cat "$FAKE/n")"; fi
new_case
t "--wait gives up after the time and reports pending"; write_checks 1 pending; fg --wait 2; has 'MISMATCH ci: still pending after 2s'

# --- pr -----------------------------------------------------------------------------------------
new_case
t "a pull request into main is a MISMATCH"; write_view OPEN main; fg; has "MISMATCH pr: base 'main' is not feat/big-thing: the owner merges it"
t "... with exit 1 and no command"; if [ "$RC" = 1 ] && ! printf '%s\n' "$OUT" | grep -q '^run:'; then pass; else fail "rc=$RC"; fi
t "another feature branch is a MISMATCH"; write_view OPEN feat/other; fg; has "base 'feat/other' is not feat/big-thing"
t "a base that only contains the name is a MISMATCH"; write_view OPEN release/feat/big-thing; fg; has "is not feat/big-thing"
t "a head that moved is a MISMATCH"; write_view OPEN feat/big-thing "$OTHER"; fg; has "is not --head"
t "the label pd-bot:owner-merge is a MISMATCH"
write_view OPEN feat/big-thing "$HEAD" false '[{"name":"bug"},{"name":"pd-bot:owner-merge"}]'; fg; has 'it has the label pd-bot:owner-merge: the owner merges it'
t "... with exit 1 and no command"; if [ "$RC" = 1 ] && ! printf '%s\n' "$OUT" | grep -q '^run:'; then pass; else fail "rc=$RC"; fi
t "other labels do not stop the gate"; write_view OPEN feat/big-thing "$HEAD" false '[{"name":"bug"}]'; fg; rc_is 0
t "labels that cannot be read are a MISMATCH"; write_view OPEN feat/big-thing "$HEAD" false '"x"'; fg; has 'the labels could not be read'

t "a merged pull request is a MISMATCH"; write_view MERGED; fg; has "state is 'MERGED', not OPEN"
t "a closed pull request is a MISMATCH"; write_view CLOSED; fg; has "state is 'CLOSED', not OPEN"
t "a draft is a MISMATCH"; write_view OPEN feat/big-thing "$HEAD" true; fg; has 'the pull request is a draft'
t "gh pr view failing is a MISMATCH"; rm -f "$FAKE/view.json"; fg; has 'MISMATCH pr: gh pr view failed'
t "an unreadable pull request object is a MISMATCH"; printf '[]\n' > "$FAKE/view.json"; fg; has 'MISMATCH pr: gh pr view gave no readable object'
t "control characters from gh are not printed"
printf '{"state":"OPEN","isDraft":false,"baseRefName":"ma\\u001b[31min","headRefOid":"%s"}\n' "$HEAD" > "$FAKE/view.json"; fg
if printf '%s' "$OUT" | LC_ALL=C grep -q "$(printf '\033')"; then fail "an escape byte reached the output"; else pass; fi

# --- result file and usage ----------------------------------------------------------------------
new_case
t "a symlinked fleet-gate.json is not followed and the gate fails"
: > "$FAKE/victim"; ln -s "$FAKE/victim" "$EV/fleet-gate.json"; fg
if [ "$RC" = 1 ] && [ ! -s "$FAKE/victim" ] && ! printf '%s\n' "$OUT" | grep -q '^run:'; then pass; else fail "rc=$RC victim=$(cat "$FAKE/victim")"; fi
t "... and the last line names the write failure"
if [ "$(printf '%s\n' "$OUT" | tail -1)" = 'fleet-gate: cannot write the result file, so the gate does not pass' ]; then pass; else fail "$(printf '%s\n' "$OUT" | tail -1)"; fi
new_case
u() { OUT=$(FAKE="$FAKE" PATH="$BIN:$PATH" bash "$FG" "$@" 2>&1); RC=$?; }
why() { if [ "$RC" = 2 ] && printf '%s' "$OUT" | grep -qF -- "$1"; then pass; else fail "rc=$RC, want 2 with '$1' — $(printf '%s' "$OUT" | tr '\n' '|')"; fi; }
t "no arguments exits 2"; u; rc_is 2
t "a missing --pr exits 2 and names it"; u --evidence "$EV" --head "$HEAD" --gh-repo o/r --base feat/x
if [ "$RC" = 2 ] && printf '%s' "$OUT" | grep -qF -- '--pr is required'; then pass; else fail "rc=$RC $OUT"; fi
t "a short --head exits 2"; u --evidence "$EV" --head abc123 --pr 1 --gh-repo o/r --base feat/x; why '--head must be a full'
t "a --pr that is not a number exits 2"; u --evidence "$EV" --head "$HEAD" --pr '1;x' --gh-repo o/r --base feat/x; why '--pr must be a positive number'
t "a --gh-repo with no slash exits 2"; u --evidence "$EV" --head "$HEAD" --pr 1 --gh-repo name --base feat/x; why '--gh-repo must be owner/name'
t "a --gh-repo with a shell character exits 2"; u --evidence "$EV" --head "$HEAD" --pr 1 --gh-repo 'o/r;x' --base feat/x; why '--gh-repo must be owner/name'
t "a --gh-repo that starts with a dash exits 2"; u --evidence "$EV" --head "$HEAD" --pr 1 --gh-repo '-o/r' --base feat/x; why '--gh-repo must be owner/name'
t "a --wait that is not a number exits 2"; u --evidence "$EV" --head "$HEAD" --pr 1 --gh-repo o/r --base feat/x --wait soon; why '--wait must be a number'
t "an --evidence that is not a directory exits 2"; u --evidence "$EV/nope" --head "$HEAD" --pr 1 --gh-repo o/r --base feat/x; why '--evidence is not a directory'
t "a missing --base exits 2"; u --evidence "$EV" --head "$HEAD" --pr 1 --gh-repo o/r; rc_is 2
t "a --base of main exits 2"; u --evidence "$EV" --head "$HEAD" --pr 1 --gh-repo o/r --base main; rc_is 2
t "a --base of bare feat/ exits 2"; u --evidence "$EV" --head "$HEAD" --pr 1 --gh-repo o/r --base feat/; rc_is 2
t "a --base with a shell character exits 2"; u --evidence "$EV" --head "$HEAD" --pr 1 --gh-repo o/r --base 'feat/x;y'; rc_is 2
t "an unknown option exits 2"; u --evidence "$EV" --head "$HEAD" --pr 1 --gh-repo o/r --force; rc_is 2
for h in --help -h; do
  t "$h prints the usage to stdout, exits 0 and writes nothing to stderr"
  OUT=$(FAKE="$FAKE" PATH="$BIN:$PATH" bash "$FG" "$h" 2>"$FAKE/h.err"); RC=$?
  if [ "$RC" = 0 ] && printf '%s' "$OUT" | grep -q '^usage: fleet-gate.sh' && [ ! -s "$FAKE/h.err" ]; then pass; else fail "rc=$RC out: $OUT err: $(cat "$FAKE/h.err")"; fi
  t "$h runs no check (no result file)"
  rm -f "$EV/fleet-gate.json"; u "$h" --evidence "$EV" --head "$HEAD" --pr 1 --gh-repo o/r --base feat/x
  if [ "$RC" = 0 ] && [ ! -e "$EV/fleet-gate.json" ]; then pass; else fail "rc=$RC out: $OUT"; fi
done
t "a usage error still prints to stderr, not stdout"
OUT=$(FAKE="$FAKE" PATH="$BIN:$PATH" bash "$FG" --bogus 2>/dev/null); RC=$?
if [ "$RC" = 2 ] && [ -z "$OUT" ]; then pass; else fail "rc=$RC stdout: $OUT"; fi
t "a usage error calls gh zero times"; if [ -f "$FAKE/calls" ]; then fail "calls: $(cat "$FAKE/calls")"; else pass; fi

finish
