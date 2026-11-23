# Artifact hook fixture

A fake output of the artifact hook (HANDOFF §11.3) for the step 2 demo and the Mac check. It never
writes to `~/claude-artifacts` or `~/.claude`.

```bash
eval "$(bash apps/pod-shape/fixtures/hook/make-demo.sh "$(mktemp -d)")"
cd apps/pod-shape && npm run tauri dev
```

`make-demo.sh <dest> [project folder]` writes `<dest>/claude-artifacts/` and `<dest>/artifact-log.tsv`
and prints two variables, quoted for the shell. Only a debug build (`tauri dev`, `tauri build --debug`)
reads them. `<dest>` must be new or empty, or a folder the script made before; it refuses the home
folder and the real hook folders. Use a private folder (`mktemp -d`), and keep probe logs and the demo
folder out of commits.

What it holds:

| Artifact id | What | Log |
|---|---|---|
| `Fx7hQ2protoShapeBoard` | the real prototype `docs/prototype/shape-board-prototype.html`, 2 versions | 6 columns; session folder `<project>/apps/pod-shape`, so auto-select picks the project at `<project>` |
| `SecProbe9Kc` | `files/security-probe.html`, the sandbox probe | 6 columns; plus a `.tmp` file that must be ignored |
| `OldNotes4col` | `files/old-notes.html` | the old 4-column form |
| — | `this line is broken` | a bad line: skipped, counted in the banner |

Security probe: add it to a project, open its preview, and check that every automatic row says
**blocked**. Then press the navigation buttons, drop a file outside the drop zone, and drop a file onto
the preview frame: the app must stay on its own page. The app log prints
`preview request, sec-fetch-dest = ...` for each frame load; note whether the engine sends it. Check it
on Linux and on macOS (dev and `tauri build --debug`).

Mac check list (step 2):

- The probe: every automatic row says **blocked**; the navigation buttons and both drops leave the app on
  its page. Note the `sec-fetch-dest` value the log prints.
- One frame load makes one `preview request` log line. If WKWebView requests a URL twice for one load, the
  second request gets the 404 of a used URL and the frame goes blank: report it.
- "Reload" above the frame loads the version again (a new URL each time).
- A real file drop from Finder on the Add artifact drop zone fills the dialog.
- The HTML textarea in the Add artifact dialog grows to 10 rows, then scrolls.
- A new file written by the real hook shows up without a restart (FSEvents).
