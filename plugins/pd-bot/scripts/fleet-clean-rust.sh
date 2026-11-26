#!/bin/bash
# Deletes stale Rust build output (a cargo `target/` folder) that a fleet run left behind, and prints
# the space it freed. The orchestrator runs it in the clean-up step (skills/pd-orchestrate/references/
# podbay.md, "Clean up"), for what `podbay-dev ws-rm` does not remove: the main checkout's own
# `apps/*/src-tauri/target`, a CARGO_TARGET_DIR that a session used outside its workspace, and the
# `target/` of a workspace that `ws-rm` refused and so stays.
#
# What it deletes: a folder that is all of
#   - a real directory, not a symlink;
#   - a cargo target dir: it holds a regular file CACHEDIR.TAG whose first line is the cache-directory
#     signature (`Signature: 8a477f597d28d172789f06886806bc55`, shared by every tool that writes such a
#     tag) and whose second line is cargo's own (`# This file is a cache directory tag created by cargo.`);
#   - found by --root: also has a Cargo.toml beside it (the project's own build folder);
#   - looks like build output: `.rustc_info.json`, `debug` or `release` is in it, and every entry at its top
#     level is something cargo writes (`CACHEDIR.TAG`, `.rustc_info.json`, `.cargo-lock`, `doc`, `package`,
#     `tmp`, `flycheck*`, or a folder that holds `deps`, `.fingerprint`, `debug` or `release`: a profile or a
#     target-triple folder). A shared folder with the tag and other files in it is kept whole;
#   - not `/`, not a folder of fewer than two path parts, not the home folder or a folder above it,
#     not a folder above a checkout in --root, holds no `.git` at any depth and no `Cargo.toml`,
#     `src` or `package.json` of its own, and no file that git tracks (that is source, not build output);
#   - not used by a running process: no process has a file or its working directory inside it, and,
#     for a folder found by --root, no build process (cargo, rustc, a C compiler or linker, tauri, node, npm)
#     works inside the project that owns it. Paths are compared as the system spells them (`/bin/pwd -P`,
#     which also fixes the case of a path on a case-insensitive disk), the way lsof prints them.
# Anything else is kept, and the line says why. Source, an installed app (~/Applications) and a
# `target` folder of another tool (no CACHEDIR.TAG) are never touched. Only the target folder is removed:
# `rm -rf` of that one folder, never of its parent.
#
# If it cannot tell what is in use (no `lsof`; a list that lacks this script's own working directory or
# most of the user's processes), or HOME is unset, or git is missing, it deletes nothing and exits 1.
# --root and --target must be absolute paths: a value that comes from a session's report is not
# trusted. Known limit: a process can start, or a path can be swapped, between the checks and the
# `rm` (same user, build output only).
#
# Usage:
#   fleet-clean-rust.sh -h|--help
#   fleet-clean-rust.sh [--dry-run] [--root <checkout>]... [--target <dir>]...
# --root <dir>    a checkout or workspace to scan for target folders (to 5 levels down;
#                 `node_modules` and `.git` are skipped); repeatable
# --target <dir>  one target folder named exactly (a CARGO_TARGET_DIR a session reported); repeatable.
#                 Run --dry-run first for a folder that came from a report, and never pass a folder
#                 above a checkout or a live workspace as --root.
# --dry-run       change nothing; print what would be deleted
# Needs at least one --root or --target.
# Output: `deleted|would delete <path> (<size>)`, `kept <path>: <reason>`, and the last line
# `fleet-clean-rust: freed <size> in <n> folder(s), kept <m>` (with --dry-run: `would free`).
# Exit: 0 when it finished (a path that does not exist is kept, not an error); 1 when a delete failed
# or it cannot tell what is in use; 2 on a usage error.
set -u
unset CDPATH

SIGNATURE='Signature: 8a477f597d28d172789f06886806bc55'
TAG_LINE2='# This file is a cache directory tag created by cargo.'

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

# Fail closed: two checks need git and HOME, and a check that is off by accident deletes too much.
if ! command -v git >/dev/null 2>&1; then
  echo "fleet-clean-rust: git is not installed, so I cannot tell whether git tracks files in a build folder; deleted nothing" >&2
  exit 1
fi
if [ -z "${HOME:-}" ]; then
  echo "fleet-clean-rust: HOME is not set, so I cannot protect the home folder; deleted nothing" >&2
  exit 1
fi

# The real path of the working directory, spelled as the system and lsof spell it: the shell's own
# `pwd -P` keeps the case that was typed on a case-insensitive disk. A pwd that is not the system
# one is not trusted.
PWDP=/bin/pwd; [ -x "$PWDP" ] || PWDP=/usr/bin/pwd
if [ ! -x "$PWDP" ]; then echo "fleet-clean-rust: no /bin/pwd; deleted nothing" >&2; exit 1; fi

WORK=$(mktemp -d) || exit 1
trap 'rm -rf "$WORK"' EXIT
ps -U "$(id -u)" -o pid= 2>/dev/null > "$WORK/ps.txt"
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
# Sanity: an lsof that failed or sees little (a sandbox, a container, a limit, a permission error) gives an
# empty or partial list, and "nothing uses it" would be wrong. The list must show this script's own
# working directory under the same spelling, and at least 80 % of the user's processes that are
# still alive (the `ps` list was taken before lsof ran).
own_cwd=$("$PWDP" -P)
cut -f1 "$WORK/open.tsv" | LC_ALL=C sort -u > "$WORK/pids.txt"
alive=0; covered=0
while IFS= read -r pid; do
  pid=${pid//[[:space:]]/}
  [ -n "$pid" ] || continue
  kill -0 "$pid" 2>/dev/null || continue
  alive=$((alive + 1))
  grep -qx -- "$pid" "$WORK/pids.txt" && covered=$((covered + 1))
done < "$WORK/ps.txt"
if ! OWN="$own_cwd" awk -F'\t' -v me="$$" '$1 == me && $3 == "cwd" && $4 == ENVIRON["OWN"] { found = 1 } END { exit !found }' "$WORK/open.tsv" \
   || [ "$alive" -lt 1 ] || [ $((covered * 5)) -lt $((alive * 4)) ]; then
  echo "fleet-clean-rust: lsof gave no usable list of open files, so I cannot tell whether a process uses a build folder; deleted nothing" >&2
  exit 1
fi

canon() { (cd "$1" 2>/dev/null && "$PWDP" -P); }

home_real=$(canon "$HOME") || { echo "fleet-clean-rust: cannot resolve HOME; deleted nothing" >&2; exit 1; }
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
    proj != "" && $3 == "cwd" && ($2 ~ /^(cargo|rustc|rustdoc|clippy|cc|c\+\+|clang|gcc|g\+\+|ld|lld|link|tauri|node|npm|pnpm|yarn|bun)/) && (n == proj || index(n, proj "/") == 1) {
      print $2 " (pid " $1 ") is building in " proj; found = 1; exit }
    END { exit !found }' "$WORK/open.tsv") || return 1
  echo "$why"
}

# consider <dir> <found by --root: the canonical root, else empty>
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
  # A folder that a scan found must lie inside the root it was found under.
  if [ -n "$scanned" ]; then
    case "$p/" in "$scanned"/*) ;; *) kept "$p" "outside the --root it was found under"; return ;; esac
  fi
  if [ -n "$(printf '%s' "$p" | LC_ALL=C tr -d '[:print:]')" ]; then kept "$p" "its path holds a control character"; return; fi
  if [ "$p" = / ] || [ "$(printf '%s' "$p" | tr -cd / | wc -c | tr -d ' ')" -lt 2 ]; then
    kept "$p" "the root folder or a folder too close to it"; return
  fi
  case "$home_real/" in "$p"/*) kept "$p" "the home folder, or a folder that holds it"; return ;; esac
  for r in ${roots_real[@]+"${roots_real[@]}"}; do
    case "$r/" in "$p"/*) kept "$p" "holds a checkout"; return ;; esac
  done
  if [ -n "$(find "$p" -name .git -prune -print -quit 2>/dev/null)" ]; then
    kept "$p" "holds a git checkout"; return
  fi
  if [ -e "$p/.git" ] || [ -e "$p/Cargo.toml" ] || [ -e "$p/src" ] || [ -e "$p/package.json" ]; then
    kept "$p" "holds source (.git, Cargo.toml, src or package.json)"; return
  fi
  if [ -L "$p/CACHEDIR.TAG" ] || [ ! -f "$p/CACHEDIR.TAG" ] \
     || [ "$(head -n 1 "$p/CACHEDIR.TAG" 2>/dev/null)" != "$SIGNATURE" ] \
     || [ "$(sed -n 2p "$p/CACHEDIR.TAG" 2>/dev/null)" != "$TAG_LINE2" ]; then
    kept "$p" "not a cargo target dir (no CACHEDIR.TAG written by cargo)"; return
  fi
  if [ ! -f "$p/.rustc_info.json" ] && [ ! -d "$p/debug" ] && [ ! -d "$p/release" ]; then
    kept "$p" "does not look like build output (no .rustc_info.json, debug or release)"; return
  fi
  # Every top-level entry must be one cargo writes: a folder shared with other files is kept whole.
  while IFS= read -r e; do
    case "$e" in
      CACHEDIR.TAG|.rustc_info.json|.cargo-lock|.future-incompat-report.json|doc|package|tmp|cargo-timings|flycheck*) ;;
      *) if [ -d "$p/$e" ] && [ ! -L "$p/$e" ] \
            && { [ -d "$p/$e/deps" ] || [ -d "$p/$e/.fingerprint" ] || [ -d "$p/$e/debug" ] || [ -d "$p/$e/release" ]; }; then :
         else kept "$p" "not only build output: $e is not something cargo writes"; return; fi ;;
    esac
  done < <(ls -A "$p" 2>/dev/null)
  if [ -n "$(git -C "$p" -c core.fsmonitor=false ls-files 2>/dev/null | head -n 1)" ]; then
    kept "$p" "git tracks files in it"; return
  fi
  parent=""
  if [ -n "$scanned" ]; then
    parent=$(dirname "$p")
    if [ ! -f "$parent/Cargo.toml" ]; then kept "$p" "no Cargo.toml beside it"; return; fi
  fi
  if why=$(in_use "$p" "$parent"); then kept "$p" "in use: $why"; return; fi
  kib=$(du -sk "$p" 2>/dev/null | cut -f1)
  kib=${kib:-0}
  if [ "$dry" = 1 ]; then
    echo "would delete $p ($(human "$kib"))"
  else
    # Look again just before the delete: still the same real folder, still a directory, not a symlink.
    if [ -L "$p" ] || [ ! -d "$p" ] || [ "$(canon "$p")" != "$p" ]; then kept "$p" "changed while I checked it"; return; fi
    rm -rf -- "$p"
    if [ -e "$p" ]; then
      echo "FAILED to delete $p" >&2; failed=1; n_kept=$((n_kept + 1)); return
    fi
    echo "deleted $p ($(human "$kib"))"
  fi
  freed_kib=$((freed_kib + kib)); n_done=$((n_done + 1))
}

for r in ${roots[@]+"${roots[@]}"}; do
  while [ "${#r}" -gt 1 ] && [ "${r%/}" != "$r" ]; do r=${r%/}; done   # `link/` must still be seen as a symlink
  if [ ! -d "$r" ] || [ -L "$r" ]; then kept "$r" "root is not a directory"; continue; fi
  rr=$(canon "$r") || { kept "$r" "cannot enter it"; continue; }
  # NUL-separated, so a newline in a folder name cannot split a path in two.
  while IFS= read -r -d '' found; do
    consider "$found" "$rr"
  done < <(find "$r" -maxdepth 5 \( -name node_modules -o -name .git \) -prune -o -type d -name target -print0 -prune 2>/dev/null | LC_ALL=C sort -z)
done
for t in ${targets[@]+"${targets[@]}"}; do
  consider "$t" ""
done

verb="freed"; [ "$dry" = 1 ] && verb="would free"
echo "fleet-clean-rust: $verb $(human "$freed_kib") in $n_done folder(s), kept $n_kept"
exit "$failed"
