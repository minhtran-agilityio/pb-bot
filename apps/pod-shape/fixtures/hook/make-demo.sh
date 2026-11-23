#!/bin/bash
# Builds a fake artifact-hook output (HANDOFF §11.3) in a folder, for the step 2 demo and the Mac check.
# Usage: bash apps/pod-shape/fixtures/hook/make-demo.sh <dest> [<project folder for auto-select>]
# Then run a debug build of the app with the two variables it prints (release builds ignore them).
# It never touches ~/claude-artifacts or ~/.claude.
set -euo pipefail

dest=${1:?usage: make-demo.sh <dest> [project folder]}
here=$(cd "$(dirname "$0")" && pwd)
app=$(cd "$here/../.." && pwd)
project=${2:-$(cd "$app/../.." && pwd)}

# Safety: this script deletes and rewrites <dest>/claude-artifacts. Resolve <dest> first, so no spelling of
# the home folder (~, ~/, ~/., a symlink) can make that the real ~/claude-artifacts. Then only a new or empty
# folder, or one this script made before (it holds the marker file), is used.
marker=".shape-board-demo"
mkdir -p "$dest"
dest=$(cd "$dest" && pwd -P)
home=$(cd ~ && pwd -P)
case "$dest/" in
  "$home/" | "$home/.claude/"* | "$home/claude-artifacts/"* | "/") echo "refusing: $dest is the home folder or a real hook folder" >&2; exit 1 ;;
esac
if [ -n "$(ls -A "$dest")" ] && [ ! -f "$dest/$marker" ]; then
  echo "refusing: $dest is not empty and was not made by this script" >&2
  exit 1
fi
touch "$dest/$marker"

root="$dest/claude-artifacts"
log="$dest/artifact-log.tsv"
rm -rf "$root" "$log"
mkdir -p "$root"

# put <id> <version file name> <source> <touch time YYYYMMDDhhmm>
put() {
  mkdir -p "$root/$1"
  cp "$2" "$root/$1/$3"
  touch -t "$4" "$root/$1/$3"
}

proto="$app/docs/prototype/shape-board-prototype.html"
probe="$here/files/security-probe.html"
old="$here/files/old-notes.html"

# 1. The real prototype, two versions (the second one adds one comment line), published from the repo.
put Fx7hQ2protoShapeBoard "$proto" 20261121T090000Z_shape-board-prototype.html 202611210900
{ cat "$proto"; echo "<!-- version 2 -->"; } >"$dest/v2.html"
put Fx7hQ2protoShapeBoard "$dest/v2.html" 20261122T111340Z_shape-board-prototype.html 202611221113
rm "$dest/v2.html"
# 2. The security probe, published from the repo.
put SecProbe9Kc "$probe" 20261122T120000Z_security-probe.html 202611221200
# 3. An old 4-column line, from no repo.
put OldNotes4col "$old" 20261120T080000Z_old-notes.html 202611200800
# 4. A half-written file the hook is still copying: ignored.
printf '<p>half</p>' >"$root/SecProbe9Kc/20261122T130000Z_next.html.tmp"

tab=$'\t'
{
  echo "2026-11-20T08:00:00Z${tab}https://claude.ai/artifact/OldNotes4col${tab}Old notes${tab}$root/OldNotes4col"
  echo "2026-11-21T09:00:00Z${tab}https://claude.ai/artifact/Fx7hQ2protoShapeBoard${tab}Shape Board prototype${tab}$root/Fx7hQ2protoShapeBoard${tab}$project/apps/pod-shape${tab}$root/Fx7hQ2protoShapeBoard/20261121T090000Z_shape-board-prototype.html"
  echo "this line is broken"
  echo "2026-11-22T11:13:40Z${tab}https://claude.ai/artifact/Fx7hQ2protoShapeBoard${tab}Shape Board prototype${tab}$root/Fx7hQ2protoShapeBoard${tab}$project/apps/pod-shape${tab}$root/Fx7hQ2protoShapeBoard/20261122T111340Z_shape-board-prototype.html"
  echo "2026-11-22T12:00:00Z${tab}https://claude.ai/artifact/SecProbe9Kc${tab}Security probe${tab}$root/SecProbe9Kc${tab}$project${tab}$root/SecProbe9Kc/20261122T120000Z_security-probe.html"
} >"$log"

echo "export SHAPE_BOARD_ARTIFACTS_DIR='$root'"
echo "export SHAPE_BOARD_ARTIFACT_LOG='$log'"
