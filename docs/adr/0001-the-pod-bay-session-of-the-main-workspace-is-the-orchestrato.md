---
ticket: P3
stage: design
status: accepted
confidence: high
decided_by: owner
date: 2026-11-20
---

# 0001. The Pod Bay session of the main workspace is the orchestrator

## Context

The owner wants the first session of every repository that uses Pod Bay and the pd-bot plugin to know that it is the orchestrator, with no command to type. Pod Bay 0.1.22 sets no role and no environment variable in a session. It does set tmux options on each session it starts: `@tn_kind` is `claude`, `@claude_dir` is the checkout, and `@tn_context` is `<project>/<repo>` in the main workspace (src) and `<project>/<repo>@NN` in a parallel workspace (src-NN). A hook process inherits `TMUX_PANE`, so it can read these options in about 1 ms. A SessionStart hook runs again on resume, clear and compaction. The options are not documented by Pod Bay.

## Options

- A SessionStart hook that reads the tmux options of the session (chosen)
- Parse the tmux session name `claude-<project>-NN`: ambiguous when the repository has another name than the project
- A CLAUDE.md or rule fragment: every clone reads it, workers included, and the model must then detect its own role
- An output style or a committed `agent` setting: applies to every clone, not to one session
- Wait for a role signal from Pod Bay: nothing works until Pod Bay ships it

## Decision

A second SessionStart hook, `plugins/pd-bot/hooks/session-role.sh`, adds the orchestrator role text only to the Pod Bay session of the main workspace. It decides in this order: `PD_BOT_ROLE` (`orchestrator` gives the role, any other value gives none); the session is interactive (`CLAUDE_CODE_ENTRYPOINT` is `cli`) and inside tmux; the tmux session is Pod Bay's, its `@claude_dir` is the project directory and its pane was started as `claude`; `@tn_context` has no `@NN` suffix; the project config does not switch the role off. Every other session gets nothing: a worker, a reviewer and an advisor take their role from their brief. The role is on by default. A repository switches it off with `{"orchestrator": "off"}` in `.claude/pd-bot.json`. The hook fails open: any doubt or error prints nothing and exits 0. The owner sends Pod Bay a request for a documented role signal per session; until Pod Bay ships one, the hook uses `@tn_context`.

## Why

The tmux options are the only signal that exists today and that tells the workspace of a session without a guess. The entry-point check keeps a nested `claude -p` in the same pane (the stop-evidence judge is one) from getting the role. The directory and launch-command checks keep a shell the owner split off in the same tmux session from getting it. A parallel workspace also runs sessions that are not ticket workers (a release bot, a skill coach, reviewers, advisors), so the hook gives no role there.

## Consequences

The hook depends on an internal detail of Pod Bay. When Pod Bay renames the options, the hook is silent and no session gets the role; `tests/hooks/session-role.test.sh` pins the expected format. A repository with two Pod Bay projects, or the same repository on two machines, has one orchestrator per main-workspace session; a fleet run stays on one machine and its ledger is per repository. A Pod Bay user who writes code in the main workspace gets the role text and must switch it off.

## How to reverse

Remove the `session-role.sh` entry from `plugins/pd-bot/hooks/hooks.json`, or set `{"orchestrator": "off"}` in `.claude/pd-bot.json` of one repository, or `PD_BOT_ROLE=none` for one session.
