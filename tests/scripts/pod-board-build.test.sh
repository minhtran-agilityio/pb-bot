#!/bin/bash
# Contract tests for apps/pod-board/build-mac.sh (issue #177): the app is signed with the fixed local identity when
# the keychain has it, so its designated requirement does not depend on the cdhash; without it the build is ad hoc.
# `security`, `cargo` and `codesign` are stubs on PATH: nothing here touches a keychain or builds the app.
source "$(dirname "$0")/../lib.sh"

SCRIPT="$ROOT/apps/pod-board/build-mac.sh"

# fake <identity-listed: yes|no> <designated requirement text> → sets D to a folder of stubs; its log records what ran.
# Not called in a $(...) substitution: the folder must be registered for cleanup in this shell.
fake() {
  local d; d=$(mktemp -d); _CLEANUP_DIRS+=("$d"); D=$d; mkdir -p "$d/bin"
  if [ "$1" = yes ]; then
    printf '#!/bin/sh\necho "  1) AAAA \\"Pod Board Local\\""\n' > "$d/bin/security"
  else
    printf '#!/bin/sh\necho "     0 identities found"\n' > "$d/bin/security"
  fi
  # the stub cargo records the signing variable and "builds" a bundle folder where the real build leaves it
  cat > "$d/bin/cargo" <<STUB
#!/bin/sh
echo "cargo \$* identity=\${APPLE_SIGNING_IDENTITY:-none} other=\${APPLE_CERTIFICATE:-}\${APPLE_ID:-}\${APPLE_PASSWORD:-}\${APPLE_TEAM_ID:-}\${APPLE_API_KEY:-}" >> "$d/log"
[ -n "\${NO_BUNDLE:-}" ] || mkdir -p "\${CARGO_TARGET_DIR:-$d/work/target}/release/bundle/macos/Pod Board.app"
STUB
  printf '#!/bin/sh\necho "designated => %s"\n' "$2" > "$d/bin/codesign"
  chmod +x "$d/bin/"*
  mkdir -p "$d/work"
}

run() { # run <fake dir> [env...] → output; exit code in $rc
  local d=$1; shift
  out=$(env POD_BOARD_SRC_TAURI="$d/work" PATH="$d/bin:$PATH" "$@" bash "$SCRIPT" 2>&1); rc=$?
}

t "the script exists"
[ -f "$SCRIPT" ] && pass || fail "missing $SCRIPT"

t "with the identity in the keychain the build is signed with it, and a certificate-based requirement is accepted"
fake yes 'identifier "io.pdbot.podboard" and certificate leaf = H"0011"'; d=$D
run "$d"
if [ "$rc" -eq 0 ] && grep -q 'tauri build identity=Pod Board Local' "$d/log"; then pass; else fail "rc=$rc out=$out log=$(cat "$d/log" 2>/dev/null)"; fi

t "with the identity but a cdhash requirement the script fails and says so"
fake yes 'cdhash H"abcd"'; d=$D
run "$d"
if [ "$rc" -ne 0 ] && printf '%s' "$out" | grep -qi 'cdhash'; then pass; else fail "rc=$rc out=$out"; fi

t "without the identity the build still succeeds, ad hoc, and says it is ad hoc"
fake no 'cdhash H"abcd"'; d=$D
run "$d"
if [ "$rc" -eq 0 ] && grep -q 'tauri build identity=none' "$d/log" && printf '%s' "$out" | grep -qi 'ad hoc'; then pass; else fail "rc=$rc out=$out log=$(cat "$d/log" 2>/dev/null)"; fi

t "with the identity, a requirement that names no certificate fails"
fake yes 'identifier "io.pdbot.podboard"'; d=$D
run "$d"
if [ "$rc" -ne 0 ] && printf '%s' "$out" | grep -qi 'names no certificate'; then pass; else fail "rc=$rc out=$out"; fi

t "with the identity, a root-certificate requirement is accepted"
fake yes 'identifier "io.pdbot.podboard" and certificate root = H"0011"'; d=$D
run "$d"
[ "$rc" -eq 0 ] && pass || fail "rc=$rc out=$out"

t "with the identity, a codesign that cannot read the bundle fails"
fake yes 'unused'; d=$D
printf '#!/bin/sh\necho "code object is not signed at all" >&2\nexit 1\n' > "$d/bin/codesign"
run "$d"
if [ "$rc" -ne 0 ] && printf '%s' "$out" | grep -q 'not signed at all'; then pass; else fail "rc=$rc out=$out"; fi

t "with the identity, a missing bundle fails"
fake yes 'identifier "io.pdbot.podboard" and certificate leaf = H"0011"'; d=$D
run "$d" NO_BUNDLE=1
if [ "$rc" -ne 0 ] && printf '%s' "$out" | grep -qi 'no bundle'; then pass; else fail "rc=$rc out=$out"; fi

t "with the identity, the bundle is found under CARGO_TARGET_DIR"
fake yes 'identifier "io.pdbot.podboard" and certificate leaf = H"0011"'; d=$D
run "$d" CARGO_TARGET_DIR="$d/elsewhere"
[ "$rc" -eq 0 ] && pass || fail "rc=$rc out=$out"

t "the other Apple signing and notarization variables never reach the build"
fake yes 'identifier "io.pdbot.podboard" and certificate leaf = H"0011"'; d=$D
run "$d" APPLE_CERTIFICATE=c APPLE_CERTIFICATE_PASSWORD=p APPLE_ID=i APPLE_PASSWORD=w APPLE_TEAM_ID=t APPLE_API_KEY=k APPLE_API_ISSUER=q APPLE_API_KEY_PATH=/x
if [ "$rc" -eq 0 ] && grep -q 'identity=Pod Board Local other=$' "$d/log"; then pass; else fail "rc=$rc log=$(cat "$d/log" 2>/dev/null)"; fi

t "an identity already in APPLE_SIGNING_IDENTITY is not used when the local one is missing"
fake no 'cdhash H"abcd"'; d=$D
run "$d" APPLE_SIGNING_IDENTITY="Someone Else"
if [ "$rc" -eq 0 ] && grep -q 'identity=none' "$d/log"; then pass; else fail "rc=$rc log=$(cat "$d/log" 2>/dev/null)"; fi

t "the identity name can be changed with POD_BOARD_SIGN_IDENTITY"
fake yes 'identifier "io.pdbot.podboard" and certificate leaf = H"0011"'; d=$D
printf '#!/bin/sh\necho "  1) AAAA \\"Other Name\\""\n' > "$d/bin/security"
run "$d" POD_BOARD_SIGN_IDENTITY="Other Name"
if [ "$rc" -eq 0 ] && grep -q 'identity=Other Name' "$d/log"; then pass; else fail "rc=$rc out=$out log=$(cat "$d/log" 2>/dev/null)"; fi

t "the repo holds no personal signing identity"
grep -qEi 'Apple Development:|Developer ID|@icloud|@gmail|signingIdentity' "$SCRIPT" "$ROOT/apps/pod-board/src-tauri/tauri.conf.json"; rc=$?
if [ "$rc" -eq 1 ]; then pass; else fail "grep rc=$rc (0 = a personal identity or a signingIdentity key is in the build files, 2 = a file is missing)"; fi

finish
