---
type: system
title: Pod Board Alerts, Build and Signing
description: The pure alert state machine in alerts.rs (new waits, reminders, finishes, Do Not Disturb, sound and flash), the build-mac.sh signing-identity logic that keeps macOS Local Network permission across rebuilds (ADR 0006), the one-installed-copy rule, and the tests that pin them.
tags: [pod-board, alerts, notifications, code-signing, macos, tauri]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-26T15:02:34.678Z
sources:
  - id: openwiki-source-b776ff28b037f05bc4649943
    resource: repo://apps/pod-board/build-mac.sh
  - id: openwiki-source-d607ecb35ffcb03781a5a3b6
    resource: repo://apps/pod-board/src-tauri/src/alerts.rs
  - id: openwiki-source-4e215ee0543d29d72f88d23f
    resource: repo://apps/pod-board/src-tauri/tests/bundle_config.rs
  - id: openwiki-source-64f14727fcfa9c2c5a5f77a9
    resource: repo://apps/pod-board/src-tauri/tests/theme_tokens.rs
  - id: openwiki-source-bc538321ded9a266fb7a753f
    resource: repo://docs/adr/0006-pod-board-is-signed-by-a-build-script-with-a-keychain-identity.md
  - id: openwiki-source-627c18839675e6b52d5bf0ec
    resource: repo://tests/scripts/pod-board-build.test.sh
generated: { by: "claude-code", at: "2026-11-26T15:02:34.678Z" }
---

# Pod Board Alerts, Build and Signing

<!-- openwiki: broken internal link [/openwiki/pod-board/overview.md] link "/openwiki/pod-board/overview.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
This page covers two independent mechanisms of [Pod Board](/openwiki/pod-board/overview.md): the logic that decides *when to tell the owner* about a session, and the build script that decides *how the macOS bundle is signed*.

## Alerts (`src-tauri/src/alerts.rs`)

`alerts.rs` is deliberately **pure logic**: no window, no clock, no I/O. `main.rs` feeds it one `Model` per poll through `Alerts::on_read(&model, now_ms)` and receives an `Outcome { alerts, sound, flash }` that it then turns into system notifications, a sound and a menu bar flash. This is why the module is unit-tested without a GUI.

### Alert kinds and state

Three kinds exist: `New` (a session started to wait), `Reminder(n)` (it still waits) and `Finished` (a long run ended). The struct keeps three maps keyed by `model::key(session)`: `waiting` (since when, how many reminders, when last told), `running` (since when a busy session works) and `finished`. Each entry also records the Pod Bay **machine** it belongs to (empty for a local terminal or desktop session).

### Wait notifications and reminders

- The **first read announces nothing**: it only learns what already waits. For Pod Bay sessions "first" means the first read that actually contained Pod Bay data (`podbay_read`), so a failed first `ls` does not make the first good read announce every wait as new (issue #158).
- A session whose state is `needs` that was not in the previous read raises a `New` alert, with a sound and a menu bar flash.
- A wait that continues gets a reminder after `REMIND_AFTER_MS` (10 minutes since the last notice), at most `MAX_REMINDERS` (3) per wait. A reminder plays a sound but does not flash. A wait that ends and starts again is a new wait.
- Sounds are at least `SOUND_GAP_MS` (10 s) apart, and only when the sound switch is on. Only a wait or reminder makes a sound; a finish is silent.

### Finishes

A session **finishes** when it was `busy` for at least `FINISH_MIN_RUN_MS` (2 minutes) and then stays `idle` for `FINISH_SETTLE_MS` (8 s, about two reads), so a pause between steps or a short conversational turn does not count. The run start is Pod Bay's own `since` time on the first read (the run began before the app looked), otherwise the app's clock. A session that goes from busy to a question is not a finish: the wait is announced instead. One quiet notification goes out ("Job finished", "Worked N min · …"); `Alerts::mark` then writes a `finished` timestamp onto each settled idle session, sets `counts.finished` and re-sorts so finished ones come first. `Alerts::looked(session)` clears the mark when the owner opens, peeks at or clicks the notification of that session; unattended finishes are forgotten after `FINISH_KEEP_MS` (12 hours).

### Do Not Disturb and unreachable machines

`dnd_until` mutes alerts, sound and flash while `now < until`, and `quiet()` clears it when it expires; the board itself still updates. A muted reminder is not spent: it goes out when Do Not Disturb ends, whereas a wait that *started* while muted is not announced late as new. A finish that settled during Do Not Disturb is notified after it ends, though the board marks it at once.

When Pod Bay cannot read a machine, `model.unread` lists it and its sessions are treated as **stale**: their last known state raises nothing, and their waits, runs and finishes are kept so that the machine's return after a blip is not announced as new and the reminder clock keeps running. Local sessions are read on every poll, so they keep no such state (issue #158).

### Tests

The `#[cfg(test)]` module in the same file covers: silent first read; one alert per new wait; reminder cadence (minutes 11, 21, 31, then none); sound gap and the switch; a machine dropping out for a read; sessions told apart by workspace or Remote Control id and surviving a rename; finish rules (short run, pause, question); Pod Bay's start time; Do Not Disturb interactions; and local terminal and desktop sessions taking part in the same rules.

## Build and signing

### Why signing matters

macOS ties the **Local Network** permission to the app's code signature. An ad hoc build has its `cdhash` as designated requirement, which changes on every build, so every rebuild is a new app and the granted permission is lost (issue #177). The fix, recorded in ADR 0006, is to sign each build with one fixed identity so the requirement names a certificate and stays the same. `src-tauri/Info.plist` also carries `NSLocalNetworkUsageDescription`, which macOS shows when asking; `tests/bundle_config.rs` fails if that key is missing (issue #174).

### `apps/pod-board/build-mac.sh`

The script lists valid code-signing identities with `security find-identity -v -p codesigning` and chooses the one named by `POD_BOARD_SIGN_IDENTITY`; when that variable is unset it looks for `Pod Board Local`. Matching rules: a name equal to the text wins over a name that merely contains it (so `Apple Development` finds `Apple Development: <name> (<id>)`), and the same certificate listed twice counts once.

- **Exactly one match** → export `APPLE_SIGNING_IDENTITY` with the full name and run `cargo tauri build`. Every other Apple/Tauri signing or notarization variable is cleared first, so only the local identity signs.
- **Several matches**, or a **set variable that matches nothing** → fail *before* building; a set variable never falls back to ad hoc.
- **Variable unset and no `Pod Board Local`** → an ad hoc build with a stderr warning (this is what CI produces).

After a signed build the script runs `codesign -dr -` on the bundle and **fails** if the requirement still contains `cdhash`, names no `certificate`, or cannot be read, or if `codesign` or the bundle is missing. The script never creates, imports or exports a certificate or key, and the repo contains no certificate, key or personal name. It does not verify that macOS actually keeps the permission; that is a manual check on a Mac. `tauri.conf.json` is not given a `signingIdentity`, because that would break CI where no identity exists.

ADR 0006 also records the second decision of the same ticket: a remote machine that Pod Bay lists with no address and no rows gets the chip "unreachable from Pod Board (Pod Bay gave no address)", whose tooltip says the app cannot tell the causes apart.

### One installed copy

After building, exactly one `Pod Board.app` should exist, in `~/Applications`. The README prescribes moving (not copying) the new bundle in, moving the old one to `~/.Trash` with a timestamp (never `rm -rf`), and checking with `mdfind "kMDItemFSName == 'Pod Board.app'"`; any other hit is a stale bundle to trash. An agent that builds the app is expected to name the copies it moved in its report. This is also a hard rule in the root `CLAUDE.md`.

### Tests

- `tests/scripts/pod-board-build.test.sh` tests `build-mac.sh` with **stubs** for `security`, `cargo` and `codesign` on `PATH` and made-up identity names, so nothing touches a keychain or builds the app. It checks that the identity is passed to the build, that a cdhash requirement is rejected, and the one/none/several matching cases.
<!-- openwiki: broken internal link [/openwiki/pod-shape/design-system-rules.md] link "/openwiki/pod-shape/design-system-rules.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
- `src-tauri/tests/theme_tokens.rs` parses `ui/theme.css` and Shape Board's `tokens.css` and fails when a token present in both has a different value in the light or dark theme ([Shape Board Design System](/openwiki/pod-shape/design-system-rules.md)).
- `src-tauri/tests/bundle_config.rs` checks the Local Network usage description.
<!-- openwiki: broken internal link [/openwiki/testing/test-suite-and-ci.md] link "/openwiki/testing/test-suite-and-ci.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
- `apps/pod-board/tests/acceptance/` holds Python and headless-Chrome acceptance scripts (`run.sh`, `run_137.sh`, `run_176.sh`) that exercise the two HTML pages with mock data; they do not cover the real window, Dock badge, menu bar or notifications. See [Test Suite, Lints and CI](/openwiki/testing/test-suite-and-ci.md).
