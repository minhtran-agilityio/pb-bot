---
type: system
title: Pod Board Menu Bar App
description: Architecture of Pod Board, a Tauri 2 observer app that polls podbay-dev and local Claude session files, merges them into an allow-listed model of sessions by state, and presents a board window, menu bar popover and notifications, with strict privacy sanitising.
tags: [pod-board, tauri, rust, pod-bay, observability, privacy]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-26T15:02:34.678Z
sources:
  - id: openwiki-source-889630b78981060a7036c53d
    resource: repo://apps/pod-board/src-tauri/src/local.rs
  - id: openwiki-source-06a6576e5c93092857b95860
    resource: repo://apps/pod-board/src-tauri/src/main.rs
  - id: openwiki-source-091642323a7ea1b2f07fe98f
    resource: repo://apps/pod-board/src-tauri/src/model.rs
  - id: openwiki-source-0c455a278416d22306e9b3a9
    resource: repo://apps/pod-board/src-tauri/tauri.conf.json
generated: { by: "claude-code", at: "2026-11-26T15:02:34.678Z" }
---

# Pod Board Menu Bar App

<!-- openwiki: broken internal link [/openwiki/workflows/fleet-orchestration.md] link "/openwiki/workflows/fleet-orchestration.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
<!-- openwiki: broken internal link [/openwiki/pod-board/alerts-and-build-signing.md] link "/openwiki/pod-board/alerts-and-build-signing.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
Pod Board (`apps/pod-board/`) is a small Tauri 2 desktop app (Rust backend, plain HTML/CSS/JS pages, no Node, no server, no port) that shows every Pod Bay session on every machine, plus Claude terminal and desktop sessions of the local machine, grouped as **Needs you**, **Working** and **Idle**, with workspaces that have no session shown as **Stopped** chips. It **only observes**: it never starts, stops or edits a session, and it sends nothing over the network. It is the human-facing companion of the fleet ([Fleet Orchestration](/openwiki/workflows/fleet-orchestration.md)); alerts and build are covered in [Pod Board Alerts, Build and Signing](/openwiki/pod-board/alerts-and-build-signing.md).

## Module layout

| File | Responsibility |
|---|---|
| `src-tauri/src/main.rs` | Tauri setup, poll loop, shared snapshot, commands (peek, open session/link, Do Not Disturb), tray icon and menu, popover placement, notification and sound dispatch |
| `src-tauri/src/model.rs` | Pure transformation of raw inputs into the allow-listed `Model` (sessions, stopped, counts, machines), including state derivation, ordering, de-duplication and string safety helpers |
| `src-tauri/src/local.rs` | Reads this machine's Claude session files, sub-agent metadata and desktop-app session files with size and count limits; scrubs paths |
| `src-tauri/src/alerts.rs` | Notification, reminder, finished and Do Not Disturb state machine |
| `ui/index.html`, `ui/popover.html`, `ui/theme.css` | The board window and the menu bar popover; `theme.css` copies Shape Board's tokens |

## Data flow

`poll_loop` in `main.rs` runs on its own thread:

1. Run the read-only command `podbay-dev ls --all` (10 s timeout, output capped at 16 MB). On every 10th read it also runs `checkouts --all`, `machines` and `queue ls --all`, and probes machines with a known address but no rows (a 2 s TCP connect on a private address; "No route to host" marks the machine as *unreachable from Pod Board*, a message that names no cause because macOS gives the same error when the host is off or when Local Network access is denied).
2. Read the local Claude files (`local::read_with`, and the desktop app's folder on macOS only).
3. Build a `Raw` input, call `build(&raw)` to get a `Model`, feed it to `Alerts::on_read`, mark finishes, and store the result in the shared `Snapshot`.
4. Release the lock, then send notifications, sound and the tray flash, and `publish` the snapshot to the windows and tray.
5. Sleep `POLL_PAUSE` (3 s) or less when the Refresh button wakes it.

When `ls` fails, the Pod Bay part of the snapshot keeps its last read but is flagged stale so it raises nothing, while local terminal sessions are fresh and still alert (issue #158). The error text names only the failing subcommand, never stderr. The `podbay-dev` binary is found in `PATH` and then `~/.local/bin`, `/usr/local/bin`, `/opt/homebrew/bin`, and the child process gets those directories appended to its `PATH`, because a desktop app starts with a short `PATH` and `podbay-dev` needs Homebrew's `tmux` (issue #174).

## Sources, de-duplication and state

Each session carries a source: `podbay`, `terminal` (Claude session in a terminal with Remote Control on that Pod Bay does not list) or `desktop` (Claude desktop app, macOS). `merge_local` and `merge_desktop` in `model.rs` make a session that several sources list show once, under Pod Bay first, then Terminal, then Desktop:

- Only an **interactive `cli`** local session counts; a `claude -p` run is ignored. A local session whose Remote Control id or tmux session name matches a Pod Bay row lends that row its sub-agents instead of becoming its own row.
- A local tmux session is Pod Bay's to list: while this machine's Pod Bay list is unknown or unread, it is left out rather than shown as a terminal row.
- A desktop session already shown (same Claude session id) is skipped. One with a live process file takes its state, wait reason and sub-agents from `~/.claude/sessions/<pid>.json`; one with no process is closed and goes to the Stopped strip only if its last activity is under `CLOSED_MS` (12 hours). What cannot be known is left out rather than invented.

**State derivation.** For Pod Bay rows, `state_of` yields `needs` when `waiting_for` names something, when `asks` is truthy, or when `status` is anything other than `busy` or `idle` (including no status yet); otherwise `busy` or `idle`. Local sessions need you when `waitingFor` is set or their status is not `busy`/`idle`. `order` sorts needs first, then busy, then idle, with finished idle sessions first. The only `reason` the app derives is `permission`, from `waitingFor == "permission prompt"`. A session's identity across polls is its Remote Control id when present, otherwise machine, workspace and title.

## Privacy and safety design

- **Allow-listed model.** `model.rs` builds from named fields only. It never holds account, uuid, dir, path, remote URL, machine ids or addresses. Per-session links (PR, issue, artifact) are kept as kind, title and an `https://` URL, at most 8 per session, and only a URL held by the current read can be opened.
- **Input validators.** `safe_name` admits only plain letters, digits, `-`, `_`, `.` (not starting with `-`) before a name is passed to `podbay-dev` as an argument, so it cannot be read as a flag; `safe_url` admits only `https://` printable ASCII without quotes, brackets or backslashes; `session_id` accepts a Remote Control id only in a strict form, so it is safe in a `claude://code/<id>` link.
- **Bounded file reads.** `local.rs` looks at the newest 512 files in `~/.claude/sessions`, keeps at most 64 live sessions, reads each file up to 64 KB, treats a sub-agent as not running if its transcript has not changed for 15 minutes, and never opens sub-agent transcripts (`agent-*.jsonl`), only their `.meta.json`. A session is live only if its `pid` passes `alive` (on Linux the start time is also compared). Home paths and login names are scrubbed from text.
- **Peek is on demand.** `podbay-dev detail` runs only when the owner opens a peek, for that single session, with a 25 s timeout; only the last prompt and last reply are kept (the screen is dropped) and are truncated.
- **No network.** The page's Content Security Policy in `tauri.conf.json` allows only `'self'`, `data:` images and the Tauri IPC; a clicked link is handed to the system `open` (or `xdg-open`), never fetched by the app.

## Windows and tray

`tauri.conf.json` defines two windows: the 1180×720 `main` window (overlay title bar; closing it hides it, the app keeps watching) and a 340×460 borderless, always-on-top `popover` window that is created hidden. The tray icon shows the most urgent state with a count; a left click toggles the popover (placed under the icon, moving to the other display on a click there, and closed by an observed global click because the popover is not the key window when the board is on another display, issue #170); a right click shows a menu (open, start at login, always on top, Do Not Disturb, sound, quit). The Dock badge counts sessions waiting for you, and settings that must survive a restart (sound switch, Do Not Disturb end time) are stored in `settings.json` in the app's config folder.

## Limits worth knowing

A terminal session appears only if Claude Code writes its `~/.claude/sessions/<pid>.json` (child sessions started inside another Claude session do not); other machines show no sub-agents or step because `podbay-dev detail` has neither; a hard-killed session on macOS can linger until its pid is reused; and the desktop app's file has no status, so desktop state exists only while a matching process file does. The README lists what has and has not been verified on a real machine.

## Tests

<!-- openwiki: broken internal link [/openwiki/testing/test-suite-and-ci.md] link "/openwiki/testing/test-suite-and-ci.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
`cargo test` in `apps/pod-board/src-tauri` covers the model (ordering, allow list, links, desktop and local merging), alerts, the Pod Bay reader helpers, peek text, popover geometry and the theme-token check against Shape Board. `apps/pod-board/tests/acceptance/` runs headless Chrome over the two pages with mock data. See [Test Suite, Lints and CI](/openwiki/testing/test-suite-and-ci.md).
