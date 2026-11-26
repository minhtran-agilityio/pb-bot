---
name: pd-setup
description: "First-run setup for a project that installed the pd-bot plugin. Scaffolds specs and docs, proposes a Run and test section for CLAUDE.md (and migrates an old agents/PROJECT.md), merges the permissions.deny rules into project settings, names the merge guard, ignores specs/, and asks about the Pod Bay orchestrator role. Use when the plugin was just installed in a project."
disable-model-invocation: true
allowed-tools: Read, Write, Edit, Bash, AskUserQuestion
---

# Setup

Run once per project, after `claude plugin install pd-bot@pd-bot`. Work in the project root
(`${CLAUDE_PROJECT_DIR}`). Say what each step did. Every step is safe to run twice.

1. **Scaffold.** Run `bash ${CLAUDE_PLUGIN_ROOT}/scripts/init-structure.sh --root ${CLAUDE_PROJECT_DIR}`.
   It creates `specs/` and `docs/solutions/` README files, and never overwrites.
2. **Run and test.** If `${CLAUDE_PROJECT_DIR}/CLAUDE.md` has no `## Run and test` section, propose
   one (test, targeted test, lint, full suite), detected from the manifests, and show it. Write it
   only after the owner agrees (AskUserQuestion), with Edit (or Write if `CLAUDE.md` does not
   exist). If an old `agents/PROJECT.md` exists, propose moving its *Test execution* into that
   section, and ask before deleting the file (`rm -- ${CLAUDE_PROJECT_DIR}/agents/PROJECT.md`). Running it twice is safe:
   skip the write when the section exists, and skip the migration when `agents/PROJECT.md` is gone.
3. **Deny rules.** A plugin cannot ship `permissions.deny`, so merge it into the project settings.
   Run `bash ${CLAUDE_SKILL_DIR}/scripts/merge-deny.sh --dry-run ${CLAUDE_PROJECT_DIR}/.claude/settings.json`
   and show the diff. After the owner agrees, run it again without `--dry-run`. It needs `jq`
   (exit 3 if missing) and refuses a file that is not valid JSON or whose permissions, allow or deny has the wrong type (exit 2, file untouched). It adds
   these rules and keeps every existing one: `.env` read and edit, force-push, push to main or master.
4. **Merge guard (nothing to install).** The plugin's PreToolUse hook `merge-guard.sh` asks the owner about every merge whose base is not `feat/*`.
   Tell the owner that every PR into a protected base is a human merge (the owner can approve one named PR in the
   hook's confirmation prompt), and that ticket PRs into `feat/<slug>` stay human-merged too unless they merge them:
   setup writes no allow rule for `gh pr merge`, because no command-text rule can see the PR base. Plugin hooks are not
   proven to fire for workflow agents, so the guard is a second line, not a reason to allow the merge. To let a run
   merge ticket PRs, the owner adds an allow rule for `gh pr merge` into `feat/<slug>*` to their own settings; setup never writes it.
5. **Ignore specs.** Add `specs/` to `${CLAUDE_PROJECT_DIR}/.gitignore` if no line already says
   `specs/`. Skip this if the project tracks files under `specs/`.
6. **Orchestrator role (Pod Bay only).** Skip this step when neither `podbay-dev` nor `podbay` is on
   `PATH`. The plugin's `session-role.sh` hook tells the Pod Bay session of the main workspace (`src`)
   that it is the orchestrator: it checks, coordinates and delegates hands-on work to worker
   sessions. Sessions in `src-NN` get nothing. The role is on by default. Ask the owner
   (AskUserQuestion): keep it, or switch it off for this project. To switch it off, write
   `{"orchestrator": "off"}` to `${CLAUDE_PROJECT_DIR}/.claude/pd-bot.json` (merge the key when the
   file exists). To keep it, write nothing. `PD_BOT_ROLE=none` switches it off for one session.
7. **Token trim.** Run `bash ${CLAUDE_SKILL_DIR}/scripts/merge-trim.sh --dry-run ${CLAUDE_PLUGIN_ROOT}/scripts/fleet-trim.json ${CLAUDE_PROJECT_DIR}/.claude/settings.json`
   and show the diff; merge after the owner agrees. No context cap (`--compact-window N` adds one).

Never touch: `specs/`, `docs/`, and any skill, agent or hook the
project wrote itself. Setup never writes `.claude/settings.local.json`.

Finish with a short list of what changed, what was skipped, and what the owner still has to do.
