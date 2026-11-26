---
type: system
title: "Safety Guards: merge-guard, secrets-scan, stop-evidence"
description: Decision logic, threat model and limits of the three hooks that can stop an agent action (merge-guard.sh, secrets-scan.sh, stop-evidence.sh), plus the permissions.deny rules that back them.
tags: [hooks, merge-guard, secrets, stop-evidence, security, permissions]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-26T15:02:34.678Z
sources:
  - id: openwiki-source-1f0fcec6f758c1e37d737379
    resource: repo://docs/adr/0004-the-merge-guard-asks-the-owner-on-doubt-and-stops-only-habit.md
  - id: openwiki-source-8082e2d1c881cda5106e3e36
    resource: repo://plugins/pd-bot/hooks/merge-guard.sh
  - id: openwiki-source-a85aac2f7c9681e7997f08a6
    resource: repo://plugins/pd-bot/hooks/secrets-scan.sh
  - id: openwiki-source-4321312f564f7c6853a4d291
    resource: repo://plugins/pd-bot/hooks/stop-evidence-judge.md
  - id: openwiki-source-23b2038546fe1b4f1d2ddf1a
    resource: repo://plugins/pd-bot/hooks/stop-evidence.sh
  - id: openwiki-source-8d67ed5c5b1bd7083b7dcfc9
    resource: repo://settings.json
generated: { by: "claude-code", at: "2026-11-26T15:02:34.678Z" }
---

# Safety Guards

<!-- openwiki: broken internal link [/openwiki/hooks/hook-system.md] link "/openwiki/hooks/hook-system.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
Three hooks can block or question an agent action. They back the hard rules in `rules/behavior.md`: never merge into main or the base branch without the owner's approval, never touch secrets, and "Done needs real command output". Wiring is in [Plugin Hook System](/openwiki/hooks/hook-system.md). Each guard is small and intentionally narrow, and each states what it does *not* stop.

## merge-guard.sh: doubt means ask (ADR 0004)

**Goal.** An agent never merges into `main` (or any base that is not `feat/*`) on its own; the owner is asked each time and the merge runs only on a yes. **Threat model.** An agent that merges out of habit with a normal command, not one that hides the command in escapes, scripts, interpreters or `curl` with a token. ADR 0004 replaced an earlier 407-line fail-closed parser, whose bash parsing caused many fixes and false blocks, with a short pattern that never parses bash.

`hooks.json` sends two kinds of calls to the script: every Bash command, and MCP tools whose name matches `mcp__.*__(merge_.*|enable_.*auto_merge.*)`. The script then walks these steps in order:

1. **MCP merge tool** → always `ask` (the base is not read).
2. **`git push` naming `main` or `master`** (`origin main`, `HEAD:main`, `refs/heads/main`) → `ask`.
3. **Not a merge** → exit 0. `merge` must be a whole word after `gh` or `glab`, or a `pulls/<n>/merge` or `mergepullrequest` form, so `mergeable` and `merged` do not count.
4. **`--auto` or `--admin`** → `deny`.
5. **A plain `gh pr merge <n> [flags]` or `glab mr merge <n> [flags]`** → the script reads the PR's base live (at most 15 s). A base starting with `feat/` passes; any other base, or one that cannot be read, asks the owner.
6. **Any other merge-looking form** (chains, pipes, quotes, `gh api`, GraphQL, the word in a text) → `ask`.

`ask` emits a `permissionDecision: "ask"` JSON so the owner sees a confirmation prompt. When the payload carries an `agent_id` (a subagent), nobody can answer, so the ask becomes a block (exit 2). If `jq` is missing the guard blocks any payload mentioning `merge` or `push`, because it cannot judge. 

**Known limits** (by design, from the ADR): text that merely contains the word merge in a `gh`/`glab` command asks falsely, so put such text in a file (`git commit -F`, `--body-file`); a push to a default branch with another name (for example `develop`) is not caught; without `--match-head-commit` a commit pushed between the owner's read and the yes is merged too; and the owner's own `!` or terminal commands are not seen.

## secrets-scan.sh: staged secrets

On a `git commit` command only, the hook inspects the staged changes and blocks (exit 2) in two cases:

- the staged diff (excluding `tests/`, `__tests__/`, `*.example`, `*.md` and `docs/`) matches a secret pattern such as `sk-…` keys, `AKIA…` AWS ids, or `password=`, `api_key=`, `client_secret=` and `secret=` assignments with quoted values;
- a staged filename ends in `.env`.

It fails open when `jq` or `git` is missing or the payload is bad, and it passes any command that is not a commit. The block message tells the agent to move the value to an environment variable, or to ask the owner when the value is a fake test value.

## stop-evidence.sh: "Done needs real command output"

This Stop hook blocks the end of a main session when **code changed** (non-doc files in `git status`, excluding `docs/`, `specs/`, `*.md` and `*.txt`) and the transcript holds **no Bash tool result**. It reads the transcript defensively, because the format is not documented. The decision flow:

1. Exit at once if `STOP_EVIDENCE_GUARD` is set (the judge itself runs `claude -p`, which would load this hook again), if `python3` is missing, or if no code file changed or a Bash result exists.
2. Compute the **risk floor**: a changed path under `hooks/` or `scripts/`, or auth/secret words (password, secret, api key, credential, jwt, oauth, auth*, login, sign in) in the changed files or added lines.
3. If the transcript is unreadable and the floor did not match, pass.
4. If the main model is Opus and the floor did not match, skip the judge: block once with a light reminder (a second stop, with `stop_hook_active`, passes).
5. Otherwise run a **judge**: `claude -p --model opus` with `stop-evidence-judge.md` plus session facts, a diff summary and the last assistant message, limited by `STOP_EVIDENCE_JUDGE_TIMEOUT` (90 s default). The judge must answer one JSON line with severity `pass`, `light` or `severe`. `pass` exits; `light` blocks once; `severe` always blocks.
6. If the judge fails or prints junk, the hook **fails closed only on the risk floor** and otherwise passes.

The judge prompt defines `severe` as costly or hard-to-undo changes (login, permissions, payments, secrets, data deletion, hooks or scripts that run every session, merge or release logic, many files) and also a final message claiming "done" or "tests pass" with no command run. A claim in the message is never evidence.

## permissions.deny

A plugin cannot ship `permissions.deny`, so the repository's root `settings.json` carries it: reads and edits of `.env` and `.env.*`, `git push --force`/`-f`, and `git push` to `main` or `master`. `/pd-bot:pd-setup` merges these rules into a consuming project. The separate `.claude/settings.json` only disables some plugins and denies claude.ai connectors to trim tokens.

## Tests

<!-- openwiki: broken internal link [/openwiki/testing/test-suite-and-ci.md] link "/openwiki/testing/test-suite-and-ci.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
The guards have contract tests in `tests/hooks/` (`merge-guard.test.sh`, `secrets-scan.test.sh`, `stop-evidence.test.sh`) and allow/deny matrices in `tests/scripts/merge-allow.test.sh` and `tests/scripts/merge-deny.test.sh`. See [Test Suite, Lints and CI](/openwiki/testing/test-suite-and-ci.md).
