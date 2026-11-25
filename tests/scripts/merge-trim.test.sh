#!/bin/bash
# Contract tests for plugins/pd-bot/skills/pd-setup/scripts/merge-trim.sh.
source "$(dirname "$0")/../lib.sh"

SCRIPT="$ROOT/plugins/pd-bot/skills/pd-setup/scripts/merge-trim.sh"
TRIM="$ROOT/plugins/pd-bot/scripts/fleet-trim.json"

if ! command -v jq >/dev/null 2>&1; then
  t "merge-trim tests (need jq)"; skip "jq is not installed; install jq to run these cases"
  finish
fi

work() { local d; d=$(mktemp -d); _CLEANUP_DIRS+=("$d"); echo "$d"; }

t "the trim is merged and every other key and plugin choice is kept"
d=$(work)
printf '%s' '{"model":"opus","permissions":{"deny":["Read(./.env)"]},"enabledPlugins":{"mine@m":true}}' > "$d/s.json"
bash "$SCRIPT" "$TRIM" "$d/s.json" >/dev/null; rc=$?
if [ "$rc" -eq 0 ] && jq -e --slurpfile t "$TRIM" '.model == "opus" and .permissions.deny == ["Read(./.env)"]
     and .enabledPlugins["mine@m"] == true and .disableClaudeAiConnectors == true
     and (has("autoCompactWindow") | not)
     and (. as $s | $t[0].enabledPlugins | to_entries | all($s.enabledPlugins[.key] == .value))' "$d/s.json" >/dev/null; then pass
else fail "rc=$rc file: $(cat "$d/s.json")"; fi

t "a second run is a no-op: byte-identical, says already present"
cp "$d/s.json" "$d/before.json"
out=$(bash "$SCRIPT" "$TRIM" "$d/s.json"); rc=$?
if [ "$rc" -eq 0 ] && cmp -s "$d/s.json" "$d/before.json" && printf '%s' "$out" | grep -q 'already present'; then pass
else fail "rc=$rc out=$out"; fi

t "--dry-run prints a diff and writes nothing"
d=$(work)
printf '%s' '{"model":"sonnet"}' > "$d/s.json"
cp "$d/s.json" "$d/before.json"
out=$(bash "$SCRIPT" --dry-run "$TRIM" "$d/s.json"); rc=$?
if [ "$rc" -eq 0 ] && cmp -s "$d/s.json" "$d/before.json" && printf '%s' "$out" | grep -q 'disableClaudeAiConnectors'; then pass
else fail "rc=$rc out=$out"; fi

t "a missing file (and its folder) is created"
d=$(work)
bash "$SCRIPT" "$TRIM" "$d/.claude/settings.json" >/dev/null; rc=$?
if [ "$rc" -eq 0 ] && jq -e '.disableClaudeAiConnectors == true' "$d/.claude/settings.json" >/dev/null; then pass
else fail "rc=$rc"; fi

t "invalid JSON or a non-object enabledPlugins exits 2 and leaves the file untouched"
d=$(work)
ok=1
for body in '{"x": oops' '[]' '{"enabledPlugins":[]}'; do
  printf '%s' "$body" > "$d/s.json"; cp "$d/s.json" "$d/before.json"
  bash "$SCRIPT" "$TRIM" "$d/s.json" >/dev/null 2>&1; rc=$?
  { [ "$rc" -eq 2 ] && cmp -s "$d/s.json" "$d/before.json"; } || { ok=0; echo "        body=$body rc=$rc"; }
done
[ "$ok" -eq 1 ] && pass || fail "a bad file was accepted or changed"

t "a false disableClaudeAiConnectors in the trim file stays false (jq // would make it true)"
d=$(work)
printf '%s' '{"disableClaudeAiConnectors":false,"enabledPlugins":{"x@m":false}}' > "$d/t.json"
bash "$SCRIPT" "$d/t.json" "$d/s.json" >/dev/null; rc=$?
if [ "$rc" -eq 0 ] && jq -e '.disableClaudeAiConnectors == false' "$d/s.json" >/dev/null; then pass
else fail "rc=$rc file: $(cat "$d/s.json")"; fi

t "an empty settings file is treated as {}"
d=$(work)
: > "$d/s.json"
bash "$SCRIPT" "$TRIM" "$d/s.json" >/dev/null; rc=$?
if [ "$rc" -eq 0 ] && jq -e '.disableClaudeAiConnectors == true' "$d/s.json" >/dev/null; then pass
else fail "rc=$rc"; fi

t "--compact-window replaces the trim file's autoCompactWindow; below 100000 or not a number exits 1"
d=$(work)
bash "$SCRIPT" --compact-window 300000 "$TRIM" "$d/s.json" >/dev/null; rc=$?
ok=1
{ [ "$rc" -eq 0 ] && jq -e '.autoCompactWindow == 300000' "$d/s.json" >/dev/null; } || ok=0
for w in 99999 abc "" 99999999999999999999; do bash "$SCRIPT" --compact-window "$w" "$TRIM" "$d/x.json" >/dev/null 2>&1; [ "$?" -eq 1 ] || ok=0; done
[ ! -e "$d/x.json" ] || ok=0
if [ "$ok" -eq 1 ]; then pass; else fail "rc=$rc file: $(cat "$d/s.json")"; fi

t "usage error exits 1"
bash "$SCRIPT" "$TRIM" >/dev/null 2>&1; rc=$?
[ "$rc" -eq 1 ] && pass || fail "rc=$rc"

finish
