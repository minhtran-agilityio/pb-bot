---
type: architecture
title: System Architecture Overview
description: How the pd-bot repository is organised into the pd-bot Claude Code plugin, the Pod Board menu bar app and the Shape Board desktop app, and how tickets flow from intake to a pull request through Pod Bay sessions.
tags: [architecture, overview, plugin, pod-bay, fleet]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-26T15:02:34.678Z
sources:
  - id: openwiki-source-09160a15c85836458c6d2077
    resource: repo://.claude-plugin/marketplace.json
  - id: openwiki-source-307f36ba30733005776bf390
    resource: repo://apps/pod-board/README.md
  - id: openwiki-source-06a6576e5c93092857b95860
    resource: repo://apps/pod-board/src-tauri/src/main.rs
  - id: openwiki-source-ae6fa55671607272e4809389
    resource: repo://docs/adr/0005-pd-bot-runs-only-with-pod-bay-and-the-fleet-is-the-only-run-path.md
  - id: openwiki-source-bd5e82856d328b9dfedb4398
    resource: repo://plugins/pd-bot/hooks/hooks.json
  - id: openwiki-source-139b698785417c0b4c884c72
    resource: repo://scripts/run-tests.sh
generated: { by: "claude-code", at: "2026-11-26T15:02:34.678Z" }
---

# System Architecture Overview

This repository ships three deliverables that share one purpose: take a ticket from a written request to a reviewed pull request that a human merges.

| Deliverable | Location | What it is |
|---|---|---|
| **pd-bot plugin** | `plugins/pd-bot/` | A Claude Code plugin: prompt-based skills, role agents, hooks, a behavior rule and helper scripts. The only supported runtime is Claude Code. |
| **Pod Board** | `apps/pod-board/` | A Tauri 2 menu bar app that observes every Pod Bay session by state. It never changes anything. |
| **Shape Board** | `apps/pod-shape/` | A Tauri 2 + React desktop app that grills a prototype and turns it into GitHub milestones and issues. |

The marketplace file `.claude-plugin/marketplace.json` publishes exactly one plugin, `pd-bot`, whose source is `./plugins/pd-bot`. Everything else (`scripts/`, `tests/`, `docs/`, `apps/`) supports the plugin's development, checks and operation.

## Pod Bay is a hard dependency

pd-bot runs only together with Pod Bay (`podbay-dev`). ADR 0005 removed the older `ship.js` workflow script, the `pd-autopilot` skill, the run board and git-worktree isolation; `/pd-bot:pd-orchestrate` is now the only way to run tickets with agents. Isolation comes from Pod Bay itself: each session works in its own workspace, which is a clean clone, so a session does not need a git worktree. Only the temporary detached worktrees that the reviewer and `fleet-check.sh` create to inspect a commit, plus the `clean-worktrees.sh` hook, remain.

Consequences stated in the ADR: pd-bot does not run without `podbay-dev`; `pd-implement` steps run one at a time on the working branch; and the fleet gates depend on `fleet-check.sh`, which blocks only when the base branch is `feat/*`.

<!-- openwiki: broken internal link [/openwiki/pod-board/overview.md] link "/openwiki/pod-board/overview.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
Pod Board is the human-facing counterpart: it reads the same Pod Bay listing (plus local Claude session files) to show which sessions need attention. See [Pod Board Menu Bar App](/openwiki/pod-board/overview.md).

## Plugin component layout

The plugin directory has six components:

- `skills/` — the prompt programs run as `/pd-bot:<name>` (intake, shape, xia2, writing-plans, implement, orchestrate, work, review, advise, compound, release notes, and others). `skills/README.md` is the map of skills and handoffs.
- `agents/` — nine subagent role definitions (advisor, implementer, intent-reviewer, qa-verifier, qa-writer, researcher, security-reviewer, task-reviewer, test-runner), each fixing a model, effort and tool set in its frontmatter.
- `hooks/` — shell hooks wired through `hooks/hooks.json`.
- `rules/behavior.md` — the single source of behavioral rules.
- `scripts/` — fleet checks, ledger backup, settings writer, ADR writer, solution index builder, scaffolder.
- `templates/` — scaffolding for `specs/` and the project structure.

<!-- openwiki: broken internal link [/openwiki/hooks/hook-system.md] link "/openwiki/hooks/hook-system.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
<!-- openwiki: broken internal link [/openwiki/hooks/safety-guards-merge-secrets-evidence.md] link "/openwiki/hooks/safety-guards-merge-secrets-evidence.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
A Claude Code plugin has no "rules" component, so the `session-rules.sh` SessionStart hook injects `rules/behavior.md` into each session as context. The hook wiring registers SessionStart (`session-rules.sh`, `session-role.sh`, and `clean-worktrees.sh` on `startup`), Stop (`stop-evidence.sh`), PreToolUse on Bash (`secrets-scan.sh`, `merge-guard.sh`) and on MCP merge tools (`merge-guard.sh`), and PostToolUse on Bash (`pr-opened.sh`, `clean-worktrees.sh`). Details are in [Plugin Hook System](/openwiki/hooks/hook-system.md) and [Safety Guards](/openwiki/hooks/safety-guards-merge-secrets-evidence.md).

## How a ticket moves through the system

<!-- openwiki: broken internal link [/openwiki/workflows/ticket-to-pr-skill-pipeline.md] link "/openwiki/workflows/ticket-to-pr-skill-pipeline.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
The logical pipeline is: intake (readiness gate and depth S, M or L) → shape → xia2 research (depth L) → writing-plans → qa-writer blind acceptance tests (depth M and L) → implement (one implementer subagent per step) → security review for risky tickets → code review and intent review → acceptance check (UAT for apps, `qa-verifier` for no-app tickets) → finishing the branch (push and open a PR). A human merges. See [Ticket-to-PR Skill Pipeline](/openwiki/workflows/ticket-to-pr-skill-pipeline.md).

<!-- openwiki: broken internal link [/openwiki/workflows/fleet-orchestration.md] link "/openwiki/workflows/fleet-orchestration.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
<!-- openwiki: broken internal link [/openwiki/operations/fleet-checks-gate-and-settings.md] link "/openwiki/operations/fleet-checks-gate-and-settings.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
At run time, the **fleet** carries this out with free agent sessions: an Opus orchestrator session in the main Pod Bay workspace assigns one worker session per ticket (`pd-work`), a different session reviews (`pd-review`), and an advisor session can be consulted when the orchestrator is stuck (`pd-advise`). The order of steps is free; the end state must show the evidence defined in `skills/pd-orchestrate/references/protocol.md`, which `fleet-check.sh` and the reviewer check. See [Fleet Orchestration](/openwiki/workflows/fleet-orchestration.md) and [Fleet Check, Gate and Settings Scripts](/openwiki/operations/fleet-checks-gate-and-settings.md).

## Cross-cutting design choices

- **Fail-open hooks.** Hooks generally fail open; `stop-evidence.sh` fails closed only on its risk floor, and `merge-guard.sh` asks the owner when unsure.
- **Humans merge.** No agent merges into main or the base branch unless the owner approves that one pull request.
<!-- openwiki: broken internal link [/openwiki/workflows/knowledge-store-adr-and-solutions.md] link "/openwiki/workflows/knowledge-store-adr-and-solutions.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
- **One store per record type.** Decisions live in `docs/adr/`, lessons in `docs/solutions/`, and per-ticket scratch in the gitignored `specs/`. See [Knowledge Store](/openwiki/workflows/knowledge-store-adr-and-solutions.md).
<!-- openwiki: broken internal link [/openwiki/workflows/agents-and-model-selection.md] link "/openwiki/workflows/agents-and-model-selection.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
- **Model choice is task-driven.** Defaults and hard limits (for example, depth S and M workers run Sonnet 5.5) are in [Subagents and Model/Effort Selection](/openwiki/workflows/agents-and-model-selection.md).
<!-- openwiki: broken internal link [/openwiki/testing/test-suite-and-ci.md] link "/openwiki/testing/test-suite-and-ci.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
- **Verification is mechanical.** `scripts/run-tests.sh` runs lints, hook tests, script tests and pytest, and CI runs the same suite on ubuntu and macOS. See [Test Suite, Lints and CI](/openwiki/testing/test-suite-and-ci.md).

## The two apps

<!-- openwiki: broken internal link [/openwiki/pod-shape/overview.md] link "/openwiki/pod-shape/overview.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
<!-- openwiki: broken internal link [/openwiki/pod-shape/design-system-rules.md] link "/openwiki/pod-shape/design-system-rules.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
Pod Board and Shape Board are independent Tauri applications with no runtime link to the plugin, but they share a design language: Pod Board's `ui/theme.css` copies Shape Board's `tokens.css`, and a Rust test fails if a shared token differs. See [Shape Board App](/openwiki/pod-shape/overview.md) and [Shape Board Design System](/openwiki/pod-shape/design-system-rules.md).
