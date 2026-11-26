#!/bin/bash
# Build Pod Board.app (issue #177). Run it from anywhere: bash apps/pod-board/build-mac.sh
#
# Signs the bundle with a fixed local code-signing identity when the keychain has one, named "Pod Board Local" (or the
# name in POD_BOARD_SIGN_IDENTITY). With that identity the app's designated requirement is its identifier plus the
# certificate, the same for every build, so macOS keeps the Local Network permission across rebuilds. Without it the
# build is ad hoc (as in CI): its requirement is the cdhash, which changes with every build.
# The certificate is created by hand in Keychain Access (README, "Keep Local Network across rebuilds"). This script
# never creates, imports or exports a certificate or key.
#
# Verifies:        a signed build has a designated requirement that names a certificate and holds no cdhash; the script
#                  fails when the bundle or codesign is missing or codesign cannot read the requirement. It prints it.
# Does not verify: that macOS keeps the permission (check on the Mac, README).
set -euo pipefail

SRC_TAURI="${POD_BOARD_SRC_TAURI:-$(cd "$(dirname "$0")" && pwd)/src-tauri}"
IDENTITY="${POD_BOARD_SIGN_IDENTITY:-Pod Board Local}"
cd "$SRC_TAURI"

signed=0
if command -v security >/dev/null 2>&1 && security find-identity -p codesigning 2>/dev/null | grep -qF "\"$IDENTITY\""; then
  signed=1
  echo "Signing with the identity \"$IDENTITY\"."
else
  echo "No code-signing identity named \"$IDENTITY\" in the keychain: the build is ad hoc, so macOS treats every build as a new app" >&2
  echo "and asks again for Local Network. Create the certificate once (README) to keep the permission." >&2
fi

# Only the local identity signs this app. Any other Tauri signing or notarization setting in the shell (a certificate to
# import, an Apple ID that uploads to Apple) is cleared first.
unset APPLE_SIGNING_IDENTITY APPLE_CERTIFICATE APPLE_CERTIFICATE_PASSWORD APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID \
  APPLE_API_KEY APPLE_API_ISSUER APPLE_API_KEY_PATH
[ "$signed" = 1 ] && export APPLE_SIGNING_IDENTITY="$IDENTITY"

cargo tauri build

# A build meant to be signed is checked and fails when the check cannot be made.
if [ "$signed" = 1 ]; then
  APP="${CARGO_TARGET_DIR:-target}/release/bundle/macos/Pod Board.app"
  fail() { echo "error: $1" >&2; exit 1; }
  command -v codesign >/dev/null 2>&1 || fail "codesign not found, so the signature of the signed build cannot be checked."
  [ -d "$APP" ] || fail "no bundle at $SRC_TAURI/$APP, so the signature cannot be checked."
  requirement=$(codesign -dr - "$APP" 2>&1) || fail "codesign -dr - failed on $APP: $requirement"
  echo "$requirement"
  printf '%s' "$requirement" | grep -q 'cdhash' && fail "signed with \"$IDENTITY\", but the designated requirement is still a cdhash; Local Network would not survive a rebuild."
  printf '%s' "$requirement" | grep -q 'certificate' || fail "signed with \"$IDENTITY\", but the designated requirement names no certificate; Local Network would not survive a rebuild."
elif command -v codesign >/dev/null 2>&1; then
  codesign -dr - "${CARGO_TARGET_DIR:-target}/release/bundle/macos/Pod Board.app" 2>&1 || true
fi
