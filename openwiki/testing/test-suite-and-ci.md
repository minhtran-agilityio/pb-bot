---
type: testing
title: Test Suite, Lints and CI
description: How scripts/run-tests.sh layers lints, hook contract tests, script tests and pytest; the hermetic helpers in tests/lib.sh; the strict-mode conventions for shellcheck and the Claude CLI; the pd-tickets evals; and the GitHub workflows that run it all.
tags: [testing, ci, shellcheck, lint, github-actions, hermetic]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-26T15:02:34.678Z
sources:
  - id: openwiki-source-6d4b4e707b8d60b6ccfa3425
    resource: repo://.github/workflows/openwiki-update.yml
  - id: openwiki-source-cc9c7a308ede00308548d268
    resource: repo://.github/workflows/pd-bot-ci.yml
  - id: openwiki-source-ff18dfbdea4b73dc8a89523e
    resource: repo://.github/workflows/pod-shape-ci.yml
  - id: openwiki-source-4fdc049df9c4d1981f4d02a5
    resource: repo://scripts/check_allowed_tools_wide.py
  - id: openwiki-source-dbd57945d6b656adf292554c
    resource: repo://scripts/check_frontmatter_contract.py
  - id: openwiki-source-c1116bf9eaf7841fd5afafc3
    resource: repo://scripts/check_ste_baseline.py
  - id: openwiki-source-f6029e8e0abe4e747763a2c8
    resource: repo://scripts/check-plugin-validate.sh
  - id: openwiki-source-6ad087599169f8f4e23cba81
    resource: repo://scripts/check-shellcheck.sh
  - id: openwiki-source-e76d83a45d52c4eac45c3166
    resource: repo://scripts/lint-doc-truth.sh
  - id: openwiki-source-139b698785417c0b4c884c72
    resource: repo://scripts/run-tests.sh
  - id: openwiki-source-a3746245b64ea1db56e95e81
    resource: repo://tests/evals/tickets/README.md
  - id: openwiki-source-1c0a4aa5b34378c4e82b65d7
    resource: repo://tests/lib.sh
generated: { by: "claude-code", at: "2026-11-26T15:02:34.678Z" }
---

# Test Suite, Lints and CI

`bash scripts/run-tests.sh` runs the whole harness suite and is what CI runs. The root `CLAUDE.md` asks contributors to run it before changing `plugins/pd-bot/hooks/` or `scripts/`, and to run `bash scripts/check-shellcheck.sh` before opening a PR.

## Layers of `scripts/run-tests.sh`

The script runs from the repo root, never stops at the first failure, and records every failed step so that a red run ends with a short `FAILURES:` list instead of leaving the cause buried in about a thousand lines of output. It prints `ALL GREEN` and exits 0 only when nothing failed.

1. **L1 static checks**: `bash -n` over every hook, plugin script, `scripts/*.sh` and test file; `lint-doc-truth.sh`; `lint-adr-paths.sh`; `lint-skill-bash.sh`; `check-hook-root-source.sh`; `check-plugin-validate.sh`; `check-plugin-version.sh`; `check-md-links.sh`; `check-allowed-tools.sh`; and, when `python3` exists, `check_allowed_tools_wide.py`, `check_frontmatter_contract.py`, `check_skill_tool_conformance.py` and the STE linter self-test. `check_ste_baseline.py` also runs but only reports.
2. **L2 hook tests** (`tests/hooks/*.test.sh`) and **L3 script tests** (`tests/scripts/*.test.sh`). Suites are hermetic, so they run in parallel with `xargs -P` (job count from `PD_BOT_TEST_JOBS`, default the CPU count). Each suite writes its own log and rc file, and the logs are replayed in glob order so the output reads as if serial. `PD_BOT_SKIP_SUITES` skips suites the caller already ran.
3. **Python unit tests**: every `test_*.py` under `scripts/` and `plugins/pd-bot/` is *collected by `find`*, not hand-listed, because a hand list had drifted and left four test files running nowhere. They run with pytest (with `pytest-xdist` when it imports) from a shared venv built once up front so parallel suites do not race on venv creation.

### What the L1 lints guard

- `lint-doc-truth.sh` fails when core docs (`CLAUDE.md`, `README.md`, `AGENTS.md`, the skills README, plugin agents and rules) mention repo paths that do not exist, or when the hook table in `CLAUDE.md` disagrees with `hooks/hooks.json` (✅ rows must be registered, every hook script must appear in the table).
- `check_frontmatter_contract.py` requires every skill and agent on disk to have frontmatter with non-empty `name:` (matching its directory or file name) and `description:` containing a trigger cue ("use" or "when"). It uses only the standard library, so it runs where the `claude plugin validate` CLI is unavailable.
- `check_allowed_tools_wide.py` fails when a skill's `allowed-tools` pre-allows a wide rule. This exists because a typed slash command applies a skill's `allowed-tools` without a prompt even under `dontAsk`, and a `*` inside a Bash rule also matches extra arguments (for example `git -C <dir> -c core.fsmonitor=<cmd>`). A wide entry passes only if listed in `scripts/allowed-tools-wide-baseline.json`, and the baseline may only shrink (checked against the baseline at `origin/main`, failing closed when git or the ref is missing). It does not judge `WebFetch`, `Agent`, `Skill` or MCP tools.
- `check_ste_baseline.py` compares each prose file's hard STE (Simplified Technical English) count with `scripts/ste-baseline.json`. It warns only and always exits 0, unless `--strict` is passed.
<!-- openwiki: broken internal link [/openwiki/operations/release-and-plugin-packaging.md] link "/openwiki/operations/release-and-plugin-packaging.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
- `check-plugin-version.sh`, `check-plugin-validate.sh` and the install smoke test are described in [Release and Plugin Packaging](/openwiki/operations/release-and-plugin-packaging.md).

## `tests/lib.sh`: hermetic contract-test helpers

Test files `source tests/lib.sh`. Hooks have a pure contract (stdin JSON plus arguments in; exit code plus output out), so each test copies the hook under test into a throwaway `mktemp` git repo (`new_repo`), stages files (`stage`), and runs the hook with that repo as the working directory (`run_hook`, which sets `OUT` and `RC`). Nothing in the real repo is touched, which is why suites can run in parallel. Hooks resolve the repo root from `CLAUDE_PROJECT_DIR`, else from git in the working directory, never from their own location. Assertions are `assert_rc`, `assert_rc_contains` and `assert_rc_not_contains`; `t` names a case, and `finish` prints the tally, removes the temp directories and exits nonzero if any case failed. `ensure_pyenv` builds the shared pytest venv and a `python` shim, because hooks prefer `python` while macOS and CI often have only `python3`. The case-name line format (`  FAIL  <name>`) is what `run-tests.sh` greps to list failures at the end.

<!-- openwiki: broken internal link [/openwiki/hooks/safety-guards-merge-secrets-evidence.md] link "/openwiki/hooks/safety-guards-merge-secrets-evidence.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
<!-- openwiki: broken internal link [/openwiki/hooks/hook-system.md] link "/openwiki/hooks/hook-system.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
<!-- openwiki: broken internal link [/openwiki/operations/fleet-checks-gate-and-settings.md] link "/openwiki/operations/fleet-checks-gate-and-settings.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
What the suites cover is mostly the guard hooks ([Safety Guards](/openwiki/hooks/safety-guards-merge-secrets-evidence.md), [Plugin Hook System](/openwiki/hooks/hook-system.md)), the fleet scripts ([Fleet Check, Gate and Settings Scripts](/openwiki/operations/fleet-checks-gate-and-settings.md)), the release scripts, and "contract" tests that pin the prompts and structure of skills and agents (for example `shape-contract`, `verify-contract`, `qa-verifier-agent`, `task-reviewer-readonly`).

## Strict mode versus local skip

Two gates call external tools and are designed so they never silently vanish in CI. Without the tool, locally they print a `skip` line and exit 0; with an environment flag they fail instead:

| Gate | Skips when | Flag that makes it fail |
|---|---|---|
| `check-shellcheck.sh` (runs `shellcheck` over the hooks, plugin scripts, the check-plugin and release scripts, and `check-pod-shape-ds.sh`) | `shellcheck` missing | `REQUIRE_SHELLCHECK=1` |
| `check-plugin-validate.sh`, `check-plugin-install.sh` | `claude` CLI missing | `REQUIRE_CLAUDE_CLI=1` |

## Evals

`tests/evals/tickets/` holds eight eval cases for `/pd-bot:pd-tickets` (hostile briefs, secrets in a brief, closed milestone names, milestone flags). Each runs `claude -p` in a throwaway directory with a stub `gh` first on `PATH` and a fake `GH_TOKEN`, so it cannot reach GitHub, and the fixture stays outside the run's directory so the run is blind to it. `mocks/run_case.sh` runs a case and `mocks/score.py` holds every check, exiting 1 if a `SAFETY` check fails. Because runs cost model usage, **CI does not run evals**, and results are not committed.

## GitHub workflows

- **`pd-bot-ci.yml`** runs on push to `main` and on every pull request. A `test` job runs `scripts/run-tests.sh` on a matrix of `ubuntu-latest` and `macos-latest` (the hooks are bash developed on macOS and run on Linux, so the BSD/GNU split is exactly what the matrix catches), with shellcheck installed on macOS, Node 22, and Claude Code CLI pinned to an exact version so a new release cannot turn CI red unprompted. It sets `REQUIRE_CLAUDE_CLI=1`. A separate Linux `plugin` job runs shellcheck with `REQUIRE_SHELLCHECK=1`, the frontmatter contract, link check, version check, strict plugin validation and the plugin install smoke test (not in `run-tests.sh` because it installs a plugin).
<!-- openwiki: broken internal link [/openwiki/pod-shape/design-system-rules.md] link "/openwiki/pod-shape/design-system-rules.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
- **`pod-shape-ci.yml`** runs only when `apps/pod-shape/**`, `scripts/check-pod-shape-ds.sh` or the workflow change, on Ubuntu only (no macOS-only code exists, and macOS minutes cost about ten times as much). Steps: install the Tauri system libraries, the design-system grep gate, `npm ci`, typecheck, Node tests (`*.test.mjs`, failing if none are found), build, `cargo test --locked`, and `cargo clippy -- -D warnings`. Its actions are pinned by commit SHA and it has `contents: read` permission and a cancel-in-progress concurrency group. See [Shape Board Design System](/openwiki/pod-shape/design-system-rules.md).
<!-- openwiki: broken internal link [/openwiki/operations/release-and-plugin-packaging.md] link "/openwiki/operations/release-and-plugin-packaging.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
- **`release.yml`** and **`release-tag.yml`** implement the release pipeline ([Release and Plugin Packaging](/openwiki/operations/release-and-plugin-packaging.md)).
- **`openwiki-update.yml`** runs daily (and on demand), runs `openwiki code --update --print` on the full history, and opens a `docs: update OpenWiki` pull request that stages only `openwiki`, `AGENTS.md`, the workflow and `CLAUDE.md` if present. If the run fails the PR keeps only the pages completed before the failure, and the job then fails.

<!-- openwiki: broken internal link [/openwiki/pod-board/alerts-and-build-signing.md] link "/openwiki/pod-board/alerts-and-build-signing.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
No workflow in the repo runs Pod Board's Rust tests (`cargo test` in `apps/pod-board/src-tauri`) or its acceptance scripts; those are run by hand ([Pod Board Alerts, Build and Signing](/openwiki/pod-board/alerts-and-build-signing.md)).
