#!/bin/bash
# Run the project wiki check and print its result. REPORT ONLY: it never fails the suite.
#
# Verifies:        nothing. It shows `wiki_check.py --strict` for docs/wiki: each quote is in its
#                  source, each source is tracked, each ADR is on a page, no page is stale.
# Does not verify: that the wiki is current. A failing wiki prints "report only" and exits 0.
#
# Why report only: the wiki is an experiment (#144). `pd-wiki` forbids a wiki change in a ticket
# pull request, so a hard gate would turn every ticket red that adds an ADR or rewords a quoted
# line, with no allowed fix. #146 turns these failures into CI failures, with the Action of #147
# that updates the wiki after a merge. The unit tests of wiki_check.py and wiki_gen.py still fail
# the suite as normal tests.
#
# Usage: scripts/check-wiki.sh [repo-root]    (default: the git toplevel of the cwd)
# Exit 0 always, except 2 when the root cannot be entered.
set -u

HERE="$(cd "$(dirname "$0")" && pwd)"
CHECK="$HERE/../plugins/pd-bot/scripts/wiki_check.py"
ROOT="${1:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"
cd "$ROOT" || { echo "check-wiki: cannot cd to $ROOT" >&2; exit 2; }

if ! command -v python3 >/dev/null 2>&1; then
  echo "  skip — no python3"
  exit 0
fi

if python3 "$CHECK" --root . --dir docs/wiki --strict; then
  echo "  wiki (report only, see #146): ok"
else
  echo "  wiki (report only, see #146): the lines above failed; this is not a suite failure"
fi
exit 0
