Screenshots for feature/pod-shape-artifact (Shape Board build step 2, "Artifact"), 1280x800.

The app ran on Linux (WebKitGTK, `npm run tauri dev`) against the hook fixture
(`apps/pod-shape/fixtures/hook/make-demo.sh`) in an isolated data folder, with one project (this pb-bot
checkout). The prototype screens come from headless Chrome on `docs/prototype/shape-board-prototype.html`.

- compare-<screen>-<theme>.png: the prototype on the left, the app on the right. Screens: projects, project
  (Artifacts tab), add (Add artifact dialog), preview (preview Dialog with the real prototype running in
  the sandboxed frame). The prototype has mock data; the app shows the fixture.
  In compare-projects-* the app shows "Artifact hook not found": that run had no fixture variables set
  (the real ~/claude-artifacts does not exist on this Linux machine).
- app-new-artifacts-bad-line-light.png: the New artifacts box from the fixture (auto-selected project for
  the two artifacts published from this repo; the old 4-column one asks for a project) and the banner for
  the bad log line.
- app-updated-v3-light.png: a new hook file for an added artifact, picked up while the app ran: "Updated", v3.
- app-add-errors-dark.png, app-add-file-dark.png, app-paste-new-version-dark.png: field errors, a file chosen
  through the drop zone, and a pasted new version ("Saved version 2").
- app-overview-tab-light.png: Overview tab (EmptyState until build step 5).
- linux-prototype-in-frame.png: the real prototype inside the preview:// sandboxed frame (engine check).
- linux-probe-*.png, app-security-probe-dark.png: the security probe inside the frame: 24 automatic probes,
  0 reachable; CSP violations seen; the navigation probes stay in the frame.
