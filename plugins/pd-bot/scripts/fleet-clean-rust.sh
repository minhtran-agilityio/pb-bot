#!/bin/bash
# Deletes stale Rust build output (a cargo `target/` folder) that a fleet run left behind, and prints
# the space it freed. The orchestrator runs it in the clean-up step (skills/pd-orchestrate/references/
# podbay.md, "Clean up"), for what `podbay-dev ws-rm` does not remove: the main checkout's own
# `apps/*/src-tauri/target`, a CARGO_TARGET_DIR that a session used outside its workspace, and the
# `target/` of a workspace that `ws-rm` refused and so stays.
#
# What it deletes: a folder that is all of
#   - a real directory, not a symlink;
#   - a cargo target dir: it holds a regular file CACHEDIR.TAG whose first line is cargo's signature
#     (`Signature: 8a477f597d28d172789f06886806bc55`). Cargo writes that file itself;
#   - found by --root: also has a Cargo.toml beside it (the project's own build folder);
#   - looks like build output: `.rustc_info.json`, `debug` or `release` is in it;
#   - not `/`, not a folder of fewer than two path parts, not the home folder or a folder above it,
#     not a folder above a checkout in --root, holds no `.git` (to 4 levels down) and no `Cargo.toml`,
#     `src` or `package.json` of its own, and no file that git tracks (that is source, not build output);
#   - not used by a running process: no process has a file or its working directory inside it, and,
#     for a folder found by --root, no `cargo` or `rustc` process works inside the project that owns it.
# Anything else is kept, and the line says why. Source, an installed app (~/Applications) and a
# `target` folder of another tool (no CACHEDIR.TAG) are never touched. Only the target folder is removed:
# `rm -rf` of that one folder, never of its parent.
#
# If it cannot tell what is in use (no `lsof`, or `lsof` gives no usable list), it deletes nothing and
# exits 1. --root and --target must be absolute paths: a value that comes from a session's report is
# not trusted.
#
# Usage:
#   fleet-clean-rust.sh -h|--help
#   fleet-clean-rust.sh [--dry-run] [--root <checkout>]... [--target <dir>]...
# --root <dir>    a checkout or workspace to scan for target folders (to 5 levels down;
#                 `node_modules` and `.git` are skipped); repeatable
# --target <dir>  one target folder named exactly (a CARGO_TARGET_DIR a session reported); repeatable
# --dry-run       change nothing; print what would be deleted
# Needs at least one --root or --target.
# Output: `deleted|would delete <path> (<size>)`, `kept <path>: <reason>`, and the last line
# `fleet-clean-rust: freed <size> in <n> folder(s), kept <m>` (with --dry-run: `would free`).
# Exit: 0 when it finished (a path that does not exist is kept, not an error); 1 when a delete failed
# or it cannot tell what is in use; 2 on a usage error.
set -u
unset CDPATH

SIGNATURE='Signature: 8a477f597d28d172789f06886806bc55'

usage_text() { echo "usage: fleet-clean-rust.sh [-h|--help] [--dry-run] [--root <checkout>]... [--target <dir>]..."; }
usage() { echo "fleet-clean-rust: $*" >&2; usage_text >&2; exit 2; }

dry=0
roots=()
targets=()
while [ $# -gt 0 ]; do
  case "$1" in
    -h|--help) usage_text; exit 0 ;;
    --dry-run) dry=1 ;;
    --root|--target)
      [ $# -ge 2 ] || usage "$1 needs a value"
      case "$2" in /*) ;; *) usage "$1 needs an absolute path, got: $2" ;; esac
      if [ "$1" = --root ]; then roots+=("$2"); else targets+=("$2"); fi
      shift ;;
    *) usage "unknown argument: $1" ;;
  esac
  shift
done
[ ${#roots[@]} -gt 0 ] || [ ${#targets[@]} -gt 0 ] || usage "give at least one --root or --target"

# Fail closed: without lsof nothing says whether a process uses a folder.
if ! command -v lsof >/dev/null 2>&1; then
  echo "fleet-clean-rust: lsof is not installed, so I cannot tell whether a process uses a build folder; deleted nothing" >&2
  exit 1
fi

WORK=$(mktemp -d) || exit 1
trap 'rm -rf "$WORK"' EXIT
# One row per open file of every process: pid, command, fd (cwd, txt, a number), path. A partial
# dump (lsof exits 1 when it could not read some process) is still usable.
lsof -w -n -F pcfn 2>/dev/null | awk '
  /^p/ { pid = substr($0, 2) }
  /^c/ { cmd = substr($0, 2) }
  /^f/ { fd = substr($0, 2) }
  /^n/ {
    # lsof writes a backslash as two; control characters stay escaped (consider() keeps such a path).
    name = substr($0, 2); out = ""
    while ((i = index(name, "\\\\")) > 0) { out = out substr(name, 1, i) ; name = substr(name, i + 2) }
    print pid "\t" cmd "\t" fd "\t" out name }' > "$WORK/open.tsv"
# Sanity: the list must show this script's own working directory. An lsof that failed (a sandbox, a
# limit, a permission error) gives an empty or partial list, and "nothing uses it" would be wrong.
if ! awk -F'\t' -v me="$$" '$1 == me && $3 == "cwd" { found = 1 } END { exit !found }' "$WORK/open.tsv"; then
  echo "fleet-clean-rust: lsof gave no usable list of open files, so I cannot tell whether a process uses a build folder; deleted nothing" >&2
  exit 1
fi

canon() { (cd "$1" 2>/dev/null && pwd -P); }

home_real=$(canon "${HOME:-/nonexistent}")
roots_real=()
for r in ${roots[@]+"${roots[@]}"}; do
  rr=$(canon "$r") && roots_real+=("$rr")
done

# human <KiB> -> "2.5 GiB"
human() {
  awk -v k="$1" 'BEGIN {
    if (k >= 1048576) printf "%.1f GiB", k / 1048576
    else if (k >= 1024) printf "%.1f MiB", k / 1024
    else printf "%d KiB", k }'
}

freed_kib=0; n_done=0; n_kept=0; failed=0; seen=$'\n'

kept() { n_kept=$((n_kept + 1)); echo "kept $1: $2"; }

# in_use <dir> <project dir or empty>: prints why and returns 0 when a running process uses it.
in_use() {
  local why
  why=$(D="$1" PROJ="$2" awk -F'\t' '
    BEGIN { d = ENVIRON["D"]; proj = ENVIRON["PROJ"] }
    { n = $4 }
    n == d || index(n, d "/") == 1 { print $2 " (pid " $1 ") has " (($3 == "cwd") ? "its working directory" : "a file") " inside it"; found = 1; exit }
    proj != "" && $3 == "cwd" && ($2 ~ /^cargo/ || $2 ~ /^rustc/) && (n == proj || index(n, proj "/") == 1) {
      print $2 " (pid " $1 ") is building in " proj; found = 1; exit }
    END { exit !found }' "$WORK/open.tsv") || return 1
  echo "$why"
}

# consider <dir> <found by --root: 1|0>
consider() {
  local dir="$1" scanned="$2" p parent why kib r
  while [ "${#dir}" -gt 1 ] && [ "${dir%/}" != "$dir" ]; do dir=${dir%/}; done   # `link/` must still be seen as a symlink
  if [ -L "$dir" ]; then kept "$dir" "a symlink"; return; fi
  if [ ! -e "$dir" ]; then
    # Already deleted through another name (a --root scan, then the same folder as --target): not a second report.
    p=$(canon "$(dirname "$dir")") && case "$seen" in *$'\n'"$p/$(basename "$dir")"$'\n'*) return ;; esac
    kept "$dir" "does not exist"; return
  fi
  if [ ! -d "$dir" ]; then kept "$dir" "not a directory"; return; fi
  p=$(canon "$dir") || { kept "$dir" "cannot enter it"; return; }
  case "$seen" in *$'\n'"$p"$'\n'*) return ;; esac
  seen="$seen$p"$'\n'
  if [ -n "$(printf '%s' "$p" | LC_ALL=C tr -d '[:print:]')" ]; then kept "$p" "its path holds a control character"; return; fi
  if [ "$p" = / ] || [ "$(printf '%s' "$p" | tr -cd / | wc -c | tr -d ' ')" -lt 2 ]; then
    kept "$p" "the root folder or a folder too close to it"; return
  fi
  case "$home_real/" in "$p"/*) kept "$p" "the home folder, or a folder that holds it"; return ;; esac
  for r in ${roots_real[@]+"${roots_real[@]}"}; do
    case "$r/" in "$p"/*) kept "$p" "holds a checkout"; return ;; esac
  done
  if [ -n "$(find "$p" -maxdepth 4 -name .git -print -quit 2>/dev/null)" ]; then
    kept "$p" "holds a git checkout"; return
  fi
  if [ -e "$p/.git" ] || [ -e "$p/Cargo.toml" ] || [ -e "$p/src" ] || [ -e "$p/package.json" ]; then
    kept "$p" "holds source (.git, Cargo.toml, src or package.json)"; return
  fi
  if [ -L "$p/CACHEDIR.TAG" ] || [ ! -f "$p/CACHEDIR.TAG" ] \
     || [ "$(head -n 1 "$p/CACHEDIR.TAG" 2>/dev/null)" != "$SIGNATURE" ]; then
    kept "$p" "not a cargo target dir (no CACHEDIR.TAG with cargo's signature)"; return
  fi
  if [ ! -f "$p/.rustc_info.json" ] && [ ! -d "$p/debug" ] && [ ! -d "$p/release" ]; then
    kept "$p" "does not look like build output (no .rustc_info.json, debug or release)"; return
  fi
  if command -v git >/dev/null 2>&1 && [ -n "$(git -C "$p" ls-files 2>/dev/null | head -n 1)" ]; then
    kept "$p" "git tracks files in it"; return
  fi
  parent=""
  if [ "$scanned" = 1 ]; then
    parent=$(dirname "$p")
    if [ ! -f "$parent/Cargo.toml" ]; then kept "$p" "no Cargo.toml beside it"; return; fi
  fi
  if why=$(in_use "$p" "$parent"); then kept "$p" "in use: $why"; return; fi
  kib=$(du -sk "$p" 2>/dev/null | cut -f1)
  kib=${kib:-0}
  if [ "$dry" = 1 ]; then
    echo "would delete $p ($(human "$kib"))"
  else
    rm -rf -- "$p"
    if [ -e "$p" ]; then
      echo "FAILED to delete $p" >&2; failed=1; n_kept=$((n_kept + 1)); return
    fi
    echo "deleted $p ($(human "$kib"))"
  fi
  freed_kib=$((freed_kib + kib)); n_done=$((n_done + 1))
}

for r in ${roots[@]+"${roots[@]}"}; do
  if [ ! -d "$r" ] || [ -L "$r" ]; then kept "$r" "root is not a directory"; continue; fi
  while IFS= read -r found; do
    [ -n "$found" ] && consider "$found" 1
  done < <(find "$r" -maxdepth 5 \( -name node_modules -o -name .git \) -prune -o -type d -name target -print -prune 2>/dev/null | LC_ALL=C sort)
done
for t in ${targets[@]+"${targets[@]}"}; do
  consider "$t" 0
done

verb="freed"; [ "$dry" = 1 ] && verb="would free"
echo "fleet-clean-rust: $verb $(human "$freed_kib") in $n_done folder(s), kept $n_kept"
exit "$failed"
