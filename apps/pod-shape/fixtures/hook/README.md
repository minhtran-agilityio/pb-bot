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
