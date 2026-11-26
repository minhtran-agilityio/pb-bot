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
#   - looks like build output: `.rustc_info.json`, `debug` or `release` is in it, and one scan of its whole
#     tree (see `tree_stray` below) finds only what cargo and tauri write: a symlink, a special file, a
#     `.git`, a `Cargo.toml`, or any entry outside cargo's layout (a `.rs`, `.py`, `.c`, `.toml`, README, a
#     script) keeps the whole target, whatever its depth, except inside the opaque zones listed there;
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
# trusted. A path with a `.` or `..` component, a control character or a non-ASCII byte is kept. Just before
# the `rm` it checks again that the folder is the same one (device and inode), is not a symlink, and still
# holds cargo's tag, no source, no `.git` at any depth and no git-tracked file; a folder that changed is
# kept and the exit status is 1. Paths in the output are printed with every byte that is not printable ASCII
# replaced by `?`.
# Known limit: the window between that last check and the `rm` itself (a few milliseconds), in which a
# same-user process could still swap the folder, and a process that starts using the folder in that window.
# If the script stops before its last line it exits 1.
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
# Exit: 0 when it finished (a path that does not exist is kept, not an error); 1 when a delete failed,
# a folder changed while it was checked, it stopped early, or it cannot tell what is in use; 2 on a usage error.
set -u
unset CDPATH
# Byte-wise matching: [:print:] below must mean printable ASCII, so that a newline, a control
# character or a non-ASCII byte in a path is seen as one.
export LC_ALL=C

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
# Every path is absolute. Work from `/`, so a working folder with a non-ASCII name cannot make the
# self-check of the lsof list below fail (lsof escapes such a name).
cd / || exit 1

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
finished=0
# shellcheck disable=SC2329  # runs through the EXIT trap
cleanup() {
  local rc=$?
  rm -rf "$WORK"
  if [ "$finished" != 1 ] && [ "$rc" = 0 ]; then echo "fleet-clean-rust: stopped before the end" >&2; exit 1; fi
}
trap cleanup EXIT
# The processes of the user that can be inspected, taken before lsof runs. A zombie, and on Linux a
# process whose /proc/<pid>/cwd cannot be read (not dumpable, another credential), is not in the list:
# lsof cannot list those either, and they would make a full lsof list look partial.
if [ -r /proc/self/cwd ]; then
  for d in /proc/[0-9]*; do
    [ -O "$d" ] && readlink "$d/cwd" >/dev/null 2>&1 && echo "${d#/proc/}"
  done > "$WORK/ps.txt"
else
  ps -U "$(id -u)" -o pid= -o stat= 2>/dev/null | awk '$2 !~ /^Z/ { print $1 }' > "$WORK/ps.txt"
fi
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
# working directory under the same spelling, and at least 80 % of the user's inspectable processes that
# are still alive (that list was taken before lsof ran).
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
  echo "fleet-clean-rust: lsof gave no usable list of open files (it lists $covered of the $alive processes of yours that I can inspect), so I cannot tell whether a process uses a build folder; deleted nothing" >&2
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

# esc <text>: every byte that is not printable ASCII becomes `?`, so a folder name cannot forge an output line.
esc() { printf '%s' "$1" | tr -c '[:print:]' '?'; }
kept() { n_kept=$((n_kept + 1)); echo "kept $(esc "$1"): $(esc "$2")"; }

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

# keeps_source <dir>: true when its top level holds something that is source, not build output.
keeps_source() { [ -e "$1/.git" ] || [ -e "$1/Cargo.toml" ] || [ -e "$1/src" ] || [ -e "$1/package.json" ]; }

# has_cargo_tag <dir>: a regular CACHEDIR.TAG written by cargo.
has_cargo_tag() {
  [ -f "$1/CACHEDIR.TAG" ] && [ ! -L "$1/CACHEDIR.TAG" ] \
    && [ "$(head -n 1 "$1/CACHEDIR.TAG" 2>/dev/null)" = "$SIGNATURE" ] \
    && [ "$(sed -n 2p "$1/CACHEDIR.TAG" 2>/dev/null)" = "$TAG_LINE2" ]
}

# has_git_inside <dir>: a `.git` anywhere inside. git_tracks <dir>: git tracks a file in it.
has_git_inside() { [ -n "$(find "$1" -name .git -prune -print -quit 2>/dev/null)" ]; }
git_tracks() { [ -n "$(git -C "$1" -c core.fsmonitor=false ls-files 2>/dev/null | head -n 1)" ]; }

# devino <dir>: device and inode, to see that a folder is still the one that was checked.
# The system stat by its absolute path: another stat first on PATH (GNU coreutils on macOS) takes other options.
STAT=/usr/bin/stat; [ -x "$STAT" ] || STAT=/bin/stat
UNAME=/usr/bin/uname; [ -x "$UNAME" ] || UNAME=/bin/uname
devino() {
  local out
  if [ "$("$UNAME" 2>/dev/null)" = Linux ]; then out=$("$STAT" -c '%d:%i' -- "$1" 2>/dev/null); else out=$("$STAT" -f '%d:%i' "$1" 2>/dev/null); fi
  case "$out" in ''|*[!0-9:]*|:*|*:|*:*:*) return 1 ;; esac   # only digits:digits counts
  printf '%s' "$out"
}

# bad_path <path>: prints why when a path is not one the checks can be trusted on, and returns 0.
# The in-use check compares text with what lsof prints, and lsof escapes a control character and may
# escape a non-ASCII byte; a `.` or `..` component can lead through a symlink that -L does not see.
bad_path() {
  case "$1" in *[![:print:]]*) echo "its path holds a character that lsof may escape (a control character or a non-ASCII byte)"; return 0 ;; esac
  case "$1/" in */./*|*/../*) echo "its path has a . or .. component"; return 0 ;; esac
  return 1
}

# tree_stray <dir>: one scan of the whole folder tree, at every depth (find -P: a symlink is never
# followed). It allows only what cargo and tauri write, by path shape, and prints the first thing that
# is anything else, as a path inside <dir>, returning 0; it returns 1 when everything is plain build
# output.
#   Anywhere, also in the opaque zones: a symlink, a special file (fifo, socket, device), a name with a
#   control or non-ASCII byte, a `.git` (folder or file) and a `Cargo.toml` are never build output.
#   Top level: CACHEDIR.TAG, .rustc_info.json, .rustdoc_fingerprint.json, .future-incompat-report.json,
#   .cargo-* files; the folders doc, tmp, package, cargo-timings, flycheck*; profile folders (debug,
#   release, and any folder that holds deps or .fingerprint); and target-triple folders, which hold
#   profile folders, a doc folder and CACHEDIR.TAG.
#   Profile folder: build, deps, examples, incremental, .fingerprint, bundle and *.dSYM folders; .cargo-*
#   files; artifact files (*.d *.rlib *.rmeta *.o *.a *.so *.dylib *.dll *.lib *.pdb *.exe *.wasm *.dwo
#   *.dwp *.exp); and executables (no extension, executable bit, not starting with `#!`).
#   deps and examples hold only artifact files, executables and *.dSYM folders.
#   Opaque zones, whose content cargo or a build script writes freely and which are not judged beyond
#   the first rule: doc, tmp, package, cargo-timings, flycheck*, build/<package>/, incremental/*/,
#   .fingerprint/*/, bundle/ and *.dSYM/.
# Known limit: a source tree that a build script writes into build/<package>/out/ is deleted when it holds
# no `.git` and no `Cargo.toml`; so is a source file in tmp/. Nothing outside an opaque zone passes.
tree_stray() {
  local d="$1" hit line rel
  hit=$(find -P "$d" -mindepth 1 \( -type l -o \( ! -type f ! -type d \) -o -name '*[![:print:]]*' \
         -o -name .git -o -name Cargo.toml \) -print -quit 2>/dev/null)
  if [ -n "$hit" ]; then printf '%s' "${hit#"$d"/}"; return 0; fi
  # Every name is printable now, so no name holds a newline or a tab: one path per line is safe.
  hit=$({ find -P "$d" -mindepth 1 -type d -print 2>/dev/null | sed 's/^/d /'
          find -P "$d" -mindepth 1 -type f -print 2>/dev/null | sed 's/^/f /'; } | D="$d" awk '
    function art(n) { return (n ~ /\.(d|rlib|rmeta|o|a|so|dylib|dll|lib|pdb|exe|wasm|dwo|dwp|exp)$/ || n ~ /\.so\.[0-9.]+$/) }
    function stray(r) { print "STRAY " r; found = 1; exit }
    function profile_top(t) { return (type[t "/deps"] == "d" || type[t "/.fingerprint"] == "d") }
    BEGIN { d = ENVIRON["D"] }
    { t = substr($0, 1, 1); rel = substr($0, 3); rel = substr(rel, length(d) + 2); type[rel] = t; paths[++n] = rel
      k = split(rel, c, "/"); if (k == 2 && t == "d") childdir[c[1]] = 1 }
    END {
      if (found) exit
      for (i = 1; i <= n; i++) {
        rel = paths[i]; t = type[rel]; k = split(rel, c, "/"); top = c[1]
        if (k == 1) {
          if (t == "f") { if (!(top ~ /^(CACHEDIR\.TAG|\.rustc_info\.json|\.rustdoc_fingerprint\.json|\.future-incompat-report\.json)$/ || top ~ /^\.cargo-/)) stray(rel); continue }
          if (top ~ /^(doc|tmp|package|cargo-timings)$/ || top ~ /^flycheck/ || top == "debug" || top == "release" || profile_top(top)) continue
          if (!childdir[top]) stray(rel)          # a folder that is neither a profile nor holds one
          continue
        }
        if (top ~ /^(doc|tmp|package|cargo-timings)$/ || top ~ /^flycheck/) continue
        if (top == "debug" || top == "release" || profile_top(top)) pd = 1
        else {                                      # a target-triple folder
          if (k == 2) { if (t == "f" && c[2] != "CACHEDIR.TAG") stray(rel); continue }
          if (c[2] == "doc" || type[top "/" c[2]] != "d") continue
          pd = 2
        }
        e = c[pd + 1]
        if (k == pd + 1) {
          if (t == "d") { if (!(e ~ /^(build|deps|examples|incremental|\.fingerprint|bundle)$/ || e ~ /\.dSYM$/)) stray(rel) }
          else if (e ~ /^\.cargo-/ || art(e)) { }
          else if (e !~ /\./) print "MAGIC " rel
          else stray(rel)
          continue
        }
        if (e ~ /\.dSYM$/ || e == "bundle") continue
        if (e == "incremental" || e == ".fingerprint") { if (k == pd + 2 && t != "d") stray(rel); continue }
        if (e == "build") { if (k == pd + 2 && t == "f" && c[pd + 2] !~ /^\.cargo-/) stray(rel); continue }
        if (e == "deps" || e == "examples") {
          if (k == pd + 2) {
            n2 = c[pd + 2]
            if (t == "d") { if (n2 !~ /\.dSYM$/) stray(rel) }
            else if (art(n2)) { }
            else if (n2 !~ /\./) print "MAGIC " rel
            else stray(rel)
          }
          continue
        }
      }
    }')
  rel=""
  while IFS= read -r line; do
    case "$line" in
      "STRAY "*) rel=${line#STRAY }; break ;;
      "MAGIC "*)
        # An extensionless file is an executable only with the executable bit and no `#!` script start.
        if [ ! -x "$d/${line#MAGIC }" ] || [ "$(head -c 2 "$d/${line#MAGIC }" 2>/dev/null)" = '#!' ]; then rel=${line#MAGIC }; break; fi ;;
    esac
  done <<EOF
$hit
EOF
  [ -n "$rel" ] || return 1
  printf '%s' "$rel"
}

# consider <dir> <found by --root: the canonical root, else empty>
consider() {
  local dir="$1" scanned="$2" p parent why kib r id1 ids stray
  if why=$(bad_path "$dir"); then kept "$dir" "$why"; return; fi
  while [ "${#dir}" -gt 1 ] && [ "${dir%/}" != "$dir" ]; do dir=${dir%/}; done   # `link/` must still be seen as a symlink
  if [ -L "$dir" ]; then kept "$dir" "a symlink"; return; fi
  if [ ! -e "$dir" ]; then
    # Already deleted through another name (a --root scan, then the same folder as --target): not a second report.
    p=$(canon "$(dirname "$dir")") && case "$seen" in *$'\n'"$p/$(basename "$dir")"$'\n'*) return ;; esac
    kept "$dir" "does not exist"; return
  fi
  if [ ! -d "$dir" ]; then kept "$dir" "not a directory"; return; fi
  p=$(canon "$dir") || { kept "$dir" "cannot enter it"; return; }
  if why=$(bad_path "$p"); then kept "$p" "$why"; return; fi
  case "$seen" in *$'\n'"$p"$'\n'*) return ;; esac
  seen="$seen$p"$'\n'
  # A folder that a scan found must lie inside the root it was found under.
  if [ -n "$scanned" ]; then
    case "$p/" in "$scanned"/*) ;; *) kept "$p" "outside the --root it was found under"; return ;; esac
  fi
  if ! id1=$(devino "$p"); then kept "$p" "cannot read its device and inode"; failed=1; return; fi
  if [ "$p" = / ] || [ "$(printf '%s' "$p" | tr -cd / | wc -c | tr -d ' ')" -lt 2 ]; then
    kept "$p" "the root folder or a folder too close to it"; return
  fi
  case "$home_real/" in "$p"/*) kept "$p" "the home folder, or a folder that holds it"; return ;; esac
  for r in ${roots_real[@]+"${roots_real[@]}"}; do
    case "$r/" in "$p"/*) kept "$p" "holds a checkout"; return ;; esac
  done
  if has_git_inside "$p"; then
    kept "$p" "holds a git checkout"; return
  fi
  if keeps_source "$p"; then
    kept "$p" "holds source (.git, Cargo.toml, src or package.json)"; return
  fi
  if ! has_cargo_tag "$p"; then
    kept "$p" "not a cargo target dir (no CACHEDIR.TAG written by cargo)"; return
  fi
  if [ ! -f "$p/.rustc_info.json" ] && [ ! -d "$p/debug" ] && [ ! -d "$p/release" ]; then
    kept "$p" "does not look like build output (no .rustc_info.json, debug or release)"; return
  fi
  # The whole tree, at every depth: anything but what cargo writes keeps the whole folder.
  if stray=$(tree_stray "$p"); then
    kept "$p" "not only build output: $stray is not something cargo writes (or is a symlink, a special file, a .git or a Cargo.toml)"; return
  fi
  if git_tracks "$p"; then
    kept "$p" "git tracks files in it"; return
  fi
  parent=""
  if [ -n "$scanned" ]; then
    parent=$(dirname "$p")
    if [ ! -f "$parent/Cargo.toml" ]; then kept "$p" "no Cargo.toml beside it"; return; fi
  fi
  if why=$(in_use "$p" "$parent"); then kept "$p" "in use: $why"; return; fi
  kib=$(du -sk "$p" 2>/dev/null | cut -f1)
  case "$kib" in ''|*[!0-9]*) kib=0 ;; esac
  if [ "$dry" = 1 ]; then
    echo "would delete $(esc "$p") ($(human "$kib"))"
  else
    # Look again just before the delete: still the same folder (device and inode), a directory, not a
    # symlink, still carrying cargo's tag and no source. Something swapped in during the checks is kept.
    ids=$(devino "$p")
    if [ -L "$p" ] || [ ! -d "$p" ] || [ "$(canon "$p")" != "$p" ] || [ "$ids" != "$id1" ] \
       || keeps_source "$p" || ! has_cargo_tag "$p" || has_git_inside "$p" || git_tracks "$p" || tree_stray "$p" >/dev/null; then
      kept "$p" "changed while I checked it; deleted nothing here"; failed=1; return
    fi
    rm -rf -- "$p"
    if [ -e "$p" ]; then
      echo "FAILED to delete $(esc "$p")" >&2; failed=1; n_kept=$((n_kept + 1)); return
    fi
    echo "deleted $(esc "$p") ($(human "$kib"))"
  fi
  freed_kib=$((freed_kib + kib)); n_done=$((n_done + 1))
}

for r in ${roots[@]+"${roots[@]}"}; do
  while [ "${#r}" -gt 1 ] && [ "${r%/}" != "$r" ]; do r=${r%/}; done   # `link/` must still be seen as a symlink
  if why=$(bad_path "$r"); then kept "$r" "root: $why"; continue; fi
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

finished=1
verb="freed"; [ "$dry" = 1 ] && verb="would free"
echo "fleet-clean-rust: $verb $(human "$freed_kib") in $n_done folder(s), kept $n_kept"
exit "$failed"
