# Pod Board design notes (UDS v3.0)

Pod Board uses the UI design system (UDS v3.0) that Shape Board uses. Shape Board's design system is described in
`apps/pod-shape/docs/DESIGN_SYSTEM.md` (v2.4). This file holds only what is specific to Pod Board: its tokens, its
parts, and where it departs from the UDS screens and why.

## Tokens: `ui/theme.css`

- **A. Shared tokens:** a verbatim copy of the three token blocks of `apps/pod-shape/src/styles/tokens.css`:
  the dark `:root`, the light `@media (prefers-color-scheme: light)` block and the light `:root[data-theme="light"]`
  block. Edit `tokens.css` first, then copy the blocks. `cargo test` (`src-tauri/tests/theme_tokens.rs`) fails when a
  shared token differs between the two files, in either theme.
- **B. Pod Board tokens:**
  - `--session-needs`, `--session-working`, `--session-idle`, `--session-finished` and their `-soft` pairs, as
    aliases of the shared tokens (accent, status-running, status-closed, success with status-open-soft). In dark,
    `--session-idle-soft` is fainter (alpha .12), so the idle word passes AA 4.5:1 on a card.
  - `--pressed-sh`, for segmented tracks.
  - `--pod-brand-bg`, `--pod-brand-cat` and `--pod-brand-eye`, for the logo and the cat only.
- **C. Fonts and base:**
  - Fonts: Outfit 400, 500 and 600, and JetBrains Mono 400. Each is latin + latin-ext woff2 in `ui/fonts/`, so no
    network is needed.
  - Body type is 14/22.
  - Font weight never goes above 600.
  - A single focus rule covers every control: `outline: 2px solid var(--ring); outline-offset: 2px`. Do not add
    per-part outline rules.
- **Theme:** the page sets `data-theme` to `light` or `dark`, with the choice stored under `pod-board-theme`
  ("follow the system" is resolved with `matchMedia`). The popover reads the same key.
- **Colours:** no raw colour in `index.html` or `popover.html`, except inside the Pod Cat logo drawing.

## Icons

There is one inline Lucide sprite per page. Icons use stroke 1.75 and sizes 12 (pills, tags), 14 (list icons,
peek button) or 16 (sidebar, buttons). Each meaning has one icon:

| Meaning | Icon | Meaning | Icon |
|---|---|---|---|
| Needs you | hand | Overview | house |
| Working | loader (spins) | Board / List | layout-grid / list |
| Idle | coffee | Peek | message-square |
| Stopped | moon-star | Refresh | refresh-cw |
| Dark / light / system theme | moon / sun / monitor | Sidebar | panel-left |
| Pod Bay error | triangle-alert | Close | x |

The menu bar images (`src-tauri/icons/tray-*.png`) are the same shapes, drawn black at 44x44 as template images.

## Shell

- **Title bar (48 px):**
  - Contents: the sidebar button, the Pod Cat logo (20 px), and `Pod Board › <page>`.
  - The window uses `titleBarStyle: Overlay` with `hiddenTitle`. `trafficLightPosition {18, 18}` centres the macOS
    window buttons on the bar, and `--tb-inset` (84 px on macOS) leaves room for them.
  - The bar and its words carry `data-tauri-drag-region`, which needs `core:window:allow-start-dragging`.
- **Sidebar (256 px, `--side`, radius 28):**
  - Items are 38 px pills. A shared thumb (`--surface-2` with `--card-sh`) slides to the current page.
  - The Machines group label is plain text, 12/16 at weight 600.
  - **SidebarStatus** (radius 18): the state dot and word (Live, Reading, Stale data, Paused, Pod Bay error), then
    "read at" and Pause.
  - The foot holds the theme switch.
  - **Collapsed (⌘B or a narrow window):** the sidebar is 64 px of icons with tooltips, and the theme switch stacks.
- **Main panel (`--surface-1`, radius 28):**
  - The page title is 26/34, with Refresh and the Board/List switch on the right.
  - Below it: the banner, the summary row, then the view.

## Parts

- **SummaryRow with AppCharacter:**
  - A neutral card with the cat in a 44 px round tile (`--side`), one sentence, one sub line, and count chips that
    are buttons.
  - It shows on every page.
  - The cat has three poses (waves, types, sleeps), drawn in `--pod-brand-*`. Under Reduce motion it stays still.
- **State pill (22 px):** a soft fill, an icon and the word. Lane heads, list groups, page heads, the peek and the
  popover groups all use it. Stopped is an outline pill.
- **Lane:** neutral, on `--bg`, radius 24, padding 12. The columns are 1.25 / 1 / 1. The state shows only in the
  pill.
- **SessionCard:**
  - Radius 24, padding 16/20, gap 8, `--card-sh`. On hover it lifts 3 px with `--card-sh-hover`.
  - Row 1: the title (15/24, weight 600), the time (a wait time in `--accent-text`), and the peek button.
  - Then the body line, then the meta row: place, tags and links.
  - Tags and links are 22 px pills:
    - `input needed`: needs-soft.
    - `finished`: finished-soft. This tag replaces the old left stripe.
    - `n queued`: neutral.
    - Links: outline pills in `--accent-text`.
  - **For #137:** the wait reason, the step line and the sub-agent list go between row 1 and the meta row, and the
    source chip goes first in the meta row.
- **List:** a single card with a state pill per group. Each row is 13/20 with a 26 px round soft state icon. Rows
  get no tint.
- **StoppedStrip:** a plain card (radius 18) that sticks to the bottom of the main panel, with outline chips. No
  blur. The lanes (or the list) scroll above it, so it covers no card (issue #159). A card title that would get less
  than 9rem puts its time and peek button on a second line, at the right.
- **Peek:**
  - A sheet on the right: radius 28, `--surface-2`, `--elev-lg`.
  - Labels are plain. Bubbles use `--surface-3` (the reply) and `--bg` with a border (your prompt), and code is
    JetBrains Mono.
  - Buttons: the primary is "Open in Claude", the secondary is "Read again".
- **Popover:**
  - The head shows the cat as a still drawing, or a warning triangle on an error.
  - Group titles are state pills, and rows are cards (radius 18).
  - The window height follows the rows, up to 460 px. The page resizes the window and keeps its top-left corner in
    place, using `capabilities/popover.json`.
  - The window keeps square corners, because rounding them needs a macOS private API.

## Departures from the UDS screens, and why

- **Lanes on `--bg` with radius 24:** this follows the prototype, not the UDS screen (`--surface-1` with radius 28).
  On a `--surface-1` panel, a `--surface-1` lane would not show.
- **Colours:** the darker AA light colours of `tokens.css` (DESIGN_SYSTEM.md §1.5) are used instead of the
  prototype's.
- **Focus:** a 2 px `--ring` outline is used instead of the prototype's 4 px halo, which is below 3:1 against the
  page.
- **Card spacing:** padding 16/20 and gap 8 (the SessionCard of UDS), not the prototype's 14/18 and 6.
- **Kept from the app, though the prototype drops them:**
  - Sidebar collapse and the icon-only sidebar.
  - The Stopped page and its count.
  - Cards open the session; the peek opens from its own button.
  - Do not disturb, the error banner and the dimmed view.
  - Every peek state.
  - The popover's Finished group and quiet state.
- **The sidebar button sits on the title bar,** so it still works while the sidebar is folded.
