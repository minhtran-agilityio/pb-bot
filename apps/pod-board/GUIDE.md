# Pod Board

Pod Board is a small desktop app that shows every Pod Bay Claude session on every machine, sorted by what it needs
from you: **Needs you**, **Working**, **Idle** and **Stopped**.
It tells you when a session starts to wait for you, so you can leave long tasks running and come back when needed.

It only watches. It reads `podbay-dev` and never changes a session.

## Requirements

- macOS (Linux also builds; it needs the WebKitGTK 4.1 development packages).
- `podbay-dev` installed and signed in on this machine.
- Rust, and the Tauri CLI: `cargo install tauri-cli --version '^2' --locked`.

## Build and install

```sh
cd apps/pod-board/src-tauri   # from the repo root
cargo test                      # optional
cargo tauri build
cp -R "target/release/bundle/macos/Pod Board.app" ~/Applications/
open ~/Applications/"Pod Board.app"
```

The app is not signed, so the first start on a new Mac asks for confirmation (right-click the app, then **Open**).

## Use

- **Board:** one lane per state. Filter by state or machine in the sidebar; switch between **Board** and **List** at
  the top right.
- **Open a session:** click a card. The Claude desktop app switches to that session.
- **Peek:** the speech-bubble button on a card shows the session's last reply and your last prompt, without opening it.
- **Menu bar:** the icon shows the most urgent state and a count. Left-click for a small list of who needs you and
  what is working; right-click for settings (sound, do not disturb, start at login, always on top).
- **Alerts:** a notification, a short sound and a Dock badge when a session starts to need you, with a reminder
  every 10 minutes while it still waits (at most 3). **Do not disturb** mutes them for 30 minutes to 2 hours.
  Click a notification to open its session.
- **Finished:** a session that worked for 2 minutes or more and went idle gets a quiet notification and a green
  `finished` tag at the top of the Idle lane, until you open or peek at it.
- **Links:** the small buttons on a card (`PR #266`, an artifact's title) open that page in the browser.
- **Closing the window** keeps the app running in the menu bar; click the Dock icon to bring it back.

More detail on what it reads and its limits: [`README.md`](README.md).
