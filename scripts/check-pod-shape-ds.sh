#!/bin/bash
# DESIGN_SYSTEM.md §10 greps for apps/pod-shape, as a gate: exits 1 on any hit.
#
# Verifies:        in apps/pod-shape/src (*.tsx): no raw hex/px/default Tailwind colour; none of the
#                  §0.1 banned styles; font-mono only in RunLog, BriefCard, ChatMessage (code spans) and
#                  the Textarea code variant; lucide-react imported only by src/components/app/icons.ts.
# Does not verify: the other §10 rows (states, themes, keyboard, one primary button): a person checks those.
set -u
cd "$(dirname "$0")/../apps/pod-shape" || exit 1
[ -d src ] || { echo "  ✗ apps/pod-shape/src is missing" >&2; exit 1; }

fail=0
# A grep that exits 2 (error) must not read as "no hits": 0 = hits, 1 = none, 2 = error.
g() { # grep arguments; prints hits; exits the script on a grep error
  local out rc
  out=$(grep "$@"); rc=$?
  if [ "$rc" -gt 1 ]; then echo "  ✗ grep failed (exit $rc): grep $*" >&2; exit 1; fi
  printf '%s' "$out"
}
check() { # name, expected-empty grep output
  if [ -n "$2" ]; then
    echo "  ✗ $1" >&2
    printf '%s\n' "$2" >&2
    fail=1
  else
    echo "  ✓ $1"
  fi
}

check "no hex, [Npx] or default Tailwind colour" \
  "$(g -rnE "#[0-9a-fA-F]{3,8}|\[[0-9]+px\]|-(red|blue|green|gray|slate|zinc)-[0-9]" src/ --include='*.tsx')"
check "nothing from §0.1" \
  "$(g -rnE "gradient|backdrop-blur|Sparkles|Wand|uppercase|tracking-wider|rounded-(2xl|3xl|lg)|shadow-(xl|2xl|md)" src/ --include='*.tsx')"
check "font-mono only in RunLog, BriefCard, ChatMessage, Textarea" \
  "$(g -rn "font-mono" src/ --include='*.tsx' | grep -vE '^src/components/(app/RunLog|app/BriefCard|app/ChatMessage|ui/textarea)\.tsx:')"
check "lucide-react only through icons.ts" \
  "$(g -rlE "from ['\"]lucide-react['\"]" src/ --include='*.ts' --include='*.tsx' | grep -v '^src/components/app/icons\.ts$')"

exit "$fail"
