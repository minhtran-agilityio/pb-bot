#!/bin/bash
# plugins/pd-bot/scripts/fleet-clean-rust.sh deletes stale cargo `target/` folders in the fleet clean-up
# (#181) and keeps everything else. Every case builds its own folders under mktemp; nothing outside a
# temp folder is read or deleted. A process that "uses" a folder is a real `sleep` started by the case.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
CL="$ROOT/plugins/pd-bot/scripts/fleet-clean-rust.sh"
SIG='Signature: 8a477f597d28d172789f06886806bc55'
PIDS=()
trap 'for p in ${PIDS[@]+"${PIDS[@]}"}; do kill "$p" 2>/dev/null; done' EXIT

# mk_target <dir> [MiB]: a folder that looks like cargo's output, with a file of the given size.
mk_target() {
  mkdir -p "$1/release/deps"
  printf '%s\n# This file is a cache directory tag created by cargo.\n' "$SIG" > "$1/CACHEDIR.TAG"
  dd if=/dev/zero of="$1/release/deps/libx.rlib" bs=1048576 count="${2:-1}" 2>/dev/null
}
# mk_project <checkout> <app>: <checkout>/apps/<app>/src-tauri with source and a Cargo.toml.
mk_project() {
  mkdir -p "$1/apps/$2/src-tauri/src"
  printf '[package]\nname = "%s"\n' "$2" > "$1/apps/$2/src-tauri/Cargo.toml"
  echo 'fn main() {}' > "$1/apps/$2/src-tauri/src/main.rs"
}
new_dir() { local d; d=$(mktemp -d); _CLEANUP_DIRS+=("$d"); echo "$d"; }
run() { OUT=$("$@" 2>&1); RC=$?; }

if ! command -v lsof >/dev/null 2>&1; then
  t "fleet-clean-rust needs lsof"; skip "lsof not installed"; finish
fi

t "-h prints the usage and exits 0; no folder given and an unknown argument are usage errors (2)"
run bash "$CL" -h; a=$RC
run bash "$CL"; b=$RC
run bash "$CL" --bogus; c=$RC
run bash "$CL" --root; d=$RC
if [ "$a" = 0 ] && [ "$b" = 2 ] && [ "$c" = 2 ] && [ "$d" = 2 ]; then pass; else fail "rc=$a $b $c $d"; fi

t "--root: deletes the target of each app in the checkout, keeps source and every other file, prints the freed size"
W=$(new_dir); mk_project "$W" pod-board; mk_project "$W" pod-shape
mk_target "$W/apps/pod-board/src-tauri/target" 3; mk_target "$W/apps/pod-shape/src-tauri/target" 2
echo keep > "$W/apps/pod-board/src-tauri/target-notes.md"
run bash "$CL" --root "$W"
if [ "$RC" = 0 ] && [ ! -e "$W/apps/pod-board/src-tauri/target" ] && [ ! -e "$W/apps/pod-shape/src-tauri/target" ] \
   && [ -f "$W/apps/pod-board/src-tauri/Cargo.toml" ] && [ -f "$W/apps/pod-shape/src-tauri/src/main.rs" ] \
   && [ -f "$W/apps/pod-board/src-tauri/target-notes.md" ] \
   && echo "$OUT" | grep -q 'deleted .*pod-board/src-tauri/target (3\.' \
   && echo "$OUT" | grep -q 'fleet-clean-rust: freed 5\.[0-9] MiB in 2 folder(s), kept 0'; then pass; else fail "rc=$RC out: $OUT"; fi

t "--dry-run deletes nothing and says what it would free"
W=$(new_dir); mk_project "$W" a; mk_target "$W/apps/a/src-tauri/target" 2
run bash "$CL" --dry-run --root "$W"
if [ "$RC" = 0 ] && [ -f "$W/apps/a/src-tauri/target/release/deps/libx.rlib" ] \
   && echo "$OUT" | grep -q 'would delete' && echo "$OUT" | grep -q 'would free 2\.'; then pass; else fail "rc=$RC out: $OUT"; fi

t "--root keeps a target folder of another tool (no CACHEDIR.TAG), even with a Cargo.toml beside it"
W=$(new_dir); mk_project "$W" a; mkdir -p "$W/apps/a/src-tauri/target/classes"; echo x > "$W/apps/a/src-tauri/target/classes/Foo.class"
run bash "$CL" --root "$W"
if [ "$RC" = 0 ] && [ -f "$W/apps/a/src-tauri/target/classes/Foo.class" ] && echo "$OUT" | grep -q 'not a cargo target dir'; then pass; else fail "rc=$RC out: $OUT"; fi

t "--root keeps a CACHEDIR.TAG with another signature, and a CACHEDIR.TAG that is a symlink"
W=$(new_dir); mk_project "$W" a; mk_project "$W" b
mk_target "$W/apps/a/src-tauri/target"; printf 'Signature: 0000\n' > "$W/apps/a/src-tauri/target/CACHEDIR.TAG"
mk_target "$W/apps/b/src-tauri/target"; printf '%s\n' "$SIG" > "$W/real-tag"; rm "$W/apps/b/src-tauri/target/CACHEDIR.TAG"; ln -s "$W/real-tag" "$W/apps/b/src-tauri/target/CACHEDIR.TAG"
run bash "$CL" --root "$W"
if [ "$RC" = 0 ] && [ -d "$W/apps/a/src-tauri/target" ] && [ -d "$W/apps/b/src-tauri/target" ]; then pass; else fail "rc=$RC out: $OUT"; fi

t "--root keeps a cargo target with no Cargo.toml beside it"
W=$(new_dir); mkdir -p "$W/apps/c/src-tauri"; mk_target "$W/apps/c/src-tauri/target"
run bash "$CL" --root "$W"
if [ "$RC" = 0 ] && [ -d "$W/apps/c/src-tauri/target" ] && echo "$OUT" | grep -q 'no Cargo.toml beside it'; then pass; else fail "rc=$RC out: $OUT"; fi

t "--root does not look inside node_modules or .git"
W=$(new_dir); mk_project "$W" a
mkdir -p "$W/node_modules/x" "$W/.git/y"; printf '[package]\n' > "$W/node_modules/x/Cargo.toml"; printf '[package]\n' > "$W/.git/y/Cargo.toml"
mk_target "$W/node_modules/x/target"; mk_target "$W/.git/y/target"
run bash "$CL" --root "$W"
if [ "$RC" = 0 ] && [ -d "$W/node_modules/x/target" ] && [ -d "$W/.git/y/target" ]; then pass; else fail "rc=$RC out: $OUT"; fi

t "--target: deletes a CARGO_TARGET_DIR outside any workspace (no Cargo.toml beside it needed)"
S=$(new_dir); mk_target "$S/scratch-ct" 2
run bash "$CL" --target "$S/scratch-ct"
if [ "$RC" = 0 ] && [ ! -e "$S/scratch-ct" ] && [ -d "$S" ] && echo "$OUT" | grep -q 'freed 2\.[0-9] MiB in 1 folder'; then pass; else fail "rc=$RC out: $OUT"; fi

t "--target keeps a folder that is not a cargo target dir, one that holds source, and a missing one"
S=$(new_dir); mkdir -p "$S/plain" "$S/src-holder/src"; echo x > "$S/plain/f"
mk_target "$S/src-holder"; echo '[package]' > "$S/src-holder/Cargo.toml"
run bash "$CL" --target "$S/plain" --target "$S/src-holder" --target "$S/nope"
if [ "$RC" = 0 ] && [ -f "$S/plain/f" ] && [ -f "$S/src-holder/Cargo.toml" ] && [ -d "$S/src-holder/release" ] \
   && echo "$OUT" | grep -q 'does not exist' && echo "$OUT" | grep -q 'holds source'; then pass; else fail "rc=$RC out: $OUT"; fi

t "--target keeps a symlink and leaves the folder it points to untouched"
S=$(new_dir); mk_target "$S/real"; ln -s "$S/real" "$S/link"
run bash "$CL" --target "$S/link"
if [ "$RC" = 0 ] && [ -L "$S/link" ] && [ -f "$S/real/release/deps/libx.rlib" ] && echo "$OUT" | grep -q 'a symlink'; then pass; else fail "rc=$RC out: $OUT"; fi

t "--target keeps the home folder, and a folder that holds a --root checkout, even when it carries the cargo tag"
S=$(new_dir); mk_target "$S"                       # $S itself looks like a cargo target dir
mkdir -p "$S/checkout/apps"; printf '%s\n' "x" > "$S/checkout/apps/keep"
run env HOME="$S" bash "$CL" --target "$S"
a=$RC; o1=$OUT
run bash "$CL" --root "$S/checkout" --target "$S"
if [ "$a" = 0 ] && [ "$RC" = 0 ] && [ -f "$S/checkout/apps/keep" ] && [ -f "$S/CACHEDIR.TAG" ] \
   && echo "$o1" | grep -q 'home folder' && echo "$OUT" | grep -q 'holds a checkout'; then pass; else fail "rc=$a/$RC out: $o1 / $OUT"; fi

t "the same folder given twice (by --root and --target) is deleted once and counted once"
W=$(new_dir); mk_project "$W" a; mk_target "$W/apps/a/src-tauri/target" 2
run bash "$CL" --root "$W" --root "$W" --target "$W/apps/a/src-tauri/target"
if [ "$RC" = 0 ] && [ ! -e "$W/apps/a/src-tauri/target" ] && echo "$OUT" | grep -q 'in 1 folder(s), kept 0'; then pass; else fail "rc=$RC out: $OUT"; fi

t "keeps a target while a process has a file open inside it, then deletes it once the process is gone"
W=$(new_dir); mk_project "$W" a; mk_target "$W/apps/a/src-tauri/target"
( exec 3<"$W/apps/a/src-tauri/target/release/deps/libx.rlib"; exec sleep 60 ) & P=$!; PIDS+=("$P"); sleep 1
run bash "$CL" --root "$W"
kept_rc=$RC; kept_out=$OUT; still=0; [ -d "$W/apps/a/src-tauri/target" ] && still=1
kill "$P" 2>/dev/null; wait "$P" 2>/dev/null
run bash "$CL" --root "$W"
if [ "$kept_rc" = 0 ] && [ "$still" = 1 ] && echo "$kept_out" | grep -q 'in use: sleep' && [ ! -e "$W/apps/a/src-tauri/target" ]; then pass; else fail "kept_rc=$kept_rc still=$still out: $kept_out / $OUT"; fi

t "keeps a --target folder while a process works inside it (its working directory)"
S=$(new_dir); mk_target "$S/ct"
( cd "$S/ct/release" && exec sleep 60 ) & P=$!; PIDS+=("$P"); sleep 1
run bash "$CL" --target "$S/ct"
if [ "$RC" = 0 ] && [ -d "$S/ct/release" ] && echo "$OUT" | grep -q 'in use'; then pass; else fail "rc=$RC out: $OUT"; fi
kill "$P" 2>/dev/null; wait "$P" 2>/dev/null

t "keeps a project's target while a cargo process builds in that project, and deletes the other project's"
W=$(new_dir); mk_project "$W" busy; mk_project "$W" idle
mk_target "$W/apps/busy/src-tauri/target"; mk_target "$W/apps/idle/src-tauri/target"
# lsof names a process after its real executable, so the stand-in is a copy of sleep called cargo
# (a macOS copy of a system binary needs an ad hoc signature to run).
BIN=$(new_dir); cp "$(command -v sleep)" "$BIN/cargo"; command -v codesign >/dev/null 2>&1 && codesign -f -s - "$BIN/cargo" >/dev/null 2>&1
( cd "$W/apps/busy/src-tauri" && exec "$BIN/cargo" 60 ) & P=$!; PIDS+=("$P"); sleep 1
run bash "$CL" --root "$W"
if [ "$RC" = 0 ] && [ -d "$W/apps/busy/src-tauri/target" ] && [ ! -e "$W/apps/idle/src-tauri/target" ] \
   && echo "$OUT" | grep -q 'is building in'; then pass; else fail "rc=$RC out: $OUT"; fi
kill "$P" 2>/dev/null; wait "$P" 2>/dev/null

t "without lsof it deletes nothing and exits 1"
W=$(new_dir); mk_project "$W" a; mk_target "$W/apps/a/src-tauri/target"
run env PATH=/nonexistent-bin /bin/bash "$CL" --root "$W"
if [ "$RC" = 1 ] && [ -d "$W/apps/a/src-tauri/target" ] && echo "$OUT" | grep -q 'lsof'; then pass; else fail "rc=$RC out: $OUT"; fi

# The docs that tell the orchestrator and the sessions what to do.
POD="$ROOT/plugins/pd-bot/skills/pd-orchestrate/references/podbay.md"
RUN="$ROOT/plugins/pd-bot/skills/pd-orchestrate/references/run.md"
SK="$ROOT/plugins/pd-bot/skills/pd-orchestrate/SKILL.md"
TB="$ROOT/plugins/pd-bot/skills/pd-work/references/toolbox.md"
CLEAN=$(awk '$0=="## Clean up"{on=1;next} on&&/^## /{exit} on' "$POD")

t "podbay.md, Clean up: runs fleet-clean-rust.sh on the main checkout, on each kept workspace and on each reported CARGO_TARGET_DIR"
if echo "$CLEAN" | grep -q 'fleet-clean-rust.sh' && echo "$CLEAN" | grep -q -- '--root' && echo "$CLEAN" | grep -q -- '--target' \
   && echo "$CLEAN" | grep -qi 'kept workspace\|workspace that .ws-rm. refused\|ws-rm. refuses' && echo "$CLEAN" | grep -q 'CARGO_TARGET_DIR'; then pass; else fail "$CLEAN"; fi

t "orchestrate SKILL.md allows exactly that script, and stays under 600 words"
if grep -qF 'Bash(bash ${CLAUDE_PLUGIN_ROOT}/scripts/fleet-clean-rust.sh *)' "$SK" && [ "$(wc -w < "$SK")" -lt 600 ]; then pass; else fail "allowed-tools or words=$(wc -w < "$SK")"; fi

t "run.md, the brief: a session that builds Rust keeps CARGO_TARGET_DIR inside its workspace or reports where it is"
if awk '$0=="## The brief you send"{on=1;next} on&&/^## /{exit} on' "$RUN" | grep -q 'CARGO_TARGET_DIR'; then pass; else fail "no CARGO_TARGET_DIR in the brief section"; fi

t "worker toolbox: the same rule, and where to report it"
if grep -q 'CARGO_TARGET_DIR' "$TB" && grep -q 'cargo_target_dirs' "$TB"; then pass; else fail "toolbox"; fi

finish
