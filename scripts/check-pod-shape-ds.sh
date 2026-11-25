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
# grep exits 0 (hits), 1 (none) or 2 (error). An error must fail the gate, not read as "no hits".
# Runs in the main shell (not a $(...) subshell) so a failure can stop the script.
g() { # grep arguments; result in $HITS
  HITS=$(grep "$@"); local rc=$?
  if [ "$rc" -gt 1 ]; then echo "  ✗ grep failed (exit $rc): grep $*" >&2; exit 1; fi
}
report() { # name; reads $HITS
  if [ -n "$HITS" ]; then
    echo "  ✗ $1" >&2
    printf '%s\n' "$HITS" >&2
    fail=1
  else
    echo "  ✓ $1"
  fi
}

g -rnE "#[0-9a-fA-F]{3,8}|\[[0-9]+px\]|-(red|blue|green|gray|slate|zinc)-[0-9]" src/ --include='*.tsx'
report "no hex, [Npx] or default Tailwind colour"
g -rnE "gradient|backdrop-blur|Sparkles|Wand|uppercase|tracking-wider|rounded-(2xl|3xl|lg)|shadow-(xl|2xl|md)" src/ --include='*.tsx'
report "nothing from §0.1"
g -rn "font-mono" src/ --include='*.tsx'
HITS=$(printf '%s' "$HITS" | grep -vE '^src/components/(app/RunLog|app/BriefCard|app/ChatMessage|ui/textarea)\.tsx:')
report "font-mono only in RunLog, BriefCard, ChatMessage, Textarea"
g -rlE "from ['\"]lucide-react['\"]" src/ --include='*.ts' --include='*.tsx'
HITS=$(printf '%s' "$HITS" | grep -v '^src/components/app/icons\.ts$')
report "lucide-react only through icons.ts"

exit "$fail"
