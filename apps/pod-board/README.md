# Pod Board

A small desktop app that shows every Pod Bay session on every machine by state: `Needs you` (a dialog waits for a
human), `Working`, `Idle`. Workspaces with no session show as small `Stopped` chips. It only observes.

It is a Tauri 2 app (Rust and one HTML page). It needs no server, no port and no Node.

## What it does

- **Dock:** one icon; a click opens the window. The badge on the icon is the number of sessions that wait for you.
- **Menu bar:** an icon for the most urgent state and its count: a hand (needs you), a bolt (working), a cup (idle),
  a moon (nothing running), or a warning sign when Pod Bay cannot be read. Icons: Phosphor, duotone (`THIRD-PARTY.md`).
  Its menu opens the window, turns "start at login" and "always on top" on and off, and quits.
- **Notification:** one system notification when a session changes to `Needs you`. The first read announces nothing.
- **Window:** a sidebar that filters by state and machine, one summary row and three lanes. Closing the window hides it; the app keeps watching.

## Build and run

```
cd apps/pod-board/src-tauri
cargo test                 # the model: order, allow list, notification rule
cargo run                  # run from source
cargo install tauri-cli --version '^2' --locked
cargo tauri build          # macOS: target/release/bundle/macos/Pod Board.app
```

Linux needs the WebKitGTK 4.1 development packages. The app looks for `podbay-dev` in `PATH`, then in
`~/.local/bin`, `/usr/local/bin` and `/opt/homebrew/bin`, because a desktop app starts with a short `PATH`.

## Data and privacy

- It runs the read-only commands `podbay-dev ls --all` (a 3 s pause after each read) and, every 10th read,
  `checkouts --all`, `machines` and `queue ls --all`. It never calls `podbay-dev detail`.
- The model is built from an allow list of fields (`src/model.rs`). It never holds `account`, `uuid`, `dir`, `path`,
  `remote_url`, machine ids or addresses. A failed read shows `podbay-dev <subcommand> failed`, never stderr.
- It sends nothing anywhere: no network code, and the page may only talk to the app itself.
- State: `asks: true` or no `status` yet is `needs`; else `status: "busy"` is `busy`; else `idle`. The rules are the
  same as `podsOf` in `plugins/pd-bot/board/board.js`.

## Limits

- Verifies: `cargo test` (8 cases on the model).
- Does not verify: that `asks` means a human is awaited (on 2026-11-21 four sessions had `asks: true` with no dialog
  on screen); the window, the Dock badge, the menu bar item and notifications in an automated test.
- The app is not signed or notarized. On another Mac, Gatekeeper asks before the first start.
- It shows what Pod Bay on this machine sees. A machine that this Pod Bay lists as offline has no sessions here.
