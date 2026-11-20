# Changelog

All notable changes to this harness are recorded here. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions are tracked in the root `VERSION`
file. Versions change only in the release PR (see the release section of `AGENTS.md`); the bump (**patch** for fixes and docs,
**minor** for a new skill or hook or a changed contract, **major** for a breaking change) is chosen when releasing.

## [Unreleased]

- Added: the `session-role.sh` SessionStart hook tells the Pod Bay session of the main workspace (`src`) that it is the orchestrator: it checks, coordinates and delegates hands-on work. Sessions in `src-NN` and sessions outside Pod Bay get nothing. Switch it off with `{"orchestrator": "off"}` in `.claude/pd-bot.json` or `PD_BOT_ROLE=none`; `/pd-bot:pd-setup` asks about it.
- Changed: `pr-opened.sh` tells the orchestrator session to hand the `/code-review` to another session; every other session keeps the old reminder.
- Fixed: `pr-opened.sh` fired on any command that held the text `gh pr create` (a grep, a scan, a failed create). It now fires only when the output also holds a pull request URL.
- Changed: `docs/adr/` is tracked again and starts at ADR 0001; the rest of `docs/` stays local.
- `merge-guard.sh`: the wrapper denial names the workaround (put the command in a script file and run it by a path without the word). Prose that mentions merge next to `bash x.sh` stays denied: two security reviews broke both fixes tried; the bypasses they found are now regression tests (P1).
- A PR that changes the FE/UI (ticket kind FE or FE+BE, or a diff touching `.tsx .jsx .vue .svelte .css .scss .html`) needs a screenshot. Without one the PR still opens and the merge becomes `Merge: human (no UI screenshot attached)`: in `ship.js` (the final merge reason; a ticket is never blocked), `pd-finishing-a-development-branch` the fleet outcome contract and the `pd-work` toolbox (P2).

## [0.1.0] — 2026-11-20

- Initial release of the `harness` plugin: skills, agents, hooks and the `autopilot-run` ship workflow.
