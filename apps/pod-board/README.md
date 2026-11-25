# Pod Board

A small desktop app that shows every Pod Bay session on every machine by state: `Needs you` (a dialog waits for a
human), `Working`, `Idle`. Workspaces with no session show as small `Stopped` chips. It only observes.

It is a Tauri 2 app (Rust and one HTML page). It needs no server, no port and no Node.

## What it does

- **Dock:** one icon; a click opens the window. The badge on the icon is the number of sessions that wait for you.
- **Menu bar:** an icon for the most urgent state and its count: a hand (needs you), a loader (working), a cup (idle),
  a moon with a star (nothing running), or a warning triangle when Pod Bay cannot be read. They are the same Lucide
  shapes as the window's icons, as one-colour template images (`THIRD-PARTY.md`).
  A left click opens the popover (below); a right click shows its menu, which opens the window, turns "start at
  login" and "always on top" on and off, and quits.
- **Notification:** one system notification when a session starts to need you, and a reminder when it still needs
  you 10 minutes later (at most 3 reminders for one wait). The first read after start announces nothing.
  A click on a notification opens its session in the Claude app (the board, when the session has no Remote Control
  link or the notification is from an earlier start of the app).
- **Finished:** a session that worked for at least 2 minutes and then stays idle for 8 seconds has finished. It gets
  one notification with no sound, a green `finished` tag, the first place in the Idle lane, a line in the summary
  row, its own group in the popover and a `✓n` beside the menu bar icon (hidden while a session needs you). The mark
  goes when you open the session, peek at it or click its notification, when it works again, or after 12 hours.
  A shorter run is not marked: it is a turn of a conversation you are in. Do not disturb holds the notification
  back until it ends; the mark shows at once.
- **Links:** a card shows the session's pull requests, issues and artifacts as small buttons (`PR #266`, an
  artifact's title); a click opens that page in the browser. A lane card shows 3, a list row 2, a state page 8.
- **Sound and flash:** a new wait or a reminder plays a short sound (macOS, at most one every 10 s; "Sound" in the
  menu bar menu turns it off). A new wait also blinks the menu bar icon a few times.
- **Do not disturb:** 30 minutes, 1 hour or 2 hours from the menu bar menu. It mutes notifications, reminders, sound
  and flash; the board keeps updating. It ends by itself, or with "End" in the window. A reminder that falls inside
  it goes out when it ends. The sound switch and the end time survive a restart (`settings.json` in the app's
  config folder).
- **Window:** a 48 px title bar (the Pod Cat logo and where you are, `Pod Board › Overview`; drag it to move the
  window), a sage sidebar that filters by state and machine, and a main panel with the page title, one summary row
  and three lanes. Closing the window hides it; the app keeps watching. The button on the title bar (or ⌘B)
  collapses the sidebar to its icons and expands it again; the choice is remembered. The theme switch (light, dark,
  follow the system) is at the foot of the sidebar; the popover follows the same choice.
- **Open a session:** click a card (or press Enter on it). On macOS this opens `claude://code/<id>`, so the Claude
  desktop app switches to that session; elsewhere it opens `https://claude.ai/code/<id>` in the browser.
- **Peek:** a speech-bubble button on each card (always shown on a card that needs you, on hover for the others)
  opens a side panel with the session's last reply and your last prompt, so you can see what it asks without
  switching to it. It reads them on demand with `podbay-dev --on <machine> detail <name>`; another machine can take
  about ten seconds. A read is kept for 20 s; "Read again" reads it now. Esc or × closes the panel.
- **Menu bar popover:** a left click on the menu bar icon opens a small panel under it with the sessions that need
  you, the ones that finished and the ones working; a row opens that session in the Claude app, and "Open Pod Board" opens the window. A
  right click (or Control-click) shows the menu. The popover closes when you click anywhere else.
- **Cat:** a small black cat with glowing eyes, drawn for this app in the logo's colours, sits in the round tile at
  the start of the summary row, on every page. It shows the heaviest state of the whole board: it waves when a session needs you, types when one is working, and
  sleeps otherwise. It is decorative (hidden from screen readers) and does nothing when clicked.

## Look

Pod Board uses the UI design system (UDS v3.0) that Shape Board uses: the same tokens (`ui/theme.css` copies
`apps/pod-shape/src/styles/tokens.css`, and `cargo test` fails when a shared token differs), the Outfit font
(JetBrains Mono for code only), Lucide icons at stroke 1.75, soft state pills (colour, icon and word), neutral lanes,
and a 2 px focus ring on every control. Fonts and icons ship inside the app. Design notes: [`DESIGN.md`](DESIGN.md).

## Build and run

```
cd apps/pod-board/src-tauri
cargo test                 # the model: order, allow list, notification rule; the theme token check
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
  `remote_url`, machine ids or addresses. It does hold each session's links as Pod Bay lists them (kind, title and
  `https://` address, at most 8 per session), so a pull request link names its repository on the page. The app
  opens only an address the current read holds, and only when you click it. A failed read shows `podbay-dev <subcommand> failed`, never stderr.
- It sends nothing anywhere: no network code, and the page may only talk to the app itself. A clicked link is
  handed to the system (`open`, or `xdg-open` on Linux), which opens it in your browser.
- State: `asks: true` or no `status` yet is `needs`; else `status: "busy"` is `busy`; else `idle`.

## Limits

- Verifies: `cargo test` (the model and its links, alerts and the finished rule, the Pod Bay reader, the peek text,
  the menu bar count, the popover position, and the theme tokens against Shape Board's). The look:
  `bash apps/pod-board/tests/acceptance/run.sh` (headless Chrome over the two pages, no data).
- Does not verify: that `asks` means a human is awaited (on 2026-11-21 four sessions had `asks: true` with no dialog
  on screen); the window, the Dock badge, the menu bar item and notifications in an automated test; that Pod Bay
  never reports a working session as `idle` for more than 8 seconds in the middle of a run, which would mark it
  finished too early. On 2026-11-21 two real runs on M0 (one `sleep 150`, one of ten tool calls) stayed `busy`
  from start to end; a run that waits on a background task or a subagent was not observed.
- Checked by hand on macOS on 2026-11-21, in the installed app: a click on a `needs you` and on a `finished`
  notification opens that session in the Claude app and clears the mark; a click on a notification from an earlier
  start of the app brings the board forward; each link button on a card opens its page in the browser and not the
  session; the `finished` tag (board and List, light and dark), the summary line, the popover group and `✓n` in
  the menu bar; opening the session from its card clears the mark.
- The app is not signed or notarized. On another Mac, Gatekeeper asks before the first start.
- It shows what Pod Bay on this machine sees. A machine that this Pod Bay lists as offline has no sessions here.
