# Pod Board

A small desktop app that shows every Pod Bay session on every machine, and the Claude terminal sessions of this
machine that have Remote Control on, by state: `Needs you` (a dialog waits for a human), `Working`, `Idle`.
Workspaces with no session show as small `Stopped` chips. It only observes.

It is a Tauri 2 app (Rust and one HTML page). It needs no server, no port and no Node.

## What it does

- **Source:** each session says where it runs, with one outline chip (icon and word): `Pod Bay · <machine>` for a
  session Pod Bay lists, `Terminal · RC` for a Claude session in a terminal of this machine with Remote Control on,
  which Pod Bay does not list. A session that both list (the same tmux session or the same Remote Control id) shows
  once, as Pod Bay's. The chip is on every card, the peek head and every popover row; a list row shows its icon with
  the words as a tooltip. The machine shows only on a Pod Bay chip. A Desktop source (Claude desktop-app sessions)
  is not shown: no file of this machine lists them (see Limits).
- **Why it waits:** a waiting session whose file says it waits on a permission shows `Permission` under its title,
  in the peek ("Why it waits") and in its popover row. Otherwise only the `input needed` tag shows.
- **Sub-agents:** a session on this machine (Pod Bay or terminal) shows its sub-agents: a box in a working card with
  at most 3 running rows and "+n more running", a tag `n sub-agents running` (or `done`), `n sub-agents` in list and
  popover rows, a "sub-agents" count in the summary row, and the full tree in the peek (type, task, model, running
  or how long it took; a nested sub-agent is indented). A step line ("Edit: …") shows when the data holds one; today
  none does, so a working card shows Pod Bay's description as before.
- **Dock:** one icon, the Pod Cat; a click opens the window. The badge on the icon is the number of sessions that
  wait for you. The running app sets the Dock icon itself: a Dock that pinned Pod Board before the Pod Cat icon kept
  the old icon from its cache, even after a new install (issue #159).
- **Menu bar:** an icon for the most urgent state and its count: a hand (needs you), a loader (working), a cup (idle),
  a moon with a star (nothing running), or a warning triangle when Pod Bay cannot be read (with the count of
  sessions that wait: the last Pod Bay read and this machine's own sessions, which still alert). They are the same Lucide
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
  window), a sage sidebar that filters by state, by Source (Pod Bay, Terminal) and by project (a project's dot turns
  accent, with a tooltip, while a session there needs you; each filter row toggles, "Clear filter" ends all), and a
  main panel with the page title, one summary row (working, sub-agents, idle, stopped)
  and three lanes (1.25 / 1.25 / 1). Offline machines keep their chip in the Stopped strip. Closing the window hides it; the app keeps watching. The button on the title bar (or ⌘B)
  collapses the sidebar to its icons and expands it again; the choice is remembered. The theme switch (light, dark,
  follow the system) is at the foot of the sidebar; the popover follows the same choice.
- **Open a session:** click a card (or press Enter on it). A terminal session opens by its Remote Control id too. On macOS this opens `claude://code/<id>`, so the Claude
  desktop app switches to that session; elsewhere it opens `https://claude.ai/code/<id>` in the browser.
- **Peek:** a speech-bubble button on each card (always shown on a card that needs you, on hover for the others)
  opens a side panel with the session's last reply and your last prompt, so you can see what it asks without
  switching to it. It reads them on demand with `podbay-dev --on <machine> detail <name>`; another machine can take
  about ten seconds. A read is kept for 20 s; "Read again" reads it now. Esc or × closes the panel.
- **Menu bar popover:** a left click on the menu bar icon opens a small panel under it with the sessions that need
  you, the ones that finished and the ones working; a row opens that session in the Claude app, and "Open Pod Board" opens the window. A
  right click (or Control-click) shows the menu. The popover closes when you click anywhere else: on macOS the app watches mouse clicks, because the popover is not the key window on a display when the board's window is on another one (issue #170). A click on the menu bar icon of the other display moves the popover there.
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
cargo tauri build          # macOS: target/release/bundle/macos/Pod Board.app (ad hoc; use build-mac.sh below to sign)
```

Linux needs the WebKitGTK 4.1 development packages. The app looks for `podbay-dev` in `PATH`, then in
`~/.local/bin`, `/usr/local/bin` and `/opt/homebrew/bin`, because a desktop app starts with a short `PATH`.

### Keep Local Network across rebuilds (macOS)

macOS ties the Local Network permission to the app's code signature. A build signed ad hoc (`-`) has its cdhash as
designated requirement, so every rebuild is a new app to macOS and the permission granted to the last build does not
apply. Sign every build with one fixed local identity and the requirement stays the same. `build-mac.sh` does this
when the keychain holds a code-signing certificate named `Pod Board Local` (a different name: set
`POD_BOARD_SIGN_IDENTITY`). Without it the build still works, ad hoc, and says so. The repo holds no certificate, key
or personal name; the script never creates or imports one.

Create the certificate once, by hand:

1. Open **Keychain Access**, then the menu **Keychain Access > Certificate Assistant > Create a Certificate…**
2. Name: `Pod Board Local`. Identity Type: **Self Signed Root**. Certificate Type: **Code Signing**. Create.
3. Check that the keychain lists it: `security find-identity -p codesigning` shows `"Pod Board Local"`. If `codesign`
   later says it cannot find the identity, open the certificate in Keychain Access, **Trust**, and set **Code Signing**
   to **Always Trust**.

Check the signature of an installed or built app:

```
codesign -dr - ~/Applications/"Pod Board.app"
```

- Signed with the certificate: `designated => identifier "io.pdbot.podboard" and certificate leaf = H"…"`. The same
  line after the next build.
- Ad hoc: `designated => cdhash H"…"`, a different value after each build.

The first signed build asks for Local Network once; allow it. Later builds signed with the same certificate do not
ask again. If a machine still shows as unreachable after that, check **System Settings > Privacy & Security > Local
Network**: **Pod Board** must be on. If it is on and the line stays, switch it off and on, quit and start the app
again. If **Pod Board** is not in the list at all, the app has not asked yet: quit it and start it again. If you
rebuilt it ad hoc in between, the list can hold an entry for an older build; switch that one off too.

### One installed copy (macOS)

Every new build replaces the old one: after a build there is exactly one `Pod Board.app` on the machine, in
`~/Applications`. Old copies are not kept (a second copy shows up in Spotlight, and the Dock, Launchpad and
"Open with" can start the stale one). Install by moving the new bundle in, not by copying it, so no copy stays in
`target/`:

```
cd apps/pod-board
bash build-mac.sh                                   # cargo tauri build, signed when the certificate exists (below)
pkill -x pod-board || true                          # quit the running app (pkill exits 1 when it is not running)
APP="Pod Board.app"
mkdir -p ~/Applications
[ -e ~/Applications/"$APP" ] && mv ~/Applications/"$APP" ~/.Trash/"Pod Board $(date +%Y%m%d-%H%M%S).app"
mv src-tauri/target/release/bundle/macos/"$APP" ~/Applications/
open ~/Applications/"$APP"
mdfind "kMDItemFSName == '$APP'"                    # check: only ~/Applications/Pod Board.app
```

- The old copy goes to the Trash (`mv` to `~/.Trash`, recoverable), never `rm -rf`.
- Whatever else `mdfind` lists is stale: a bundle in another checkout's `target/release/bundle/macos/` (`src`, `src-NN`,
  an older clone), a backup, a test copy. Move each to the Trash too, then run the check again. Empty the Trash
  yourself when you are sure.
- Do not keep a `.app` as a backup. To go back, check out the older commit and build it, or keep a zip
  (`ditto -c -k --keepParent "Pod Board.app" PodBoard-<date>.zip`), which Spotlight does not list as an app.
- To try a build without installing it, run it from `target/` and move it to the Trash when you are done.
- An agent that builds Pod Board follows this and names, in its report, the copies it moved to the Trash.

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
- On every read it also reads this machine's Claude session files, with no hook and no write:
  - `~/.claude/sessions/<pid>.json`, one per running Claude session: `status`, `waitingFor`, `bridgeSessionId` (the
    Remote Control id), `tmux`, `entrypoint`, `kind`, `name`, `statusUpdatedAt`; `cwd`, `sessionId` and `pid` only
    to find the sub-agent folder and to check that the process still runs. When a file has no `statusUpdatedAt`, its
    `startedAt` gives the time instead. A file whose process is gone is skipped (on Linux also one whose
    `procStart` differs from the process's start time, so a reused pid does not count); so is a missing, broken or
    odd file, a link or a pipe. The newest 512 files are looked at and at most 64 live sessions kept, each file at
    most 64 KB. A folder's listing is used again while the folder has not changed (for at most a minute); each
    session file is read on every poll.
  - The environment variables `HOME`, `USER` and `LOGNAME`, only to know the home folder and the login names to hide
    (below).
  - `/proc/<pid>/stat` (Linux only), for the `pid` of each session file: only its start time (field 22) is read and
    compared with `procStart`.
  - `~/.claude/projects/<folder>/<sessionId>/subagents/agent-*.meta.json`, the sub-agent files: `agentType`,
    `description`, `model`, `spawnDepth`, `finished`. The sub-agent transcripts next to them (`agent-*.jsonl`) are
    never opened: only their file times give the start and the end. An unfinished sub-agent whose transcript has not
    changed for 15 minutes is not counted as running. A finished sub-agent's file is read once.
  - The model keeps from these only the fields above that the page shows; the project is the last folder name of
    `cwd` (`~` for a home folder: `$HOME`, `…/home/<user>`, `/Users/<user>`, `/root`, or a folder named like the
    login). Text from these files first loses control, text-direction and zero-width characters. Then a path that
    starts with `/` at the start of the text or after a space or an opening mark (`(`, `[`, `"`, `=`, …) loses its
    folders: a home path (`$HOME/…`, `…/home/<user>/…`, `/Users/<user>/…`, `/root/…`) becomes `~/…`, a path that
    ends at the login name becomes `~`, and any other path of two or more parts keeps only its last part
    (`/srv/data/x/log.txt` becomes `…/log.txt`); a one-part path (`/tmp`) stays. In every other word with a `/` in
    it (`file:///…`, `host:/…`, `a,/…`), a whole folder name after a `home` or `Users` folder, or equal to a login
    name, becomes `~`, but the rest of the path stays. So the model holds no login name in a path, no home path, no account, no
    Claude session id and no process id; the only id it holds is the Remote Control id. A path below the home folder
    (`~/…`), the rest of a path in such a word, and a name written as a plain word in the text (a sub-agent's task)
    can still show. Prompt, reply and transcript text are read only when a peek opens, and only for Pod Bay
    sessions (with `podbay-dev detail`).
- It runs no `podbay-dev detail` in the background: on 2026-11-25 its `activity` held only `you` and `reply`
  entries, with no step and no sub-agents, so a read per poll would cost up to ten seconds per machine for nothing.
- State: `asks: true` or no `status` yet is `needs`; else `status: "busy"` is `busy`; else `idle`. A terminal
  session needs you when its `status` is `waiting` (or its file names a `waitingFor`).

## Limits

- A terminal session shows only when Claude Code writes its `~/.claude/sessions/<pid>.json`. Claude Code 2.1.332
  writes it for an interactive session whether or not it runs in tmux, but not for a child session: a `claude` started
  from inside another Claude session (its environment holds `CLAUDE_CODE_CHILD_SESSION`) writes no file, so it does
  not show (issue #159).
- Started from Finder, the Dock or at login, the app has only `/usr/bin:/bin:/usr/sbin:/sbin` in its PATH. The
  `podbay-dev` it runs gets `~/.local/bin`, `/usr/local/bin` and `/opt/homebrew/bin` after that, because it needs `tmux`
  (Homebrew) to list this machine's sessions; without it this machine listed none (issue #174).
- Other machines (U0 and so on): the board shows what `podbay-dev` returns. Pod Bay itself reaches a machine only
  some of the time: on 2026-11-25, in a shell, `podbay-dev machines` said U0 `online: true, password_current: true`
  and a few minutes later `online: false, password_current: false`, and `ls --all` then returned no U0 rows; the board
  said `U0 offline · password out of date`, which was true. The bundle is signed with the local certificate when
  `build-mac.sh` finds it (above); an ad hoc build has the cdhash as requirement and loses the Local Network permission at
  every rebuild (issue #177). Before that, a code signature with the identifier `io.pdbot.podboard` against the linker's
  `pod_board-<hash>` was tried in place on the installed app and showed no clear difference, because U0 went on and off
  in the same minutes. The bundle also says why it asks for the local network (`NSLocalNetworkUsageDescription` in
  `src-tauri/Info.plist`).
- When the app gets "No route to host" from a machine's address that Pod Bay lists (a private address such as
  `192.168.x.x`), the Stopped strip adds `<machine> unreachable from Pod Board (no route to host)` next to Pod Bay's own
  `offline` or `password out of date` chip. The line names no cause: macOS gives the same error when the host is off or
  not on this network and when Local Network access is not allowed for the app (System Settings > Privacy & Security >
  Local Network; the `podbay-dev` the app starts inherits it).
- When Pod Bay gives no address for another machine and `ls --all` returns no sessions for it (Pod Bay lists it offline,
  or names it and does not read it), the app has nothing to try. The Stopped strip then adds
  `<machine> unreachable from Pod Board (Pod Bay gave no address)`. This is all the app knows: the machine being off, an
  out-of-date password and a refused Local Network access look the same here, and the chip's tooltip says so. The
  chip does not show before the first read of `ls --all` or while the last read failed.
- On macOS a session killed hard (its file left behind) shows until its pid is reused or the file is removed: the
  format of `procStart` there was not seen, so only the pid is checked.
- Sources not shown, and data not available (step 0, 2026-11-25, on Linux and on M0):
  - **Desktop:** the Claude desktop app wrote no session file on M0, and its own folders are not read. No Desktop
    source, Desktop filter or closed desktop sessions in the Stopped strip.
  - **Reason:** `waitingFor` is `permission prompt` for a permission and for a plan approval (they cannot be told
    apart) and `input needed` for a question and for other input (they cannot be told apart either). No file holds
    the command, so the reason line has no command text. A question or a plan approval shows only `input needed` or
    `Permission`.
  - **Step line:** no session file and no `podbay-dev detail` field holds the current step; it would need the
    transcript, which the app does not read on a poll. The page shows a step when a future source gives one.
  - **Other machines:** their sessions show no sub-agents and no step: `podbay-dev detail` has neither (AC 13 and 14
    of #137 are not available).
- Verifies: `cargo test` (the model and its links, alerts and the finished rule, the Pod Bay reader, the peek text,
  the menu bar count, the popover position, and the theme tokens against Shape Board's). The look:
  `bash apps/pod-board/tests/acceptance/run.sh` (headless Chrome over the two pages, no data).
- Does not verify on the app (Mac checks): the source chips, filters, sub-agent rows and reason on live data; a
  click that opens a terminal session; alerts for a terminal session (the rule itself is in `cargo test`).
  `bash apps/pod-board/tests/acceptance/run_137.sh` checks the pages with mock data, run with
  `POD_BOARD_SKIP_DESKTOP=1` since Desktop is left out.
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
- The app is not notarized, and the certificate above is local to one Mac. On another Mac, Gatekeeper asks before the first start.
- It shows what Pod Bay on this machine sees. A machine that this Pod Bay lists as offline has no sessions here.
