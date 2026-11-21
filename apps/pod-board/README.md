# Pod Board

A small desktop app that shows every Pod Bay session on every machine by state: `Needs you` (a dialog waits for a
human), `Working`, `Idle`. Workspaces with no session show as small `Stopped` chips. It only observes.

It is a Tauri 2 app (Rust and one HTML page). It needs no server, no port and no Node.

## What it does

- **Dock:** one icon; a click opens the window. The badge on the icon is the number of sessions that wait for you.
- **Menu bar:** an icon for the most urgent state and its count: a hand (needs you), a bolt (working), a cup (idle),
  a moon (nothing running), or a warning sign when Pod Bay cannot be read. Icons: Phosphor, duotone (`THIRD-PARTY.md`).
  A left click opens the popover (below); a right click shows its menu, which opens the window, turns "start at
  login" and "always on top" on and off, and quits.
- **Notification:** one system notification when a session starts to need you, and a reminder when it still needs
  you 10 minutes later (at most 3 reminders for one wait). The first read after start announces nothing.
- **Sound and flash:** a new wait or a reminder plays a short sound (macOS, at most one every 10 s; "Sound" in the
  menu bar menu turns it off). A new wait also blinks the menu bar icon a few times.
- **Do not disturb:** 30 minutes, 1 hour or 2 hours from the menu bar menu. It mutes notifications, reminders, sound
  and flash; the board keeps updating. It ends by itself, or with "End" in the window. A reminder that falls inside
  it goes out when it ends. The sound switch and the end time survive a restart (`settings.json` in the app's
  config folder).
- **Window:** a sidebar that filters by state and machine, one summary row and three lanes. Closing the window hides it; the app keeps watching.
- **Open a session:** click a card (or press Enter on it). On macOS this opens `claude://code/<id>`, so the Claude
  desktop app switches to that session; elsewhere it opens `https://claude.ai/code/<id>` in the browser.
- **Peek:** a speech-bubble button on each card (always shown on a card that needs you, on hover for the others)
  opens a side panel with the session's last reply and your last prompt, so you can see what it asks without
  switching to it. It reads them on demand with `podbay-dev --on <machine> detail <name>`; another machine can take
  about ten seconds. A read is kept for 20 s; "Read again" reads it now. Esc or × closes the panel.
- **Menu bar popover:** a left click on the menu bar icon opens a small panel under it with the sessions that need
  you and the ones working; a row opens that session in the Claude app, and "Open Pod Board" opens the window. A
  right click (or Control-click) shows the menu. The popover closes when you click anywhere else.
- **Cat:** a small black cat with glowing eyes, drawn for this app, sits in the window's bottom-right corner, beside the Stopped strip. It
  shows the heaviest state of the whole board: it waves when a session needs you, types when one is working, and
  sleeps otherwise. It is decorative (hidden from screen readers) and does nothing when clicked.

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
  `checkouts --all`, `machines` and `queue ls --all`.
- It runs `podbay-dev --on <machine> detail <name>` only when you open a peek, for that one session, and never in
  the background. It keeps the last prompt and the last reply (Pod Bay sends a long reply as a cut head and tail,
  which the app joins) and drops the screen, which can show the account and anything else on the terminal. Machine
  and session names pass only when they are plain names (`safe_name` in `src/model.rs`).
- The model is built from an allow list of fields (`src/model.rs`). It never holds `account`, `uuid`, `dir`, `path`,
  `remote_url`, machine ids or addresses. A failed read shows `podbay-dev <subcommand> failed`, never stderr.
- It sends nothing anywhere: no network code, and the page may only talk to the app itself.
- State: `asks: true` or no `status` yet is `needs`; else `status: "busy"` is `busy`; else `idle`. The rules are the
  same as `podsOf` in `plugins/pd-bot/board/board.js`.

## Limits

- Verifies: `cargo test` (24 cases: the model, alerts, the Pod Bay reader, the peek text and the popover position).
- Does not verify: that `asks` means a human is awaited (on 2026-11-21 four sessions had `asks: true` with no dialog
  on screen); the window, the Dock badge, the menu bar item and notifications in an automated test.
- The app is not signed or notarized. On another Mac, Gatekeeper asks before the first start.
- It shows what Pod Bay on this machine sees. A machine that this Pod Bay lists as offline has no sessions here.
