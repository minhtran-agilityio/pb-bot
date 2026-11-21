#!/usr/bin/env bash
# The live run board (plugins/pd-bot/board/): label map, run-dir tailing and the SSE server. The cases are
# in tests/board/*.test.js (the fleet page test skips without playwright); this wrapper puts them in the L3 suite.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
if ! command -v node >/dev/null 2>&1; then
  echo 'board: skip — no node'
  exit 0
fi
node "$root/tests/board/board.test.js"
node "$root/tests/board/board-ui.test.js"
node "$root/tests/board/fleet-json.test.js"
node "$root/tests/board/fleet-page.test.js"
node "$root/tests/board/pods.test.js"
