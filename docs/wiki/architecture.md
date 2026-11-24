---
title: Architecture
read_when: you need to know which part does what, or where a file lives
sources:
  - CLAUDE.md
  - AGENTS.md
  - .gitignore
  - docs/adr/0005-pd-bot-runs-only-with-pod-bay-and-the-fleet-is-the-only-run-path.md
  - plugins/pd-bot/skills/pd-orchestrate/references/podbay.md
  - plugins/pd-bot/skills/pd-orchestrate/references/protocol.md
  - apps/pod-board/README.md
  - apps/pod-shape/CLAUDE.md
checked: 2026-11-24
---

# Architecture

## What the repository is

> source: CLAUDE.md
> Skill framework for Claude Code (the only supported runtime) with Pod Bay (`podbay-dev`, required): reusable prompt-based workflows from ticket to merged PR.

The repository holds three products:

| Part | Path | What it is |
|---|---|---|
| pd-bot plugin | `plugins/pd-bot/` | Skills, agents, hooks and one rule file for Claude Code |
| Pod Board | `apps/pod-board/` | A Tauri 2 desktop app that shows every Pod Bay session by state. It only observes |
| Shape Board | `apps/pod-shape/` | A Tauri 2 + React desktop app: grill a prototype, then create GitHub issues ([shape-board.md](shape-board.md)) |

> source: apps/pod-board/README.md
> It is a Tauri 2 app (Rust and one HTML page). It needs no server, no port and no Node.

## The plugin

| Folder | Holds | How a session gets it |
|---|---|---|
| `plugins/pd-bot/skills/<name>/SKILL.md` | One skill each, run as `/pd-bot:<name>` | Name and description in every session; the body when the skill runs |
| `plugins/pd-bot/skills/<name>/references/` | Detail that a skill tells the agent to read | Only when the skill says so |
| `plugins/pd-bot/agents/*.md` | Subagent roles (`pd-bot:<role>`), with model, effort and tools | When a session starts that agent |
| `plugins/pd-bot/hooks/` | `hooks.json` and the hook scripts | Claude Code runs them ([hooks.md](hooks.md)) |
| `plugins/pd-bot/rules/behavior.md` | The one behavior rule | A SessionStart hook prints it into every session |
| `plugins/pd-bot/scripts/` | Run-time scripts: `fleet-check.sh`, `fleet-gate.sh`, `fleet-settings.py`, `write_adrs.py`, `wiki_check.py` | A skill calls them |
| `plugins/pd-bot/templates/` | Scaffold files for `specs/` and `docs/solutions/` | `/pd-bot:pd-setup` copies them |

> source: CLAUDE.md
> To develop, run `claude --plugin-dir plugins/pd-bot` from the repo root.

Outside the plugin: `scripts/` (CI, lint and release scripts with their tests), `tests/` (hook and
script tests, fixtures, ticket evals), `.github/workflows/` (CI and release), `docs/adr/` (decisions),
`docs/wiki/` (this wiki).

## Sessions, workspaces and roles

Pod Bay runs Claude Code sessions in tmux. Each session works in a checkout of a workspace.

> source: plugins/pd-bot/skills/pd-orchestrate/references/podbay.md
> Pod Bay runs Claude sessions in tmux, each in a checkout of a workspace: `src`, or a parallel workspace `src-NN` of clean clones.

> source: CLAUDE.md
> Each session works in its own Pod Bay workspace (a clean clone), so a session never needs a git worktree for isolation.

| Role | Where | How it gets the role | Default model |
|---|---|---|---|
| Orchestrator | the session of `src` | the `session-role.sh` hook | Opus 5.5 |
| Worker | one `src-NN` per ticket | the `assign` brief, skill `pd-work` | Sonnet 5.5 (S, M), Opus 5.5 (L) |
| Reviewer | its own `src-NN` | the `assign` brief, skill `pd-review` | Opus 5.5 |
| Tester | its own `src-NN` (milestone) | `feature-test-request` | set by the orchestrator |
| Advisor session | its own `src-NN` | `advise-request`, skill `pd-advise` | Fable 5.2 |
| Storehouse keeper | a long-running session on Linux | `apps/pod-shape/docs/storehouse-keeper.md` | not stated in git |

Sessions talk with `SendMessage`. A fleet run stays on one machine:

> source: plugins/pd-bot/skills/pd-orchestrate/references/protocol.md
> One machine: a reviewer reads the worker's commits from the worker's checkout path. Sessions on another machine are outside v1.

## The only run path

> source: docs/adr/0005-pd-bot-runs-only-with-pod-bay-and-the-fleet-is-the-only-run-path.md
> pd-bot needs Pod Bay. `/pd-bot:pd-orchestrate` is the only way to run tickets with agents.

The steps of a run are in [workflow.md](workflow.md).

## What git tracks and what stays on one machine

This decides what another machine can read after `git clone`.

> source: .gitignore
> # Project docs and research notes are kept local only, except the decision records in docs/adr/ and the project wiki in docs/wiki/

| Tracked (every machine has it) | Local only (gitignored) |
|---|---|
| `docs/adr/`, `docs/wiki/` | every other folder under `docs/`, such as `docs/solutions/` (lessons) |
| `CLAUDE.md`, `AGENTS.md`, `REVIEW.md`, the plugin, the apps | `specs/` (the scratch files of each ticket, the fleet ledger and evidence) |
| `apps/pod-shape/docs/` (design doc, design system, hand-off) | `.claude/` except `launch.json` and `.claude/agents/*.md` |

A tracked file that names a local-only path points at nothing on a fresh clone. The known cases are
in [known-gaps.md](known-gaps.md).

## Tests and CI

> source: CLAUDE.md
> `bash scripts/run-tests.sh` runs the whole suite. CI (`pd-bot-ci`) runs the same suite on ubuntu and macOS.

> source: AGENTS.md
> CI runs on ubuntu and macOS, so avoid bash 4 features and GNU-only flags.

Shape Board has its own commands (`npm run typecheck`, `cargo test`); see `apps/pod-shape/CLAUDE.md`.
