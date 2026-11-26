# Repository Guidelines

This repository ships `pd-bot`, a Claude Code plugin of skills, agents and hooks that carry a change
from a ticket to a reviewed pull request. It runs only with Pod Bay (`podbay-dev`). `CLAUDE.md` is the working
contract (commands, workflow, hooks, gotchas) and `plugins/pd-bot/skills/README.md` is the skill
and handoff map. Read both before you change a skill, an agent, a hook or a script.

## Layout

- `.claude-plugin/marketplace.json` - the marketplace (`pd-bot`) that lists the plugin.
- `plugins/pd-bot/.claude-plugin/plugin.json` - the plugin manifest (name `pd-bot`, version).
- `plugins/pd-bot/skills/<name>/SKILL.md` - the skills, invoked as `/pd-bot:<name>`, with their references and scripts.
- `plugins/pd-bot/agents/` - the subagent role files (`pd-bot:<role>`); model, effort and tools are in their frontmatter.
- `plugins/pd-bot/hooks/` - `hooks.json` plus the Bash hooks listed in the `CLAUDE.md` Hooks table.
- `plugins/pd-bot/rules/behavior.md` - the one behavioral rule, delivered at session start.
- `plugins/pd-bot/templates/` and `plugins/pd-bot/scripts/` - scaffold files and the runtime scripts the skills call.
- `scripts/` - CI, lint, validation and release scripts for this repository, with their `test_*.py` tests.
- `tests/` - `hooks/` and `scripts/` hold contract and integration tests, `fixtures/` their inputs, `evals/tickets/` the ticket evals.
- `apps/pod-board/` - the Pod Bay session board (a Tauri menu bar app).
- `assets/` - images that the READMEs show.
- `docs/` and `specs/` are local only and gitignored. Do not link to them from tracked files.

## Commands

- `bash scripts/run-tests.sh` - the suite CI runs on ubuntu and macOS. Run it before every PR.
- `bash scripts/check-shellcheck.sh` - shellcheck over the shell scripts (CI requires shellcheck).
- `bash scripts/check-plugin-validate.sh` and `bash scripts/check-plugin-version.sh` - validate the manifests and check that `plugin.json` matches `VERSION`.
- `bash scripts/lint-doc-truth.sh` and `bash scripts/check-md-links.sh` - check documented paths and links.
- `claude --plugin-dir plugins/pd-bot` - load the plugin from the checkout while you develop.

## Style and tests

Bash uses two-space indentation, quoted variables and `set -u`. Python uses four spaces and `ruff`.
CI runs on ubuntu and macOS, so avoid bash 4 features and GNU-only flags. Name skills and
directories in lowercase kebab-case; every skill folder starts with `pd-`. Add or update a focused
test with every behavior change; shell tests run in a throwaway git repo.

## Stop and ask the owner

Ask before a change that touches auth or permissions, data migration or deletion, a public contract
(a skill or agent name, a hook's behavior, the manifests), an external provider, `settings.json` or
`plugins/pd-bot/hooks/`, or that removes a test or a feature. Also ask when the direction is
ambiguous.

## Commits, pull requests and releases

Use Conventional Commit subjects (`fix(hooks): ...`, `docs: ...`) and keep commits focused. A pull
request states the workflow impact, the tests run and the linked issue. Never merge a pull request
without human review: an agent merges into the base branch only after the owner approves that one
PR in the confirmation prompt that `merge-guard.sh` raises (ADR 0004: when in doubt it asks the owner; only
a plain merge into `feat/*` runs without a question).

Only the release PR changes `VERSION`, `plugin.json` and the versioned sections of `CHANGELOG.md`.
Run the `release` workflow from `main` to open it; merging it makes `release-tag` create the tag and
the GitHub release. Other PRs add their notes under `## [Unreleased]`.

<!-- OPENWIKI:START -->

## OpenWiki

This repository has a generated `openwiki/` evidence index. It is optional just-in-time context, not required startup reading.

- Do not enumerate, preload, or search wikis at task start. Use retrieval when the user asks for it, when unfamiliar architecture or dependency behavior materially affects the task, or when source inspection leaves an important uncertainty. Stop once the question is grounded.
- When those conditions apply and OpenWiki retrieval tools are available, use `openwiki_search` for just-in-time context and `openwiki_read` for the relevant complete sections. If search returns `workspace_required`, ask which listed workspace to use and retry with its ID.
- Use `openwiki_list_workspaces` or `openwiki_list_wikis` when workspace membership itself needs to be discovered.
- If the retrieval tools are unavailable, read `openwiki/quickstart.md` and follow its links to the relevant pages.
- Treat source code and tests as authoritative. A brief's unknowns and review items are verification gaps, not automatic requirements.
- Prefer the narrowest quiet validation that proves the changed behavior. Preserve complete failure output.

Use the repository's documented maintenance process to refresh generated pages after source changes. Check its configuration and completed runs before relying on automatic updates. Do not hand-edit generated OpenWiki pages unless explicitly asked; prefer updating source code/docs and letting OpenWiki regenerate.

<!-- OPENWIKI:END -->
