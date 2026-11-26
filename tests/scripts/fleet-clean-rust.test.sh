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

TAG2='# This file is a cache directory tag created by cargo.'
# mk_bin <dir> <name>: a program called <name> that sleeps for a minute. lsof names a process after its
# real executable, so a shell script or a renamed copy of sleep would not do (on some systems sleep is a
# multi-call binary that needs its own name): compile a tiny one, else copy sleep (a macOS copy of a
# system binary needs an ad hoc signature to run).
mk_bin() {
  mkdir -p "$1"
  if command -v cc >/dev/null 2>&1; then
    printf '#include <unistd.h>\nint main(void) { sleep(60); return 0; }\n' > "$1/$2.c"
    cc -o "$1/$2" "$1/$2.c" >/dev/null 2>&1 && return 0
  fi
  cp "$(command -v sleep)" "$1/$2"; command -v codesign >/dev/null 2>&1 && codesign -f -s - "$1/$2" >/dev/null 2>&1
}
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
mk_target "$W/apps/b/src-tauri/target"; printf '%s\n%s\n' "$SIG" "$TAG2" > "$W/real-tag"; rm "$W/apps/b/src-tauri/target/CACHEDIR.TAG"; ln -s "$W/real-tag" "$W/apps/b/src-tauri/target/CACHEDIR.TAG"
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
BIN=$(new_dir); mk_bin "$BIN" cargo   # a real program named cargo, as lsof names it
( cd "$W/apps/busy/src-tauri" && exec "$BIN/cargo" 60 ) & P=$!; PIDS+=("$P"); sleep 1
run bash "$CL" --root "$W"
if [ "$RC" = 0 ] && [ -d "$W/apps/busy/src-tauri/target" ] && [ ! -e "$W/apps/idle/src-tauri/target" ] \
   && echo "$OUT" | grep -q 'is building in'; then pass; else fail "rc=$RC out: $OUT"; fi
kill "$P" 2>/dev/null; wait "$P" 2>/dev/null

t "without lsof it deletes nothing and exits 1"
W=$(new_dir); mk_project "$W" a; mk_target "$W/apps/a/src-tauri/target"
run env PATH=/nonexistent-bin /bin/bash "$CL" --root "$W"
if [ "$RC" = 1 ] && [ -d "$W/apps/a/src-tauri/target" ] && echo "$OUT" | grep -q 'lsof'; then pass; else fail "rc=$RC out: $OUT"; fi

t "a relative --root or --target is a usage error (2): a value from a report is not trusted"
S=$(new_dir); mk_target "$S/ct"
run bash -c "cd '$S' && bash '$CL' --target ct"; a=$RC
run bash -c "cd '$S' && bash '$CL' --root ."; b=$RC
if [ "$a" = 2 ] && [ "$b" = 2 ] && [ -d "$S/ct/release" ]; then pass; else fail "rc=$a $b"; fi

t "--target keeps a folder with cargo's tag that holds other checkouts (a .git at any of the first 4 levels), and one that is not build output"
S=$(new_dir); mk_target "$S/c"; mkdir -p "$S/c/proj1/.git" "$S/c/proj2"; echo work > "$S/c/proj2/notes"
mkdir -p "$S/d"; printf '%s\n%s\n' "$SIG" "$TAG2" > "$S/d/CACHEDIR.TAG"; mkdir -p "$S/d/other"; echo work > "$S/d/other/f"
run bash "$CL" --target "$S/c" --target "$S/d"
if [ "$RC" = 0 ] && [ -d "$S/c/proj1/.git" ] && [ -f "$S/c/proj2/notes" ] && [ -f "$S/d/other/f" ] \
   && echo "$OUT" | grep -q 'holds a git checkout' && echo "$OUT" | grep -q 'does not look like build output'; then pass; else fail "rc=$RC out: $OUT"; fi

t "--target keeps a folder that holds the home folder, even when it carries the cargo tag"
S=$(new_dir); mk_target "$S"; mkdir -p "$S/home/user"
run env HOME="$S/home/user" bash "$CL" --target "$S"
if [ "$RC" = 0 ] && [ -f "$S/CACHEDIR.TAG" ] && [ -d "$S/home/user" ] && echo "$OUT" | grep -q 'holds it'; then pass; else fail "rc=$RC out: $OUT"; fi

t "--target keeps a symlink written with a trailing slash, and the folder it points to"
S=$(new_dir); mk_target "$S/real"; ln -s "$S/real" "$S/link"
run bash "$CL" --target "$S/link/"
if [ "$RC" = 0 ] && [ -L "$S/link" ] && [ -f "$S/real/release/deps/libx.rlib" ]; then pass; else fail "rc=$RC out: $OUT"; fi

t "keeps a folder whose path holds a backslash while a process works inside it"
S=$(new_dir); mk_target "$S/b\\tx"
( cd "$S/b\\tx/release" && exec sleep 60 ) & P=$!; PIDS+=("$P"); sleep 1
run bash "$CL" --target "$S/b\\tx"
if [ "$RC" = 0 ] && [ -d "$S/b\\tx/release" ] && echo "$OUT" | grep -q 'in use'; then pass; else fail "rc=$RC out: $OUT"; fi
kill "$P" 2>/dev/null; wait "$P" 2>/dev/null

t "--target keeps a shared folder: the cargo tag and release are there, but so are other files or a source tree"
S=$(new_dir); mk_target "$S/b"; mkdir -p "$S/b/Documents"; echo tax > "$S/b/Documents/tax.pdf"
mk_target "$S/f"; mkdir -p "$S/f/proj/src"; echo '[package]' > "$S/f/proj/Cargo.toml"
mk_target "$S/e"; mkdir -p "$S/e/a/b/c/d/proj/.git"
run bash "$CL" --target "$S/b" --target "$S/f" --target "$S/e"
if [ "$RC" = 0 ] && [ -f "$S/b/Documents/tax.pdf" ] && [ -f "$S/f/proj/Cargo.toml" ] && [ -d "$S/e/a/b/c/d/proj/.git" ] \
   && [ "$(echo "$OUT" | grep -c 'not only build output')" -ge 2 ]; then pass; else fail "rc=$RC out: $OUT"; fi

t "--target deletes a real cargo layout: a profile folder, a target-triple folder, doc, .rustc_info.json"
S=$(new_dir); mk_target "$S/ct"; mkdir -p "$S/ct/release-lto/deps" "$S/ct/aarch64-apple-darwin/release/deps" "$S/ct/doc"; echo '{}' > "$S/ct/.rustc_info.json"
run bash "$CL" --target "$S/ct"
if [ "$RC" = 0 ] && [ ! -e "$S/ct" ] && echo "$OUT" | grep -q '^deleted '; then pass; else fail "rc=$RC out: $OUT"; fi

t "an lsof that lists only this script's own process is not a usable list: deletes nothing, exits 1"
W=$(new_dir); mk_project "$W" a; mk_target "$W/apps/a/src-tauri/target"
FAKEBIN=$(new_dir); printf '#!/bin/sh\nprintf "p%s\\ncfake\\nfcwd\\nn/\\n" "$PPID"\n' > "$FAKEBIN/lsof"; chmod +x "$FAKEBIN/lsof"
run env PATH="$FAKEBIN:$PATH" bash "$CL" --root "$W"
if [ "$RC" = 1 ] && [ -d "$W/apps/a/src-tauri/target" ] && echo "$OUT" | grep -q 'no usable list'; then pass; else fail "rc=$RC out: $OUT"; fi

t "--target keeps a folder whose path holds a control character (lsof escapes it, so the in-use check could not match)"
S=$(new_dir); mk_target "$S/tab"$'\t'"x"
run bash "$CL" --target "$S/tab"$'\t'"x"
if [ "$RC" = 0 ] && [ -d "$S/tab"$'\t'"x/release" ] && echo "$OUT" | grep -q 'control character'; then pass; else fail "rc=$RC out: $OUT"; fi

t "an lsof that fails with no output: deletes nothing and exits 1 (a missing list is not 'nothing uses it')"
W=$(new_dir); mk_project "$W" a; mk_target "$W/apps/a/src-tauri/target"
FAKEBIN=$(new_dir); printf '#!/bin/sh\nexit 1\n' > "$FAKEBIN/lsof"; chmod +x "$FAKEBIN/lsof"
run env PATH="$FAKEBIN:$PATH" bash "$CL" --root "$W"
if [ "$RC" = 1 ] && [ -d "$W/apps/a/src-tauri/target" ] && echo "$OUT" | grep -q 'no usable list'; then pass; else fail "rc=$RC out: $OUT"; fi

t "inside a git repo: keeps a committed fake target (git tracks files in it) and deletes the ignored real one"
if ! command -v git >/dev/null 2>&1; then skip "git not installed"; else
G=$(new_dir); git -C "$G" init -q; git -C "$G" config user.email t@t; git -C "$G" config user.name t
mk_project "$G" real; mk_project "$G" fake
mk_target "$G/apps/real/src-tauri/target"; printf 'target/\n' > "$G/apps/real/src-tauri/.gitignore"
mk_target "$G/apps/fake/src-tauri/target"
git -C "$G" add -A >/dev/null 2>&1; git -C "$G" commit -qm init
run bash "$CL" --root "$G"
if [ "$RC" = 0 ] && [ ! -e "$G/apps/real/src-tauri/target" ] && [ -f "$G/apps/fake/src-tauri/target/release/deps/libx.rlib" ] \
   && echo "$OUT" | grep -q 'git tracks files in it'; then pass; else fail "rc=$RC out: $OUT"; fi
fi

t "F1: a newline in a folder name under --root cannot make the script delete a target outside --root"
# The name "sub<NL>victim" under root2 holds a target; cut at the newline, its second half reads as the
# relative path victim/target, which lies outside root2 when the script runs in $D.
D=$(new_dir); mkdir -p "$D/root2"
mk_target "$D/victim/target" 1; printf '[package]\n' > "$D/victim/Cargo.toml"
mk_target "$D/root2/sub"$'\n'"victim/target" 1; printf '[package]\n' > "$D/root2/sub"$'\n'"victim/Cargo.toml"
run bash -c "cd '$D' && bash '$CL' --dry-run --root '$D/root2'"
a=$OUT; arc=$RC
run bash -c "cd '$D' && bash '$CL' --root '$D/root2'"
if [ "$arc" = 0 ] && [ "$RC" = 0 ] && [ -d "$D/victim/target/release" ] && ! echo "$a" | grep -q "would delete .*/victim/target"; then pass; else fail "dry rc=$arc: $a / real rc=$RC: $OUT"; fi

t "F2: a different spelling of the path (case) still meets the in-use check"
S=$(new_dir); mk_target "$S/Proj/Target"
( cd "$S/Proj/Target/release" && exec sleep 60 ) & P=$!; PIDS+=("$P"); sleep 1
run bash "$CL" --dry-run --target "$S/proj/target"
# On a case-sensitive disk proj/target does not exist; on a case-insensitive one it is the same folder and is in use.
if [ "$RC" = 0 ] && ! echo "$OUT" | grep -q 'would delete'; then pass; else fail "rc=$RC out: $OUT"; fi
kill "$P" 2>/dev/null; wait "$P" 2>/dev/null

t "F3: an lsof list with only this script and one other process is not enough: deletes nothing, exits 1"
W=$(new_dir); mk_project "$W" a; mk_target "$W/apps/a/src-tauri/target"
( cd "$W/apps/a/src-tauri/target/release" && exec sleep 60 ) & P=$!; PIDS+=("$P"); sleep 1
FAKEBIN=$(new_dir)
cat > "$FAKEBIN/lsof" <<'EOF'
#!/bin/sh
# Sees the caller's cwd (spelled right) and itself, and nothing else.
printf 'p%s\ncfake\nfcwd\nn%s\n' "$PPID" "$(/bin/pwd -P)"
printf 'p%s\ncfake\nfcwd\nn/\n' "$$"
EOF
chmod +x "$FAKEBIN/lsof"
run env PATH="$FAKEBIN:$PATH" bash "$CL" --root "$W"
if [ "$RC" = 1 ] && [ -d "$W/apps/a/src-tauri/target/release" ] && echo "$OUT" | grep -q 'no usable list'; then pass; else fail "rc=$RC out: $OUT"; fi
kill "$P" 2>/dev/null; wait "$P" 2>/dev/null

t "F4: a --root symlink written with a trailing slash is not followed"
D=$(new_dir); mk_project "$D/elsewhere" p; mk_target "$D/elsewhere/apps/p/src-tauri/target"; ln -s "$D/elsewhere" "$D/rootlink"
run bash "$CL" --root "$D/rootlink/"
if [ "$RC" = 0 ] && [ -d "$D/elsewhere/apps/p/src-tauri/target/release" ] && echo "$OUT" | grep -q 'root is not a directory'; then pass; else fail "rc=$RC out: $OUT"; fi

t "F5: HOME unset: deletes nothing, exits 1"
S=$(new_dir); mk_target "$S/ct"
run env -u HOME bash "$CL" --target "$S/ct"
if [ "$RC" = 1 ] && [ -d "$S/ct/release" ] && echo "$OUT" | grep -q 'HOME'; then pass; else fail "rc=$RC out: $OUT"; fi

t "F6: a .git deep inside doc/ (past 4 levels) keeps the folder"
S=$(new_dir); mk_target "$S/ct"; mkdir -p "$S/ct/doc/a/b/c/d/e/proj/.git"
run bash "$CL" --target "$S/ct"
if [ "$RC" = 0 ] && [ -d "$S/ct/doc/a/b/c/d/e/proj/.git" ] && echo "$OUT" | grep -q 'holds a git checkout'; then pass; else fail "rc=$RC out: $OUT"; fi

t "F8: a compiler or linker working in the project keeps its target"
W=$(new_dir); mk_project "$W" busy; mk_target "$W/apps/busy/src-tauri/target"
BIN=$(new_dir); mk_bin "$BIN" clang
( cd "$W/apps/busy/src-tauri" && exec "$BIN/clang" 60 ) & P=$!; PIDS+=("$P"); sleep 1
run bash "$CL" --root "$W"
if [ "$RC" = 0 ] && [ -d "$W/apps/busy/src-tauri/target" ] && echo "$OUT" | grep -q 'is building in'; then pass; else fail "rc=$RC out: $OUT"; fi
kill "$P" 2>/dev/null; wait "$P" 2>/dev/null

t "F10: a CACHEDIR.TAG with the shared signature but not written by cargo is kept"
S=$(new_dir); mk_target "$S/ct"; printf '%s\n# Written by some other tool.\n' "$SIG" > "$S/ct/CACHEDIR.TAG"
run bash "$CL" --target "$S/ct"
if [ "$RC" = 0 ] && [ -d "$S/ct/release" ] && echo "$OUT" | grep -q 'not a cargo target dir'; then pass; else fail "rc=$RC out: $OUT"; fi

t "F13: a target whose folder name holds a newline is kept even while a process works in it, and the folder after it is still processed"
R=$(new_dir); NL=$'\n'
mk_target "$R/q${NL}z/target" 1; printf '[package]\n' > "$R/q${NL}z/Cargo.toml"
mk_project "$R" zz; mk_target "$R/apps/zz/src-tauri/target" 1
mk_project "$R" yy; mk_target "$R/apps/yy/src-tauri/target" 1
( cd "$R/q${NL}z/target/release" && exec sleep 60 ) & P=$!; PIDS+=("$P"); sleep 1
run bash "$CL" --root "$R"
if [ "$RC" = 0 ] && [ -d "$R/q${NL}z/target/release" ] && [ ! -e "$R/apps/zz/src-tauri/target" ] && [ ! -e "$R/apps/yy/src-tauri/target" ] \
   && echo "$OUT" | grep -q 'may escape' && echo "$OUT" | grep -q '^fleet-clean-rust: freed .* in 2 folder(s), kept 1$'; then pass; else fail "rc=$RC out: $OUT"; fi
kill "$P" 2>/dev/null; wait "$P" 2>/dev/null

t "F15: a non-ASCII path is kept, with a reason that names what lsof may escape"
S=$(new_dir); mk_target "$S/caf"$'\xc3\xa9'
run bash "$CL" --target "$S/caf"$'\xc3\xa9'
if [ "$RC" = 0 ] && [ -d "$S/caf"$'\xc3\xa9'/release ] && echo "$OUT" | grep -q 'may escape (a control character or a non-ASCII byte)'; then pass; else fail "rc=$RC out: $OUT"; fi

t "F14: a --root or --target with a . or .. component (a way through a symlink) is kept"
D=$(new_dir); mkdir -p "$D/ws"; mk_project "$D/elsewhere" p; mk_target "$D/elsewhere/apps/p/src-tauri/target"; ln -s "$D/elsewhere" "$D/ws/link"
mk_target "$D/real"; mkdir -p "$D/other"
run bash "$CL" --root "$D/ws/link/." --root "$D/ws/link/x/.." --target "$D/other/../real"
if [ "$RC" = 0 ] && [ -d "$D/elsewhere/apps/p/src-tauri/target/release" ] && [ -d "$D/real/release" ] \
   && [ "$(echo "$OUT" | grep -c 'a . or .. component')" = 3 ]; then pass; else fail "rc=$RC out: $OUT"; fi

# F7: du runs between the checks and the delete. A fake du that swaps the folder at that moment stands in
# for another process doing the same.
mk_fake_du() { # mk_fake_du <bin dir> <swap script body: $p is the folder>
  mkdir -p "$1"; { echo '#!/bin/sh'; echo 'p=$2'; echo "$2"; echo 'exec /usr/bin/du "$@"'; } > "$1/du"; chmod +x "$1/du"; }
t "F7: a folder with source swapped in for the target after the checks is not deleted, and the exit is 1"
S=$(new_dir); mk_target "$S/ct"; FB=$(new_dir)
mk_fake_du "$FB" 'mv "$p" "$p.old" && mkdir -p "$p/src" && echo precious > "$p/src/main.rs"'
run env PATH="$FB:$PATH" bash "$CL" --target "$S/ct"
if [ "$RC" = 1 ] && [ "$(cat "$S/ct/src/main.rs" 2>/dev/null)" = precious ] && echo "$OUT" | grep -q 'changed while I checked it'; then pass; else fail "rc=$RC out: $OUT"; fi

t "F7: a look-alike target (same content, new inode) swapped in after the checks is not deleted either"
S=$(new_dir); mk_target "$S/ct"; FB=$(new_dir)
mk_fake_du "$FB" 'cp -R "$p" "$p.copy" && mv "$p" "$p.old" && mv "$p.copy" "$p"'
run env PATH="$FB:$PATH" bash "$CL" --target "$S/ct"
if [ "$RC" = 1 ] && [ -d "$S/ct/release" ] && echo "$OUT" | grep -q 'changed while I checked it'; then pass; else fail "rc=$RC out: $OUT"; fi

t "F16: another stat first on PATH (GNU stat on macOS, other options) does not break the inode check"
S=$(new_dir); mk_target "$S/ct"; FB=$(new_dir)
# A different value on each call, so a script that used this stat would see the folder "change".
cat > "$FB/stat" <<EOF
#!/bin/sh
n=\$(cat "$FB/n" 2>/dev/null || echo 0); n=\$((n + 1)); echo "\$n" > "$FB/n"; echo "\$n:1"
EOF
chmod +x "$FB/stat"
run env PATH="$FB:$PATH" bash "$CL" --target "$S/ct"
if [ "$RC" = 0 ] && [ ! -e "$S/ct" ] && echo "$OUT" | grep -q '^deleted '; then pass; else fail "rc=$RC out: $OUT"; fi

t "F17: it works from a working folder with a non-ASCII name"
S=$(new_dir); mk_target "$S/ct"; WD="$S/d"$'\xe1\xbb\xb1'" "$'\xc3\xa1'"n"; mkdir -p "$WD"
run bash -c "cd '$WD' && bash '$CL' --target '$S/ct'"
if [ "$RC" = 0 ] && [ ! -e "$S/ct" ] && echo "$OUT" | grep -q '^deleted '; then pass; else fail "rc=$RC out: $OUT"; fi

t "F18: a .git created deep inside the target after the checks (while du runs) keeps the folder, exit 1"
S=$(new_dir); mk_target "$S/ct"; FB=$(new_dir)
mk_fake_du "$FB" 'mkdir -p "$p/release/deps/x/y/.git"'
run env PATH="$FB:$PATH" bash "$CL" --target "$S/ct"
if [ "$RC" = 1 ] && [ -d "$S/ct/release/deps/x/y/.git" ] && echo "$OUT" | grep -q 'changed while I checked it'; then pass; else fail "rc=$RC out: $OUT"; fi

t "F19: a folder name with a newline cannot forge an output line"
S=$(new_dir); NL=$'\n'; mk_target "$S/ct${NL}deleted /etc (9.9 GiB)"
run bash "$CL" --target "$S/ct${NL}deleted /etc (9.9 GiB)"
if [ "$RC" = 0 ] && ! echo "$OUT" | grep -q '^deleted /etc' && echo "$OUT" | grep -q '^kept .*ct?deleted /etc'; then pass; else fail "rc=$RC out: $OUT"; fi

t "Linux: processes of the user that lsof cannot inspect (not dumpable) and zombies do not make a full lsof list look partial"
if [ "$(uname)" != Linux ] || ! command -v cc >/dev/null 2>&1; then skip "needs Linux and a C compiler (prctl)"; else
NB=$(new_dir)
cat > "$NB/nodump.c" <<'EOF'
#include <sys/prctl.h>
#include <unistd.h>
int main(void) { prctl(PR_SET_DUMPABLE, 0); sleep(60); return 0; }
EOF
cat > "$NB/zomb.c" <<'EOF'
#include <unistd.h>
int main(void) { if (fork() == 0) _exit(0); sleep(60); return 0; }
EOF
if cc -o "$NB/nodump" "$NB/nodump.c" && cc -o "$NB/zomb" "$NB/zomb.c"; then
  for _ in $(seq 40); do "$NB/nodump" & PIDS+=("$!"); done
  for _ in $(seq 20); do "$NB/zomb" & PIDS+=("$!"); done
  sleep 1
  S=$(new_dir); mk_target "$S/ct"
  run bash "$CL" --target "$S/ct"
  if [ "$RC" = 0 ] && [ ! -e "$S/ct" ] && echo "$OUT" | grep -q '^deleted '; then pass; else fail "rc=$RC out: $OUT"; fi
else fail "could not compile the helper programs"; fi
fi

t "debug and release: an empty profile folder is deleted with the target; real cargo files (executable, .d, rlib, dSYM, bundle) are fine"
S=$(new_dir); mk_target "$S/ct"; rm -rf "$S/ct/release"; mkdir -p "$S/ct/debug" "$S/ct/release"
mk_target "$S/ct2"; mkdir -p "$S/ct2/release/bundle/macos" "$S/ct2/release/build" "$S/ct2/release/app.dSYM" "$S/ct2/x86_64-unknown-linux-gnu/release/deps"
printf 'bin' > "$S/ct2/release/app"; chmod +x "$S/ct2/release/app"; echo 'x' > "$S/ct2/release/app.d"; echo 'x' > "$S/ct2/release/libapp.rlib"; echo 'x' > "$S/ct2/release/libapp.dylib"; : > "$S/ct2/release/.cargo-lock"; : > "$S/ct2/release/.cargo-build-lock"; : > "$S/ct2/release/.cargo-artifact-lock"
run bash "$CL" --target "$S/ct" --target "$S/ct2"
if [ "$RC" = 0 ] && [ ! -e "$S/ct" ] && [ ! -e "$S/ct2" ] && echo "$OUT" | grep -q 'in 2 folder(s), kept 0'; then pass; else fail "rc=$RC out: $OUT"; fi

t "debug and release: a non-cargo file (.rs, a README, a non-executable file with no extension) or a source folder inside keeps the whole target"
S=$(new_dir)
mk_target "$S/a"; mkdir -p "$S/a/debug"; echo 'fn main() {}' > "$S/a/debug/evil.rs"
mk_target "$S/b"; echo doc > "$S/b/release/README.md"
mk_target "$S/c"; echo x > "$S/c/release/LICENSE"
mk_target "$S/d"; mkdir -p "$S/d/release/src"
mk_target "$S/e"; mkdir -p "$S/e/aarch64-apple-darwin/release/deps"; echo 'fn main() {}' > "$S/e/aarch64-apple-darwin/release/main.rs"
mk_target "$S/f"; mkdir -p "$S/f/release/.git"
run bash "$CL" --target "$S/a" --target "$S/b" --target "$S/c" --target "$S/d" --target "$S/e" --target "$S/f"
if [ "$RC" = 0 ] && [ -f "$S/a/debug/evil.rs" ] && [ -f "$S/b/release/README.md" ] && [ -f "$S/c/release/LICENSE" ] && [ -d "$S/d/release/src" ] \
   && [ -f "$S/e/aarch64-apple-darwin/release/main.rs" ] && [ -d "$S/f/release/.git" ] \
   && echo "$OUT" | grep -q 'debug/evil.rs is not something cargo writes' && echo "$OUT" | grep -q 'release/main.rs is not something cargo writes' \
   && echo "$OUT" | grep -q 'in 0 folder(s), kept 6'; then pass; else fail "rc=$RC out: $OUT"; fi

# The layouts of the reviewer's attack list: a target with one addition each. KEEP means the whole target
# stays; DELETE means it goes. The opaque zones (build/<package>/, tmp/) are deleted on purpose unless a
# .git, a Cargo.toml, a symlink, a special file or a bad name is in them.
t "every layout: source, a script, a stray folder, a symlink, a fifo, a .git or a Cargo.toml anywhere outside cargo's layout keeps the whole target; cargo's own output is deleted"
LT=$(new_dir); LNAMES=(); LWANT=()
lay() { # lay <name> <KEEP|DELETE> <setup, run with $d = the target>
  local d="$LT/$1"; LNAMES+=("$1"); LWANT+=("$2"); mk_target "$d"; : > "$d/release/deps/libx.rlib"; ( cd "$d" && eval "$3" ); }
lay r1_triple_bypass KEEP 'echo "fn main(){}" > release/main.rs; mkdir release/debug'
lay r2_deps_rs KEEP 'echo "fn main(){}" > release/deps/main.rs'
lay r3_deep_rs DELETE 'mkdir -p release/build/x-1/a/b/c/d/e/f; echo "fn main(){}" > release/build/x-1/a/b/c/d/e/f/lib.rs'
lay r4_out_rs_proj DELETE 'mkdir -p release/build/x-1/out/proj/src; echo "fn main(){}" > release/build/x-1/out/proj/src/main.rs'
lay r5_out_rs_plain DELETE 'mkdir -p release/build/x-1/out; echo "fn main(){}" > release/build/x-1/out/main.rs'
lay r6_python KEEP 'echo "print(1)" > release/tool.py'
lay r7_c_src KEEP 'mkdir release/deps/cproj; echo "int main(){}" > release/deps/cproj/main.c'
lay r8_toml KEEP 'echo "[x]" > release/config.toml'
lay r9_shell_exec KEEP 'printf "#!/bin/sh\necho hi\n" > release/deploy; chmod +x release/deploy'
lay r10_readme_lower KEEP 'echo doc > release/readme.txt'
lay r11_RS_upper KEEP 'echo "fn main(){}" > release/MAIN.RS'
lay r12_symlink_deep KEEP 'ln -s /etc release/deps/link'
lay r13_newline KEEP 'echo "fn main(){}" > "release/deps/a"$'"'"'\n'"'"'"b.rs"'
lay r14_tmp_rs DELETE 'mkdir tmp; echo "fn main(){}" > tmp/main.rs'
lay r15_gitfile KEEP 'echo "gitdir: /x" > release/deps/.git'
lay r16_cargo_lock KEEP 'echo x > release/Cargo.lock'
lay r17_ts KEEP 'echo "export {}" > release/index.ts'
lay r18_markdown KEEP 'echo "# x" > release/notes.markdown'
lay r19_out_cargo_toml KEEP 'mkdir -p release/build/x-1/out/proj; echo "[package]" > release/build/x-1/out/proj/Cargo.toml'
lay r20_git_incremental KEEP 'mkdir -p release/incremental/x-1/.git'
lay r21_fifo KEEP 'mkfifo release/deps/pipe'
lay r22_makefile_dir KEEP 'mkdir release/deps/proj; echo "all:" > release/deps/proj/Makefile'
lay r23_triple_stray KEEP 'mkdir -p x86_64-unknown-linux-gnu/release/deps; echo "fn main(){}" > x86_64-unknown-linux-gnu/main.rs'
lay c1_notes_in_deps KEEP 'echo notes > release/deps/notes.txt'
lay c2_main_c_profile KEEP 'echo "int main(){}" > release/main.c'
lay c3_rustdoc DELETE 'echo "{}" > .rustdoc_fingerprint.json; mkdir -p doc/demo; echo "<html>" > doc/demo/index.html'
lay c4_cargo_out DELETE 'mkdir -p release/build/x-1/out release/.fingerprint/x-1 release/incremental/x-1/s-1 release/examples; echo "pub fn f(){}" > release/build/x-1/out/gen.rs; printf bin > release/app; chmod +x release/app; : > release/.cargo-lock; : > release/.cargo-build-lock; echo x > release/app.d; echo x > release/examples/ex.d'
lay c5_triple_cargo DELETE 'mkdir -p x86_64-unknown-linux-gnu/release/deps x86_64-unknown-linux-gnu/release/build/y-2/out x86_64-unknown-linux-gnu/doc; echo x > x86_64-unknown-linux-gnu/release/deps/libz.rmeta; echo "pub fn f(){}" > x86_64-unknown-linux-gnu/release/build/y-2/out/gen.rs; printf x > x86_64-unknown-linux-gnu/CACHEDIR.TAG'
lay c6_empty_profiles DELETE 'rm -rf release; mkdir debug release'
largs=(); for n in ${LNAMES[@]+"${LNAMES[@]}"}; do largs+=(--target "$LT/$n"); done
run bash "$CL" ${largs[@]+"${largs[@]}"}
bad=""
for i in "${!LNAMES[@]}"; do
  if [ -e "$LT/${LNAMES[$i]}" ]; then got=KEEP; else got=DELETE; fi
  [ "$got" = "${LWANT[$i]}" ] || bad="$bad ${LNAMES[$i]}(want ${LWANT[$i]})"
done
if [ "$RC" = 0 ] && [ -z "$bad" ]; then pass; else fail "rc=$RC wrong:$bad"; fi

t "a real cargo build (debug, release, a build script writing OUT_DIR/gen.rs, an example, a test, cargo doc, --target) is deleted and the crate's source stays"
if ! command -v cargo >/dev/null 2>&1; then skip "cargo not installed"; else
RC_DIR=$(new_dir); ( cd "$RC_DIR" && cargo new -q --lib rc >/dev/null 2>&1 && cd rc \
  && printf 'fn main() { let o = std::env::var("OUT_DIR").unwrap(); std::fs::write(std::path::Path::new(&o).join("gen.rs"), "pub fn g() -> u32 { 1 }").unwrap(); }\n' > build.rs \
  && mkdir -p examples && echo 'fn main() {}' > examples/ex.rs \
  && printf 'include!(concat!(env!("OUT_DIR"), "/gen.rs"));\n#[test] fn t() { assert_eq!(g(), 1); }\n' > src/lib.rs \
  && cargo build -q >/dev/null 2>&1 && cargo build -q --release >/dev/null 2>&1 && cargo test -q >/dev/null 2>&1 \
  && cargo build -q --examples >/dev/null 2>&1 && cargo doc -q >/dev/null 2>&1 \
  && cargo build -q --release --target "$(rustc -vV | sed -n 's/host: //p')" >/dev/null 2>&1 )
if [ ! -d "$RC_DIR/rc/target" ]; then fail "the cargo build did not run"; else
  run bash "$CL" --root "$RC_DIR/rc"
  if [ "$RC" = 0 ] && [ ! -e "$RC_DIR/rc/target" ] && [ -f "$RC_DIR/rc/src/lib.rs" ] && [ -f "$RC_DIR/rc/build.rs" ] && [ -f "$RC_DIR/rc/Cargo.toml" ] \
     && echo "$OUT" | grep -q '^deleted '; then pass; else fail "rc=$RC out: $OUT"; fi
fi
fi

finish
