# Pod Board UDS acceptance tests (issue #136)

Run: `bash apps/pod-board/tests/acceptance/run.sh` (python3; page checks need Chrome or Chromium).

Manual on M0 (macOS tester, not tested here): title bar look and window buttons, drag, Dock and bundle icon,
menu bar images, popover on screen and its height, restart persistence of theme and sidebar, Reduce motion,
and every criterion that needs live session data (state pills, cards, wait time, `finished` tag, Pod Bay error,
cat poses, click/peek/Esc/link buttons, popover groups).


# Pod Board acceptance tests (issue #137: sources, wait reason, step, sub-agents)

Run: `bash apps/pod-board/tests/acceptance/run_137.sh` (python3 and Chrome/Chromium; with no Chrome the page tests skip loudly).
Skip the Desktop rows with `POD_BOARD_SKIP_DESKTOP=1 bash apps/pod-board/tests/acceptance/run_137.sh`. Desktop tests live in
classes whose name ends in `Desktop` and in tests marked `needs_desktop`. Owner decision: when no Claude desktop-app sessions are
found, the Desktop source is left out and its rows are dropped; then the worker runs with that variable set and says so.

Test names: `AC<n>` counts the checkboxes of the ticket's "Acceptance criteria" from the top (1 = first `cargo test` box ... 17 = the README
and GUIDE box). A name with only `AC` (no number) proves a "Changes" or "Gaps" line of the ticket that has no checkbox.

## Mock contract (the worker must follow it)

The ticket does not publish the shape of the new fields, so the tests define it. The page must accept exactly these names in the
`snapshot` payload (the same shape the pages read today: `ok`, `read_at`, `counts`, `sessions`, `stopped`, `machines`). The mock is
`mock137.js`; it replaces `window.__TAURI__` (`core.invoke('snapshot' | 'peek')`, `event.listen`). The Rust model must serialize these names.

Session, new fields (all optional except `source`):
- `source`: `"podbay"`, `"desktop"` or `"terminal"`. Chip text: `Pod Bay · <machine>`, `Desktop`, `Terminal · RC`.
- `reason`: `{ kind: "permission" | "question" | "plan", detail: string }`. Words shown: `Permission`, `Question`, `Plan approval`. Absent means no reason.
- `step`: string, for example `Edit: src/local.rs`. Absent means show the Pod Bay description (`what`), as today.
- `agents`: `{ running: n, done: n, list: [ { type, task, model, since (ms), done (bool), took_s, result, step, depth } ] }`. Absent means none.
  `depth` 0 is a direct child of the main session; 1 is nested.
Stopped entry for a closed desktop session: `{ source: "desktop", title, project, ws: "", machine, closed (ms) }`.
Peek answer (`invoke('peek')`) stays `{ shared, prompt, reply, at }`; the reason and the tree come from the session in the snapshot.

Page surface the tests rely on (beyond ids that exist today): sidebar groups are `p.group` headings named `Source` and `Projects`, each row a
`#nav` button with a `.lbl` label and `.n` count; a project dot is the existing `.dot` element and is the `--accent` colour when a
session there waits, with `title` or `aria-label` containing "need" or "wait"; card text carries the chip, the reason, the step, the
sub-agent rows, `+n more running` and the `n sub-agents running|done` tag; a list row has an icon (svg) with a `title` or `aria-label`
holding the source words and no visible source text; the peek head `#peekMeta` has the chip, `#peekBody` has `Why it waits` and the tree
(nested rows indented); the popover row has the chip, the reason, `n sub-agents` and a `needs you` badge; the summary row buttons are, in
order, `n working`, `n sub-agents`, `n idle`, `n stopped`; `#crumb` names the active filter; a button `Clear filter` shows while a filter is on.

## Every criterion

| Criterion (ticket "Acceptance criteria", in order) | Where |
|---|---|
| 1-6 `cargo test` local reader, privacy, sub-agent counts, wait reason, alerts, broken file | cargo test by worker (reviewer checks): the internal API does not exist yet |
| 7 source on card, list row, peek head, popover row | `SourceChips` (+ `FiltersDesktop` for Desktop) |
| 8 Source and project filter, breadcrumb, counts | `SidebarFilters`, `FiltersDesktop` |
| 9 project accent dot and tooltip | `ProjectDot` |
| 10 4 sub-agents: step line, 3 rows, +1 more, summary count, peek | `StepAndSubagents` |
| 11 permission reason on card, peek, popover | `WaitReason` |
| 12 card click opens Pod Bay, desktop, terminal session | manual on M0 (opens an app) |
| 13 `cargo test` detail fixture with activity | cargo test by worker (reviewer checks) |
| 14 other machine, step and sub-agents within two polls | manual on M0 (needs a second machine and real timing); the card data itself is `StepAndSubagents` on a `U0` session |
| 15 desktop session wait: Dock badge, sound, notification | manual on M0 |
| 16 offline machine chip in the Stopped strip | `SidebarFilters.test_AC16_...` |
| 17 README and GUIDE | `Docs` |
| Changes 8 lanes 1.25 / 1.25 / 1 | `Layout` |
| Changes 7 closed desktop sessions in the Stopped strip | `FiltersDesktop` |
