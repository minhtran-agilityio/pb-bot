---
type: system
title: Plugin Hook System
description: How the pd-bot plugin wires shell hooks into Claude Code sessions, and how the rules, orchestrator-role, PR-review reminder and worktree-cleanup hooks decide what to do and when to stay silent.
tags: [hooks, session-role, orchestrator, worktrees, fail-open]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-26T15:02:34.678Z
sources:
  - id: openwiki-source-5fb7bd7579a05c512448a8e2
    resource: repo://plugins/pd-bot/hooks/clean-worktrees.sh
  - id: openwiki-source-c21e85df6280c5cdfc173d74
    resource: repo://plugins/pd-bot/hooks/pr-opened.sh
  - id: openwiki-source-144b726599d744cd3c2abb43
    resource: repo://plugins/pd-bot/hooks/session-role.sh
  - id: openwiki-source-da38c15d554ab991368848ba
    resource: repo://plugins/pd-bot/hooks/session-rules.sh
generated: { by: "claude-code", at: "2026-11-26T15:02:34.678Z" }
---

# Plugin Hook System

<!-- openwiki: broken internal link [/openwiki/hooks/safety-guards-merge-secrets-evidence.md] link "/openwiki/hooks/safety-guards-merge-secrets-evidence.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
The plugin registers its hooks in `plugins/pd-bot/hooks/hooks.json`; every entry runs `bash` on a script under `${CLAUDE_PLUGIN_ROOT}/hooks/`. The three guard hooks that can block an action (`secrets-scan.sh`, `merge-guard.sh`, `stop-evidence.sh`) are covered in [Safety Guards](/openwiki/hooks/safety-guards-merge-secrets-evidence.md). This page covers the four hooks that shape context and housekeeping.

| Event | Hook | Purpose |
|---|---|---|
| SessionStart | `session-rules.sh` | Deliver `rules/behavior.md` as session context |
| SessionStart | `session-role.sh` | Give the orchestrator role text to one session only |
| SessionStart (`startup` matcher) | `clean-worktrees.sh SessionStart` | Sweep merged branches and worktrees |
| PostToolUse (Bash) | `pr-opened.sh` | Remind the session to review a freshly opened PR |
| PostToolUse (Bash) | `clean-worktrees.sh PostToolUse` | Clean the branch of a PR that was just merged |

Timeouts are set per hook in `hooks.json` (10 s for `session-role.sh`, 15 s for `pr-opened.sh`, 30 s for the startup sweep). The common design rule is **fail open**: on any doubt or error a hook prints nothing and exits 0, so a broken hook cannot stop a session.

## Rules delivery (`session-rules.sh`)

A Claude Code plugin has no "rules" component. `session-rules.sh` therefore reads `rules/behavior.md` (the single source of truth for agent behavior) and prints a SessionStart JSON envelope whose `additionalContext` is that text. It escapes the text with `sed` and `awk`, so it needs no `jq`, and it prints nothing when the file is missing.

## Orchestrator role (`session-role.sh`)

Only the Pod Bay session of the main workspace is the orchestrator; workers, reviewers and advisors take their role from their brief. `session-role.sh` decides this with an ordered series of checks, each of which exits silently on failure:

1. The session must be interactive (`CLAUDE_CODE_ENTRYPOINT=cli`), and `STOP_EVIDENCE_GUARD` must be unset. A nested `claude -p` (the stop-evidence judge is one) inherits `TMUX_PANE` and `PD_BOT_ROLE` and must not receive the role.
2. If `PD_BOT_ROLE` is set, it decides alone: the value `orchestrator` gives the role, any other value gives none. It wins over every other signal, so it is meant for one session, not a shell profile. `fleet-settings.py` sets it for fleet sessions.
3. Otherwise the session must run inside tmux, and Pod Bay's tmux options must match: `@tn_kind` is `claude`, `@claude_dir` equals this project's directory (compared by physical path), the pane started as `claude ...`, and `@tn_context` is `<project>/<repo>` with no `@NN` suffix (a suffix marks a parallel `src-NN` workspace). Pod Bay does not document these options, so missing options mean silence.
4. `<project>/.claude/pd-bot.json` must not switch the role off with `{"orchestrator": "off"}`. The check tries `python3 -I`, then `jq`, then `grep`; an unreadable or non-object file also switches the role off.

When all checks pass, the hook substitutes the context name into `hooks/orchestrator-role.md` and emits it as `additionalContext`. The context name is only inserted when it contains plain characters, because it goes into a `sed` expression and a JSON string. With the `--role` argument the script prints just `orchestrator` or nothing; `pr-opened.sh` uses that form. The design rationale is in ADRs 0001 and 0002 (the role is text only, with no tool guard). Behavior is pinned by `tests/hooks/session-role.test.sh`.

## PR-review reminder (`pr-opened.sh`)

After a Bash command, `pr-opened.sh` checks that the command really was `gh pr create`: it splits the command at `&&`, `||`, `;`, `|` and line ends and accepts a part only when it starts with `gh pr create` (after optional variable assignments, `command`, or a path to `gh`), so a `grep` or review comment that merely mentions the phrase does not count. It then requires a `github.com/.../pull/<n>` URL in the tool response, because a failed create opens nothing and `gh pr edit` also prints a URL but is not matched by the command test.

The reminder text depends on who ran it: a subagent is told to return the PR number to the main session; the orchestrator (per `session-role.sh --role`) is told not to run `/code-review` itself and to hand the review and intent-review to another Pod Bay session; any other session is told to run `/code-review` then intent-review before merge. It reminds only and never blocks. A known gap: a script that runs `gh pr create` internally produces no reminder.

## Worktree cleanup (`clean-worktrees.sh`)

The script removes the worktree and local branch of merged branches. It runs in three ways:

- **PostToolUse**: acts only on a real `gh pr merge <n>` (or `gh api .../pulls/<n>/merge`) command, after stripping heredoc bodies and quoted strings so text that only mentions the phrase does nothing. It looks each PR up with `gh pr view`, and cleans only the heads of PRs whose state is `MERGED`.
- **SessionStart**: sweeps every local branch, with a time budget (`CLEAN_BUDGET`, default 20 s, under the 30 s hook timeout) checked before each branch so none is left half-cleaned. It also keeps any worktree that a live process uses as its working directory (`/proc` on Linux, `lsof` elsewhere).
- **By hand**: with no argument it sweeps and prints plain text. `CLEAN_DRY_RUN=1` reports what it would do without writing.

A branch counts as merged by one of three rules: (1) with `gh`, a merged PR whose head commit equals the local tip, removed with `git branch -D` — the exact-tip match is the guard, because a squash merge is not in git history; (2) without `gh`, the upstream is gone and the tip is an ancestor of `<remote>/main`; (3) a `worktree-*` agent branch with no checked-out worktree whose tip is contained in a remote branch (only after a successful fetch). Rules (2) and (3) use `git branch -d` only. `git worktree remove` is run without force, so a dirty or locked worktree is kept and reported.

Protected names are never touched: `main`, `master`, `develop`, `development`, `staging`, `production`, `release/*`, `release-*`, `CLEAN_KEEP` names, the current branch and the worktree the session runs in. Settings come from environment variables (`CLEAN_REMOTE`, `CLEAN_KEEP`, `CLEAN_STRICT`, `CLEAN_RECREATE`). `CLEAN_RECREATE` can push a missing base branch from `<remote>/main`, never forced and only when `ls-remote` says it is absent. Tests live in `tests/hooks/post-merge-hooks.test.sh`.

## Related

<!-- openwiki: broken internal link [/openwiki/testing/test-suite-and-ci.md] link "/openwiki/testing/test-suite-and-ci.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
<!-- openwiki: broken internal link [/openwiki/workflows/fleet-orchestration.md] link "/openwiki/workflows/fleet-orchestration.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
See [Test Suite, Lints and CI](/openwiki/testing/test-suite-and-ci.md) for how hook tests run, and [Fleet Orchestration](/openwiki/workflows/fleet-orchestration.md) for the orchestrator role in practice.
