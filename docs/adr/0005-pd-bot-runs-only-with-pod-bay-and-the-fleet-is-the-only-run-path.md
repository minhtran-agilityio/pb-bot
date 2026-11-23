---
ticket: none
stage: design
status: accepted
confidence: high
decided_by: owner
date: 2026-11-22
---

# 0005. pd-bot runs only with Pod Bay, and the fleet is the only run path

## Context

pd-bot had three ways to run tickets: the `ship.js` Workflow script launched by `/pd-bot:pd-autopilot`, the fleet of Pod Bay sessions led by `/pd-bot:pd-orchestrate`, and a single agent. The old repository's ADR 0043 kept `ship.js` as one of the three. The main reason to keep it was that it runs where Pod Bay is not installed. The owner stated that pd-bot exists only to run with `podbay-dev`, so that reason is gone.

`ship.js` brought its own parts: an autopilot profile, a preflight script, a run board (`plugins/pd-bot/board/` (historical)), a run report script and a worktree per stage. The `pd-using-git-worktrees` skill and the per-step worktrees of `pd-implement` existed for the same isolation, which the Pod Bay workspace (a clean clone per session) now gives.

## Options

- Remove `ship.js`, `pd-autopilot`, the run board and the git worktree isolation of `pd-implement` and `pd-using-git-worktrees`; the fleet is the only run path (chosen)
- Keep `ship.js` as a deprecated fallback until the fleet has run several milestones
- Keep the three paths

## Decision

pd-bot needs Pod Bay. `/pd-bot:pd-orchestrate` is the only way to run tickets with agents. Removed: the `ship.js` Workflow script, the `pd-autopilot` skill with its profile, preflight and board start scripts, the run board, `run_report.py`, `ship-worktree-cleanup.sh`, the `pd-using-git-worktrees` skill, and their tests. The ticket grouping (`plan_groups.py` and `groups.md`) moved to `pd-orchestrate`. `pd-implement` runs its steps one at a time on the working branch of the session's checkout. The Pod Bay session board is `apps/pod-board/`.

Kept: the temporary detached worktrees that `pd-review` and `fleet-check.sh` create at a commit to check it without touching the worker's checkout, and the `clean-worktrees.sh` hook.

## Why

One run path is less to keep correct. The fleet already covers what `ship.js` did, and its gates are checked by `fleet-check.sh` and a separate reviewer session. A git worktree inside a Pod Bay workspace adds a second isolation layer that nothing needs.

## Consequences

pd-bot does not run without `podbay-dev`. A ticket that a fixed script would run with no orchestrator now needs an orchestrator session. The fleet gates depend on `fleet-check.sh`, which blocks only on a `feat/*` base today; making it block on every base is a follow-up. `pd-implement` steps no longer run in parallel.

## How to reverse

Restore the removed files from the commit before this change with `git checkout <sha>^ -- <path>`, and the CLAUDE.md sections that named them.
