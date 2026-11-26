# Desktop session fixtures (issue #176)

Made-up files in the Claude desktop app's format, one per session, laid out as on a Mac under
`~/Library/Application Support/Claude/`: `claude-code-sessions/<id>/<id>/local_<uuid>.json`. No real data.
The reader's internal API does not exist yet, so the tests are `cargo test` by the worker (the reviewer checks they
exist, use these files unchanged and pass). Point the reader at `tests/fixtures/desktop` as the folder that holds
`claude-code-sessions`.

Acceptance item 1 of #176: "`cargo test`: a fixture folder of desktop session files gives sessions with source
`desktop`; an archived one is not shown; one that the Pod Bay or terminal fixture also lists shows once under that source."

| File (`local_…`) | Title | What it must produce |
|---|---|---|
| `d1000000-…-000000000001` | Index repo on desktop | Three cases, each from this file alone with the clock the test sets (`lastActivityAt` is 1795000100000). (a) Live: the test adds a `~/.claude/sessions` file whose `sessionId` is this file's `cliSessionId` (`d1d1d1d1-0000-4000-8000-000000000001`) and whose process the test treats as running, the same way the #137 tests treat the local fixture: one session, source `desktop`, title `Index repo on desktop`, project `alpha` (the last part of the folder name). (b) No live process, now = `lastActivityAt` + 1 h: no session; one Stopped entry, source `desktop`, title `Index repo on desktop`, project `alpha`. (c) No live process, now = `lastActivityAt` + 13 h: no session and no Stopped entry. |
| `d2000000-…-000000000002` | Archived desktop chat | `isArchived: true`: no session and no Stopped entry. |
| `d3000000-…-000000000003` | Desktop twin of a terminal session | Its `cliSessionId` is the `sessionId` of `../local/claude/sessions/1004.json` (`Remote twin`, Terminal · RC). Read together with the local fixture: `Remote twin` shows once, with source `terminal`, and no `desktop` session is made from this file. |
| `d4000000-…-000000000004` | Desktop twin of a Pod Bay session | Its `cliSessionId` is the `sessionId` of `../local/claude/sessions/1003.json` (`Pod Bay twin`, tmux `claude-pb-bot-02`), which #137 already shows as Pod Bay's. Read together with the local fixture and a Pod Bay row for that tmux session: it shows once, with source `podbay`, and no `desktop` session is made from this file. |
| `d5000000-…-000000000005` | Privacy check | The same three cases with `cliSessionId` `d5d5d5d5-0000-4000-8000-000000000005`: (a) live: one session, source `desktop`, project `delta-desk`; (b) no live process, now = `lastActivityAt` + 1 h: no session, one Stopped entry, source `desktop`, project `delta-desk`; (c) no live process, now = `lastActivityAt` + 13 h: nothing. |

Owner rule (2026-11-26, on #176): "a desktop session closed for more than 12 hours is not shown at all". A desktop file with no live process is never a session: it is at most a Stopped entry when `lastActivityAt` is under 12 h before now, and otherwise nothing.

Every file at once, with the local fixture and a Pod Bay row for tmux `claude-pb-bot-02`: with live processes for d1 and d5, exactly two `desktop` sessions (`Index repo on desktop`, `Privacy check`) and no desktop Stopped entry; with none and now = `lastActivityAt` + 1 h, no `desktop` session and exactly two desktop Stopped entries (the same two titles); with none and now = `lastActivityAt` + 13 h, no `desktop` session and no desktop Stopped entry. d2 (archived) gives nothing in every case.

Privacy (ticket "What to build" 7 and 8), checked on the model built from all five files in each of the three cases above (sessions and Stopped entries), serialized as the page gets it:
- it holds none of `FIXTURE-SECRET-TOKEN-176`, `mcp.example.invalid`, `fixture-mcp` (`remoteMcpServersConfig` is never read
  into the model);
- it holds no full path: none of `/home/someone`, `someone`, `secret`;
- it holds no `cliSessionId` and no `sessionId` of these files, except the one id the app needs to open that session in
  the desktop app (if opening is possible; the worker names which field and why in the test);
- reading the folder writes nothing: the fixture files are unchanged after the test (same bytes and the same file
  list).
