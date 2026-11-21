#!/usr/bin/env bash
# The live run board (plugins/pd-bot/board/): label map, run-dir tailing and the SSE server. The cases are
# in tests/board/*.test.js and the #16 acceptance tests in tests/acceptance/16/*.test.js (the fleet page
# tests skip without playwright); this wrapper puts them in the L3 suite. It globs both directories, so a
# test file added later is run without an edit here. An empty glob is an error, not a silent pass.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
if ! command -v node >/dev/null 2>&1; then
  echo 'board: skip — no node'
  exit 0
fi
shopt -s nullglob
for dir in tests/board tests/acceptance/16; do
  files=("$root/$dir"/*.test.js)
  if [ "${#files[@]}" -eq 0 ]; then
    echo "board: FAIL — no *.test.js in $dir" >&2
    exit 1
  fi
  for f in "${files[@]}"; do
    node "$f"
  done
done
