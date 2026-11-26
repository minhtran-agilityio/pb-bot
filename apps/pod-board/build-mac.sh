#!/bin/bash
# Build Pod Board.app (issue #177). Run it from anywhere: bash apps/pod-board/build-mac.sh
#
# Signs the bundle with a code-signing identity from the keychain, so the app's designated requirement names a
# certificate and is the same for every build, and macOS keeps the Local Network permission across rebuilds. Without
# an identity the build is ad hoc (as in CI): its requirement is the cdhash, which changes with every build.
#
# Which identity: POD_BOARD_SIGN_IDENTITY, a name or a part of one, for example
#   POD_BOARD_SIGN_IDENTITY="Apple Development" bash apps/pod-board/build-mac.sh
# It must match exactly one valid code-signing identity (`security find-identity -v -p codesigning`): a name equal to the
# text wins over a name that only contains it, and the same identity listed twice counts once. No match, or several,
# stops the script before the build. When the variable is not set, the identity named "Pod Board Local" is used if the
# keychain has it, else the build is ad hoc.
# This script never creates, imports or exports a certificate or key.
#
# Verifies:        a signed build has a designated requirement that names a certificate and holds no cdhash; the script
#                  fails when the bundle or codesign is missing or codesign cannot read the requirement. It prints it.
# Does not verify: that macOS keeps the permission (check on the Mac, README).
set -euo pipefail

SRC_TAURI="${POD_BOARD_SRC_TAURI:-$(cd "$(dirname "$0")" && pwd)/src-tauri}"
ASKED="${POD_BOARD_SIGN_IDENTITY:-}"
WANT="${ASKED:-Pod Board Local}"
cd "$SRC_TAURI"

fail() { echo "error: $1" >&2; exit 1; }

# Count the valid code-signing identities that match $WANT, by certificate hash (an identity in two keychains is one).
exact_n=0; exact_name=""; part_n=0; part_name=""; exact_seen=" "; part_seen=" "; part_all=""
listing=""
if command -v security >/dev/null 2>&1; then listing=$(security find-identity -v -p codesigning 2>/dev/null || true); fi
line_re='^[[:space:]]*[0-9]+\)[[:space:]]+([0-9A-Fa-f]+)[[:space:]]+"(.*)"'
while IFS= read -r line; do
  [[ $line =~ $line_re ]] || continue
  hash="${BASH_REMATCH[1]}"; name="${BASH_REMATCH[2]}"
  if [ "$name" = "$WANT" ]; then
    case "$exact_seen" in *" $hash "*) ;; *) exact_seen="$exact_seen$hash "; exact_n=$((exact_n + 1)); exact_name="$name" ;; esac
  fi
  if [[ $name == *"$WANT"* ]]; then
    case "$part_seen" in *" $hash "*) ;; *) part_seen="$part_seen$hash "; part_n=$((part_n + 1)); part_name="$name"; part_all="$part_all  $name"$'\n' ;; esac
  fi
done <<< "$listing"

signed=0; identity=""
if [ "$exact_n" -gt 0 ]; then count=$exact_n; identity=$exact_name; else count=$part_n; identity=$part_name; fi
if [ "$count" -gt 1 ]; then
  fail "\"$WANT\" matches $count valid code-signing identities; name one of them in full in POD_BOARD_SIGN_IDENTITY (two certificates with the same name: delete one in Keychain Access):
$part_all"
elif [ "$count" -eq 1 ]; then
  signed=1
  echo "Signing with the identity \"$identity\"."
elif [ -n "$ASKED" ]; then
  fail "POD_BOARD_SIGN_IDENTITY=\"$ASKED\" matches no valid code-signing identity in the keychain (security find-identity -v -p codesigning). Nothing was built."
else
  echo "No code-signing identity named \"$WANT\" in the keychain: the build is ad hoc, so macOS treats every build as a new app" >&2
  echo "and asks again for Local Network. Sign with an identity (README) to keep the permission." >&2
fi

# Only the local identity signs this app. Any other Tauri signing or notarization setting in the shell (a certificate to
# import, an Apple ID that uploads to Apple) is cleared first.
unset APPLE_SIGNING_IDENTITY APPLE_CERTIFICATE APPLE_CERTIFICATE_PASSWORD APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID \
  APPLE_API_KEY APPLE_API_ISSUER APPLE_API_KEY_PATH
[ "$signed" = 1 ] && export APPLE_SIGNING_IDENTITY="$identity"

cargo tauri build

# A build meant to be signed is checked and fails when the check cannot be made.
if [ "$signed" = 1 ]; then
  APP="${CARGO_TARGET_DIR:-target}/release/bundle/macos/Pod Board.app"
  command -v codesign >/dev/null 2>&1 || fail "codesign not found, so the signature of the signed build cannot be checked."
  [ -d "$APP" ] || fail "no bundle at $SRC_TAURI/$APP, so the signature cannot be checked."
  requirement=$(codesign -dr - "$APP" 2>&1) || fail "codesign -dr - failed on $APP: $requirement"
  echo "$requirement"
  printf '%s' "$requirement" | grep -q 'cdhash' && fail "signed with \"$identity\", but the designated requirement is still a cdhash; Local Network would not survive a rebuild."
  printf '%s' "$requirement" | grep -q 'certificate' || fail "signed with \"$identity\", but the designated requirement names no certificate; Local Network would not survive a rebuild."
elif command -v codesign >/dev/null 2>&1; then
  codesign -dr - "${CARGO_TARGET_DIR:-target}/release/bundle/macos/Pod Board.app" 2>&1 || true
fi
