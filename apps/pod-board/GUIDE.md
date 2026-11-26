# Pod Board

Pod Board is a small desktop app that shows every Pod Bay Claude session on every machine, and the Claude terminal
sessions of this machine that have Remote Control on, sorted by what it needs
from you: **Needs you**, **Working**, **Idle** and **Stopped**.
It tells you when a session starts to wait for you, so you can leave long tasks running and come back when needed.

It only watches. It reads `podbay-dev` and this machine's Claude session files, and never changes a session.

## Requirements

- macOS (Linux also builds; it needs the WebKitGTK 4.1 development packages).
- `podbay-dev` installed and signed in on this machine.
- Rust, and the Tauri CLI: `cargo install tauri-cli --version '^2' --locked`.

## Build and install

```sh
cd apps/pod-board/src-tauri   # from the repo root
cargo test                      # optional
cd ..
POD_BOARD_SIGN_IDENTITY="Apple Development" bash build-mac.sh   # or no variable; see the README
cp -R "src-tauri/target/release/bundle/macos/Pod Board.app" ~/Applications/
open ~/Applications/"Pod Board.app"
```

Without an identity the build is ad hoc, so macOS asks for Local Network again after each rebuild (README, "Keep
Local Network across rebuilds"). The first start on a new Mac asks for confirmation (right-click the app, then **Open**).

## Use

- **Window:** the title bar shows the Pod Cat logo and where you are; drag it to move the window. The sidebar
  filters by state, by **Source** (Pod Bay, Terminal, Desktop) and by **Projects**: click a row to filter, click it again or
  **Clear filter** to stop. A project's dot turns accent while a session there needs you; its foot has the theme switch (light, dark, follow the system). The button on the
  title bar (or ⌘B) folds the sidebar to its icons.
- **Look:** the same design system as Shape Board: Outfit for text (JetBrains Mono only for code), Lucide icons,
  soft state pills with an icon and a word, and the Pod Cat logo. The fonts and icons ship inside the app.
- **Board:** one lane per state. Switch between **Board** and **List** at the top right of the main panel. The cat
  in the summary row waves when a session needs you, types while one works, and sleeps otherwise.
- **Source:** each card says where its session runs: `Pod Bay · M0` (with the machine) or `Terminal · RC` (a
  terminal session on this machine with Remote Control on) or `Desktop` (a session of the Claude desktop app on this
  Mac). A session that two of them list shows once. A Desktop session with no process running shows as idle; one
  that was active in the last 12 hours and has no process shows in the Stopped strip as closed. Archived desktop
  sessions are not shown.
- **Why it waits:** a card that needs you says `Permission` when the session waits on a permission; else the
  `input needed` tag shows. The peek repeats it under "Why it waits".
- **Sub-agents:** a working session on this machine shows up to 3 running sub-agents and "+n more running"; the
  peek lists every sub-agent with its model and time. Sessions on other machines show no sub-agents (Pod Bay does
  not report them).
- **Open a session:** click a card. The Claude desktop app switches to that session. A Desktop session opens by its own
  link; that link was not run when this was written, so if a click does nothing, tell the owner.
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
