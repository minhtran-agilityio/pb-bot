#!/bin/bash
# Run the Pod Board #176 acceptance tests (Desktop source). Needs python3 and Chrome/Chromium.
# #176 makes the Desktop rows of #137 required: POD_BOARD_SKIP_DESKTOP is cleared, and with no Chrome the run fails.
# Chrome is found on PATH or as the macOS app; the #137 file finds it only on PATH, so a temporary shim script puts it there
# (a symlink to the macOS binary does not start: Chrome finds its framework from the path it was started by).
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
unset POD_BOARD_SKIP_DESKTOP
CHROME=""
for n in google-chrome chromium chromium-browser google-chrome-stable; do
  CHROME="$(command -v "$n" 2>/dev/null)" && break
done
MAC_CHROME="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
[ -n "$CHROME" ] || { [ -x "$MAC_CHROME" ] && CHROME="$MAC_CHROME"; }
if [ -z "$CHROME" ]; then
  echo "run_176: no Chrome/Chromium found; the #176 page checks are required" >&2
  exit 1
fi
SHIM="$(mktemp -d "${TMPDIR:-/tmp}/pod-board-176-shim.XXXXXX")"
trap 'rm -rf "$SHIM"' EXIT
printf '#!/bin/sh\nexec "%s" "$@"\n' "$CHROME" >"$SHIM/google-chrome"
chmod +x "$SHIM/google-chrome"
export PATH="$SHIM:$PATH"
rc=0
python3 -I "$HERE/test_sources_and_steps.py" -v || rc=1
python3 -I "$HERE/test_desktop_176.py" -v || rc=1
exit "$rc"
