# Changelog

All notable changes to this harness are recorded here. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions are tracked in the root `VERSION`
file. Versions change only in the release PR (see the release section of `AGENTS.md`); the bump (**patch** for fixes and docs,
**minor** for a new skill or hook or a changed contract, **major** for a breaking change) is chosen when releasing.

## [Unreleased]

- `merge-guard.sh` now denies spellings that bash turns into `gh` or `merge`: brace expansion, a backslash line break, octal or hex escapes, a glob in the gh words, `GH`/`Gh`, and any merge-like payload when jq is missing (P5).
- `merge-guard.sh`: the wrapper denial names the workaround (put the command in a script file and run it by a path without the word). Prose that mentions merge next to `bash x.sh` stays denied: two security reviews broke both fixes tried; the bypasses they found are now regression tests (P1).
- A PR that changes the FE/UI (ticket kind FE or FE+BE, or a diff touching `.tsx .jsx .vue .svelte .css .scss .html`) needs a screenshot. Without one the PR still opens and the merge becomes `Merge: human (no UI screenshot attached)`: in `ship.js` (the final merge reason; a ticket is never blocked), `pd-finishing-a-development-branch` the fleet outcome contract and the `pd-work` toolbox (P2).

## [0.1.0] — 2026-11-20

- Initial release of the `harness` plugin: skills, agents, hooks and the `autopilot-run` ship workflow.
