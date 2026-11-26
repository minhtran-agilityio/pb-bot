---
title: Hooks and guards
read_when: a hook blocked you, or you change a hook or settings.json
sources:
  - CLAUDE.md
  - settings.json
  - plugins/pd-bot/hooks/hooks.json
  - plugins/pd-bot/hooks/session-rules.sh
  - plugins/pd-bot/hooks/session-role.sh
  - plugins/pd-bot/hooks/merge-guard.sh
  - plugins/pd-bot/hooks/secrets-scan.sh
  - plugins/pd-bot/hooks/stop-evidence.sh
  - plugins/pd-bot/hooks/stop-evidence-judge.md
  - plugins/pd-bot/hooks/pr-opened.sh
  - plugins/pd-bot/hooks/clean-worktrees.sh
checked: 2026-11-24
---

# Hooks and guards

The plugin registers seven hook scripts in `plugins/pd-bot/hooks/hooks.json`. The repository's own
`settings.json` wires no hook; it holds only the deny rules.

> source: CLAUDE.md
> All hooks fail open except `stop-evidence.sh` on its risk floor; `merge-guard.sh` asks the owner when in doubt (ADR 0004).

"Fail open" means: when the hook has an error or a doubt, it lets the action run.

| Hook | Event | What it does | When it blocks |
|---|---|---|---|
| `session-rules.sh` | SessionStart | Puts `rules/behavior.md` into the session context | never |
| `session-role.sh` | SessionStart | Gives the orchestrator role text to one session | never |
| `clean-worktrees.sh` | SessionStart (`startup`), PostToolUse (Bash) | Removes the worktree and branch of merged branches | never |
| `secrets-scan.sh` | PreToolUse (Bash) | Scans the staged diff on `git commit` | a secret pattern or a staged `.env` file |
| `merge-guard.sh` | PreToolUse (Bash, MCP merge tools) | Asks the owner about a merge | `--auto`, `--admin`; any ask inside a subagent |
| `pr-opened.sh` | PostToolUse (Bash) | After `gh pr create`, reminds the session to get a review | never |
| `stop-evidence.sh` | Stop | Checks that a session that changed code ran a command | code changed and no command output |

## session-rules.sh: how the one rule file reaches a session

> source: plugins/pd-bot/hooks/session-rules.sh
> A plugin has no "rules" component, so the plugin's one rule file reaches the model here.

It prints the file as `additionalContext`. It costs about 3.9 KB in every session that has the plugin.

## session-role.sh: who is the orchestrator

> source: plugins/pd-bot/hooks/session-role.sh
> Every other session gets nothing: a worker, a reviewer and an advisor take their role from

(The line ends with "their brief".)

The order of the checks: the session is interactive; `PD_BOT_ROLE` (it wins); the tmux options that
Pod Bay sets; `@tn_context` has no `@NN` suffix; `.claude/pd-bot.json` does not say
`{"orchestrator": "off"}`.

> source: plugins/pd-bot/hooks/session-role.sh
> PD_BOT_ROLE: "orchestrator" gives the role, any other value gives none.

## merge-guard.sh

- A plain `gh pr merge <n>` or `glab mr merge <n>` whose base is `feat/*` runs with no question.
- Every other merge-like command, a `git push` that names main or master, and an MCP merge tool: the
  owner sees a confirmation prompt.
- In a subagent nobody can answer, so the ask is a denial.
- A base that the hook cannot read in 15 seconds asks.

> source: plugins/pd-bot/hooks/merge-guard.sh
> In a subagent nobody can answer, so ask = deny.

Known false question: a `gh` or `glab` command whose text holds the word merge. Put that text in a
file (`git commit -F`, `--body-file`).

> source: plugins/pd-bot/hooks/merge-guard.sh
> text in a file (`git commit -F`, `--body-file`).

If `jq` is missing, a command with the word merge or push is blocked.

## secrets-scan.sh

> source: plugins/pd-bot/hooks/secrets-scan.sh
> Fails open: no jq/git, a bad payload, or any other command passes. Exit 2 blocks.

The diff scan leaves out tests, `*.example`, `*.md` and `docs/`.

## stop-evidence.sh

It runs when the main session stops. When code changed and the transcript holds no Bash output, it
blocks. A judge model (Opus) picks `pass`, `light` (one reminder) or `severe`. An Opus main session
skips the judge unless the risk floor matches.

> source: plugins/pd-bot/hooks/stop-evidence.sh
> Risk floor: hooks/ and scripts/ paths, or auth/secret words in added lines

> source: plugins/pd-bot/hooks/stop-evidence-judge.md
> A claim in the last message is not evidence. Only a command's output is evidence.

## The deny rules in settings.json

> source: settings.json
> "Read(./.env)",

The file denies: read and edit of `.env` and `.env.*`, `git push --force`, `git push -f`, and a push
to `main` or `master`.

## Before you change a hook

Run `bash scripts/run-tests.sh` and `bash scripts/check-shellcheck.sh`. The hook tests are in
`tests/hooks/`. `scripts/lint-doc-truth.sh` fails when the hook table in `CLAUDE.md` and
`hooks.json` differ. A hook change needs the owner's yes first ([hard-rules.md](hard-rules.md), R10).
