---
ticket: 177
stage: decision
status: accepted
confidence: medium
decided_by: owner
date: 2026-11-26
---

# 0006. Pod Board is signed by a build script with a keychain identity, and says when a machine gave no address

## Context

Pod Board built ad hoc has its cdhash as designated requirement, so macOS sees every rebuild as a new app and the Local Network permission of the last build does not apply (issue #177). Inside the app a machine that Pod Bay lists offline has no `addr`, so the #174 probe skipped it and the board showed only "offline · password out of date".

## Options

- Sign with an identity that the keychain already holds, named in `POD_BOARD_SIGN_IDENTITY` (an Apple Development certificate on a Mac set up for Apple development), picked up by a build script (chosen, owner's answer of 2026-11-26: `POD_BOARD_SIGN_IDENTITY="Apple Development" bash build-mac.sh`, because the owner does not want to open Keychain Access or create a certificate)
- Sign with a self-signed certificate `Pod Board Local` that the owner creates by hand (the owner's first answer; kept as the other way, and the default when the variable is not set)
- Put `signingIdentity` in `tauri.conf.json`: needs a name that is in the repo, and breaks the CI build where the identity does not exist
- Chip for no address, as the owner asked ("show the unreachable chip when Pod Bay gives no address for the machine"): chosen. A wider rule, "unreachable" for every offline machine, was not taken: a machine that Pod Bay lists with an address is probed first, and a machine that returned rows is never marked

## Decision

`apps/pod-board/build-mac.sh` lists the valid code-signing identities (`security find-identity -v -p codesigning`) and picks the one that `POD_BOARD_SIGN_IDENTITY` names: a name equal to the text wins over a name that only contains it (so `Apple Development` finds `Apple Development: <name> (<id>)`), and one certificate listed twice counts once. Exactly one match signs the build: `APPLE_SIGNING_IDENTITY` gets the full name, and every other Tauri signing and notarization variable is cleared. No match, or several, stops the script before the build; a variable that is set never falls back to ad hoc. With the variable not set, `Pod Board Local` is used if the keychain lists it, else the build is ad hoc as in CI. After a signed build the script fails if the designated requirement holds a cdhash or no certificate, or cannot be read. The script never creates, imports or exports a certificate or key; the README gives the Keychain Access steps for the self-signed way. No name, e-mail or team id is written into the repo, the tests or this record.
A machine other than this one, with no `addr` in `machines` and no rows in a fresh `ls --all`, gets the chip `<machine> unreachable from Pod Board (Pod Bay gave no address)`. Its tooltip names the three causes the app cannot tell apart. The #174 chip for a probed "No route to host" stays.

## Why

An identity that the owner already has needs no setup, and a name typed on the command line keeps the repo free of personal names. The environment variable keeps `tauri.conf.json` the same for CI. Several matches or none are errors, because a silent ad hoc build would lose the permission without a word. The chip rule uses only what the app observed (no rows, no address); it makes no claim about the cause.

## Consequences

The first signed build asks for Local Network once; later builds with the same identity do not. An Apple Development certificate expires and is replaced after a time; whether the requirement and the permission survive a replacement was not checked. In the data seen on M0 on 2026-11-26, an offline machine has `addr: null` and an online one has an address, so the chip shows next to each offline remote machine that returned no rows. That includes a machine that is simply off; the tooltip says that the cause is not known, and the chip's words say only that the app could not reach it. The script is tested with stubs only. A real signed build, its requirement and the permission on the Mac were not checked in the fleet run.

## How to reverse

Delete `apps/pod-board/build-mac.sh` and the `no_address` field and its chip; builds go back to ad hoc.
