---
ticket: merge-guard-v2
stage: design
status: accepted
confidence: high
decided_by: owner
date: 2026-11-22
---

# 0004. The merge guard asks the owner on doubt and stops only habitual merges

## Context

Goal: an agent never merges into main or another base that is not feat/* on its own; the owner is asked each time and the merge runs only on a yes. Threat model: an agent that merges out of habit with a normal command, not an agent that hides the command (escapes, scripts, interpreters, curl with a token).

The earlier hook (`plugins/pd-bot/hooks/merge-guard.sh`, 407 lines, test file 501 lines) tried to catch hidden merges too and denied on every doubt. Its bash parser was the source of most fixes to it (12 of 108 non-merge commits on main) and of false blocks such as the denied pd-autopilot preflight (P9). Its own header admitted that interpreters and run-time built commands pass anyway. The earlier merge-guard ADRs of the old repository (0008, 0009, 0011, 0012, not in this repository; see ADR 0003) chose fail-closed denial, a protected-base list, `--match-head-commit` for a merge into a protected base, and denial in dontAsk or an unknown mode.

## Options

- Doubt means ask: a plain `gh pr merge <n>` or `glab mr merge <n>` into feat/* passes, every other merge-like command asks the owner, a subagent's ask is a denial (chosen)
- Keep the fail-closed parser and fix each false block as it appears
- Rely on GitHub settings only (branch protection, auto-merge off) and drop the hook

## Decision

Doubt means ask. The hook (`permissionDecision: "ask"`) puts every merge-like command to the owner, who is the parser, unless it is a plain `gh pr merge <n> [flags]` or `glab mr merge <n> [flags]` whose base, read live, is `feat/*`. Only `feat/*` passes without a question; every other base, and a base that cannot be read in 15 s, asks. `--auto` and `--admin` are denied. A `git push` that names main or master asks, and an MCP tool that merges (hooks.json matcher `mcp__.*__(merge_.*|enable_.*auto_merge.*)`) always asks. When the payload has `agent_id` (a subagent or workflow agent) nobody can answer, so an ask becomes a denial. `--match-head-commit` is no longer required; it is accepted as an ordinary flag. This supersedes, in the old repository's ADRs 0008, 0009, 0011 and 0012, the fail-closed rule, the protected-base list, the `--match-head-commit` requirement and the denial by permission mode; the rule that a human approves every merge into main stays.

What an ask does where nobody can answer. Source: Claude Code hooks reference https://code.claude.com/docs/en/hooks.md, field permissionDecisionReason: "For `ask`, shown to the user in the permission prompt. When Claude Code denies the call in a `-p` run where no one can answer that prompt, Claude reads the reason in the tool result instead." Headless docs https://code.claude.com/docs/en/headless.md: "In a `-p` run with no host, these requests are denied either way." Permission modes https://code.claude.com/docs/en/permission-modes.md: dontAsk "auto-denies every call that would otherwise prompt"; in auto mode "a PreToolUse hook's `ask` normally sends the tool call to the permission prompt, and the classifier doesn't decide it." The docs do not state the bypassPermissions case or the subagent case for a hook ask: for bypassPermissions the earlier probe (default, auto, bypassPermissions: the call did not run headless) is kept as the opt-in test `RUN_ASK_PROBE=1 bash tests/hooks/merge-guard.test.sh`, now also for dontAsk; for a subagent the hook itself denies.

## Why

The owner merges alone and the real risk is an agent that merges by habit, which a short pattern catches. A question costs the owner one click, while a false denial stops a run and a parser that tries to read bash keeps breaking. A hidden command was never stopped by the old hook either, so the narrower goal loses no real protection.

## Consequences

The hook is 63 lines and never parses bash. False questions remain, by design: a gh or glab command whose text holds the word merge (`git commit -m "... gh pr merge ..."`, `gh issue comment --body "... merge ..."`, a PR title with merge) asks, and in a subagent or ship.js workflow agent that ask is a denial. The workaround is to put the text in a file (`git commit -F file`, `--body-file`). In ship.js this can hit the intake comment (`gh issue comment --body`), the verify evidence comment (`gh pr comment --body`) and `gh pr create --title "<slug>"` only when that text or slug holds the whole word merge; the other PR body writes already use `--body-file`. `git push origin feat/main` asks (a false question, harmless in the main session); `feat/main-x` and `maintenance` do not. A push to a default branch with another name (for example develop) is not caught. Without `--match-head-commit`, a commit pushed between the owner's read and the yes is merged too. The owner's own `!` or terminal commands are not seen.

## How to reverse

Restore the earlier `plugins/pd-bot/hooks/merge-guard.sh` and `tests/hooks/merge-guard.test.sh` from git history and remove the MCP matcher from `plugins/pd-bot/hooks/hooks.json`.
