#!/bin/bash
# Run the Pod Board #137 acceptance tests (sources, reasons, steps, sub-agents). Needs python3 and Chrome/Chromium.
# POD_BOARD_SKIP_DESKTOP=1 skips the Desktop rows.
exec python3 -I "$(dirname "$0")/test_sources_and_steps.py" "$@"
