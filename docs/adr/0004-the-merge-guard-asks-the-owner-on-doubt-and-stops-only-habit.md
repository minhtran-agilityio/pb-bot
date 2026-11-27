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

Doubt means ask. The hook (`permissionDecision: "ask"`) puts every merge-like command to the owner, who is the parser, unless it is a plain `gh pr merge <n> [flags]` or `glab mr merge <n> [flags]` whose base, read live, is `feat/*`. Only `feat/*` passes without a question; every other base, and a base that cannot be read in 15 s, asks. `--auto` and `--admin` are denied. A `git push` that reaches main or master asks: one that names it (quotes ignored), `--all`, `--branches`, `--mirror`, a refspec `:` or one with `*`, and one that names no branch (`git push`, `git push origin`, `git push origin HEAD`), where the hook reads the current branch, its upstream, `push.default` and `remote.<name>.push` from the repository of the command (payload `cwd`, or `git -C`), and a `git checkout main` earlier in the same command counts (ticket #190); and an MCP tool that merges (hooks.json matcher `mcp__.*__(merge_.*|enable_.*auto_merge.*)`) always asks. When the payload has `agent_id` (a subagent or workflow agent) nobody can answer, so an ask becomes a denial. `--match-head-commit` is no longer required; it is accepted as an ordinary flag. This supersedes, in the old repository's ADRs 0008, 0009, 0011 and 0012, the fail-closed rule, the protected-base list, the `--match-head-commit` requirement and the denial by permission mode; the rule that a human approves every merge into main stays.

What an ask does where nobody can answer. Source: Claude Code hooks reference https://code.claude.com/docs/en/hooks.md, field permissionDecisionReason: "For `ask`, shown to the user in the permission prompt. When Claude Code denies the call in a `-p` run where no one can answer that prompt, Claude reads the reason in the tool result instead." Headless docs https://code.claude.com/docs/en/headless.md: "In a `-p` run with no host, these requests are denied either way." Permission modes https://code.claude.com/docs/en/permission-modes.md: dontAsk "auto-denies every tool call that would otherwise prompt you". Hooks reference https://code.claude.com/docs/en/hooks.md, auto mode: "a PreToolUse hook's `ask` normally sends the tool call to the permission prompt, and the classifier doesn't decide it." The docs name `agent_id` for a subagent or in-process teammate; they do not say a Workflow agent's payload carries it. If it does not, that agent's ask goes to the owner's prompt instead of a denial, which still merges nothing unapproved. The docs do not state the bypassPermissions case or the subagent case for a hook ask: for bypassPermissions the earlier probe (default, auto, bypassPermissions: the call did not run headless) is kept as the test `RUN_ASK_PROBE=1 bash tests/hooks/merge-guard.test.sh`, now also for dontAsk; for a subagent the hook itself denies. **The ask probe is manual-only** (owner decision, ticket #190): it starts real `claude -p` sessions, needs a login and costs tokens, so CI never runs it and no CI job is planned. Its four cases skip unless `RUN_ASK_PROBE=1` is set. It is the only test of this paragraph's claim that `ask` is not turned into an allow, so run it by hand from the repo root (`RUN_ASK_PROBE=1 bash tests/hooks/merge-guard.test.sh`) after a Claude Code upgrade and whenever the hook's ask changes.

## Why

The owner merges alone and the real risk is an agent that merges by habit, which a short pattern catches. A question costs the owner one click, while a false denial stops a run and a parser that tries to read bash keeps breaking. A hidden command was never stopped by the old hook either, so the narrower goal loses no real protection.

## Consequences

The hook is about 140 lines. It reads bash only as far as a `git push`, `git checkout`, `git switch`, `cd` or `pushd` segment (split at `; & | ( )` and backticks, quotes dropped, words split on spaces), and never reads a merge command beyond the patterns above. False questions remain, by design: a gh or glab command whose text holds the word merge (`git commit -m "... gh pr merge ..."`, `gh issue comment --body "... merge ..."`, a PR title with merge) asks, and in a subagent or ship.js workflow agent that ask is a denial. The workaround is to put the text in a file (`git commit -F file`, `--body-file`). A branch name counts too: `gh pr create --head feat/12-fix-merge-button` asks. So ship.js keeps such text out of its gh command lines: the final PR and each ticket PR are created with `gh api repos/<o>/<r>/pulls -X POST --input <json file>` (title, head, base and body in the file, and no command substitution, which could raise a permission prompt a workflow agent cannot answer), and the verify comment uses `--body-file`. Before this change, a run whose slug or ticket title held the word merge would have been denied when it opened the final or the ticket PR. An agent command outside those prompts (for example `gh api .../pulls/5 --jq .merge_commit_sha`, or `gh pr checkout 5 && git merge-base ...`) can still be denied in a workflow agent. ship.js merges the final PR on its own only into a `feat/*` base; every other base is a human merge, because a workflow agent's merge into it would be denied. `git push origin feat/main` asks (a false question, harmless in the main session); `feat/main-x` and `maintenance` do not. A push to a default branch with another name (for example develop) is not caught. A push that cannot be resolved (no repository at the `cwd`, a detached HEAD) passes, because git itself stops; a `cd <dir>` earlier in the command moves the repository that is read, but a directory held in a variable (`cd "$REPO"`, `git -C "$REPO"`) or with spaces is not resolved, so that push passes. Also not handled: `git -C a -C b push` (git uses `a/b`), `--git-dir=X`, an abbreviated option (`--mirr`), `git checkout -` and a line continuation inside a push. The static `permissions.deny` rules cover the spellings a pattern can name (`:main`, `+main`, quoted `"main"`, `--all`, `--mirror`); they cannot cover a push without a branch, which only this hook resolves. Without `--match-head-commit`, a commit pushed between the owner's read and the yes is merged too. The owner's own `!` or terminal commands are not seen.

## How to reverse

Restore the earlier `plugins/pd-bot/hooks/merge-guard.sh` and `tests/hooks/merge-guard.test.sh` from git history and remove the MCP matcher from `plugins/pd-bot/hooks/hooks.json`.
