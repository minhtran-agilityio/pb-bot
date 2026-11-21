# Changelog

All notable changes to this harness are recorded here. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions are tracked in the root `VERSION`
file. Versions change only in the release PR (see the release section of `AGENTS.md`); the bump (**patch** for fixes and docs,
**minor** for a new skill or hook or a changed contract, **major** for a breaking change) is chosen when releasing.

## [Unreleased]

- Added: `fleet-check.sh` checks the reviewer's intent verdict (`intent.verdict` in the receipt must be `match`) and the report's `needs_review` decisions (a mismatch unless its title is in the `--decisions-seen <file>` list of titles the owner saw). The receipt gains an `intent` object; `match` with a non-empty `gaps` list is a mismatch. `fleet-check.json` now records the `--base` it checked.
- Added: `fleet-gate.sh` decides whether a fleet ticket pull request may go into its `feat/*` branch without the owner: a block-mode `fleet-check.json` for the head with the suite run and passed, green CI, and an open pull request on exactly the run's `feat/<slug>` (`--base`) at that head with no `pd-bot:owner-merge` label, where the checked base is in the feature branch's history (so no unchecked commit goes in). The orchestrator puts that label on a fix where the worker decided a requirement that was not clear. It changes nothing; on a pass it prints the one `gh pr merge` command, which `merge-guard.sh` still judges. `pd-orchestrate` can now run a milestone on a feature branch.
- Fixed: two tests that failed on a busy machine (P7). The clean-worktrees "item 2" time-budget case now uses a 10s budget, because the scan of every process's working directory alone took over 3s with about 2000 processes. The board-start `--no-open` case now waits up to 10s for the board to answer, because node can start after the 3s that `board-start.sh` waits. Tests only; no hook or script changed.

## [0.2.0] — 2026-11-21

- `merge-guard.sh` now denies spellings that bash turns into `gh`, `merge` or a wrapper: brace expansion, a glob or brace in the gh words, octal, hex and unicode escapes (decoded before matching), `GH`/`Gh` and `Bash`/`SH`, a gh name built from variable slices, and any merge-like command with a backslash line break. Without jq it checks the command field only (P5).
- Added: the `session-role.sh` SessionStart hook tells the Pod Bay session of the main workspace (`src`) that it is the orchestrator: it checks, coordinates and delegates hands-on work. Sessions in `src-NN` and sessions outside Pod Bay get nothing. Switch it off with `{"orchestrator": "off"}` in `.claude/pd-bot.json` or `PD_BOT_ROLE=none`; `/pd-bot:pd-setup` asks about it.
- Changed: `pr-opened.sh` tells the orchestrator session to hand the `/code-review` to another session; every other session keeps the old reminder.
- Fixed: `pr-opened.sh` fired on any command that held the text `gh pr create` (a grep, a scan, a failed create). It now fires only when `gh pr create` is a command word (not text inside an argument) and the output holds a pull request URL. A script that runs `gh pr create` inside it gets no reminder.
- Changed: `docs/adr/` is tracked again and starts at ADR 0001; the rest of `docs/` stays local.
- `merge-guard.sh`: the wrapper denial names the workaround (put the command in a script file and run it by a path without the word). Prose that mentions merge next to `bash x.sh` stays denied: two security reviews broke both fixes tried; the bypasses they found are now regression tests (P1).
- A PR that changes the FE/UI (ticket kind FE or FE+BE, or a diff touching `.tsx .jsx .vue .svelte .css .scss .html`) needs a screenshot. Without one the PR still opens and the merge becomes `Merge: human (no UI screenshot attached)`: in `ship.js` (the final merge reason; a ticket is never blocked), `pd-finishing-a-development-branch` the fleet outcome contract and the `pd-work` toolbox (P2).
- Require a screenshot on every FE/UI PR, else Merge: human (P2) (PR #3)
- merge-guard: name the workaround in the wrapper denial; regression tests for 11 bypasses (P1) (PR #4)
- feat(hooks): give the Pod Bay main-workspace session the orchestrator role (PR #5)
- merge-guard: deny spellings that bash turns into gh, merge or a wrapper (P5) (PR #6)

## [0.1.0] — 2026-11-20

- Initial release of the `harness` plugin: skills, agents, hooks and the `autopilot-run` ship workflow.
