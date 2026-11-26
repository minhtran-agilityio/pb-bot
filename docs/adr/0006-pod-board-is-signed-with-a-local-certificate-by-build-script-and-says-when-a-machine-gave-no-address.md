---
ticket: 177
stage: decision
status: accepted
confidence: medium
decided_by: owner
date: 2026-11-26
---

# 0006. Pod Board is signed with a local certificate by a build script, and says when a machine gave no address

## Context

Pod Board built ad hoc has its cdhash as designated requirement, so macOS sees every rebuild as a new app and the Local Network permission of the last build does not apply (issue #177). Inside the app a machine that Pod Bay lists offline has no `addr`, so the #174 probe skipped it and the board showed only "offline · password out of date".

## Options

- Sign with a self-signed local certificate named `Pod Board Local`, picked up by a build script from the keychain (chosen; the owner's answer)
- Put `signingIdentity` in `tauri.conf.json`: needs a name that is in the repo, and breaks the CI build where the certificate does not exist
- Sign with an Apple Development certificate: its name holds the owner's name and e-mail
- Chip for no address: show "unreachable" for every machine that is offline (rejected: says too much, an off machine would read as a fault of the app)

## Decision

`apps/pod-board/build-mac.sh` runs `cargo tauri build` with `APPLE_SIGNING_IDENTITY` set to `Pod Board Local` (or `POD_BOARD_SIGN_IDENTITY`) when `security find-identity -p codesigning` lists it, and unsets the variable otherwise, so the build stays ad hoc as in CI. After a signed build it fails if the designated requirement still holds a cdhash. The script never creates or imports a certificate or key; the README gives the steps for the owner.
A machine other than this one, with no `addr` in `machines` and no rows in a fresh `ls --all`, gets the chip `<machine> unreachable from Pod Board (Pod Bay gave no address)`. Its tooltip names the three causes the app cannot tell apart. The #174 chip for a probed "No route to host" stays.

## Why

The certificate name is generic, so nothing personal enters the repo. The script is the one place that knows the name, and an environment variable keeps `tauri.conf.json` the same for CI. The chip rule uses only what the app observed (no rows, no address); it makes no claim about the cause.

## Consequences

The first signed build asks for Local Network once; later builds with the same certificate do not. A machine that is off and has no address now also shows the chip: the tooltip says that an off machine looks the same. The script, the signed requirement and the permission on the Mac were not checked in the fleet run (no certificate exists yet).

## How to reverse

Delete `apps/pod-board/build-mac.sh` and the `no_address` field and its chip; builds go back to ad hoc.
