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
# Verifies:        a signed build has a designated requirement without a cdhash; it prints the requirement.
# Does not verify: that macOS keeps the permission (check on the Mac, README).
set -euo pipefail

SRC_TAURI="${POD_BOARD_SRC_TAURI:-$(cd "$(dirname "$0")" && pwd)/src-tauri}"
IDENTITY="${POD_BOARD_SIGN_IDENTITY:-Pod Board Local}"
cd "$SRC_TAURI"

signed=0
if command -v security >/dev/null 2>&1 && security find-identity -p codesigning 2>/dev/null | grep -qF "\"$IDENTITY\""; then
  export APPLE_SIGNING_IDENTITY="$IDENTITY"
  signed=1
  echo "Signing with the identity \"$IDENTITY\"."
else
  # An identity set in the environment for something else is not used for this app.
  unset APPLE_SIGNING_IDENTITY
  echo "No code-signing identity named \"$IDENTITY\" in the keychain: the build is ad hoc, so macOS treats every build as a new app" >&2
  echo "and asks again for Local Network. Create the certificate once (README) to keep the permission." >&2
fi

cargo tauri build

APP="target/release/bundle/macos/Pod Board.app"
if command -v codesign >/dev/null 2>&1 && [ -d "$APP" ]; then
  requirement=$(codesign -dr - "$APP" 2>&1 || true)
  echo "$requirement"
  if [ "$signed" = 1 ] && printf '%s' "$requirement" | grep -q 'cdhash'; then
    echo "error: signed with \"$IDENTITY\", but the designated requirement is still a cdhash; Local Network would not survive a rebuild." >&2
    exit 1
  fi
fi
