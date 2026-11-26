---
type: operations
title: Release and Plugin Packaging
description: How the pd-bot plugin is packaged (marketplace, plugin manifest, VERSION) and released through a two-workflow, PR-gated pipeline built on release.sh, release-guard.sh and release-tag.sh, plus the plugin install and validate checks and the release-notes/summary skills.
tags: [release, packaging, versioning, github-actions, marketplace]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-26T15:02:34.678Z
sources:
  - id: openwiki-source-9956d5e26bb8ccda5a071263
    resource: repo://.github/workflows/release-tag.yml
  - id: openwiki-source-4d1d392666be6dfdd7a91a2e
    resource: repo://.github/workflows/release.yml
  - id: openwiki-source-2eb09385728883188d0e0b24
    resource: repo://scripts/check_release_summary.py
  - id: openwiki-source-30cc5ad1d52df4206e062f3e
    resource: repo://scripts/check-plugin-install.sh
  - id: openwiki-source-f6029e8e0abe4e747763a2c8
    resource: repo://scripts/check-plugin-validate.sh
  - id: openwiki-source-baaa4651335e443ce11e6463
    resource: repo://scripts/check-plugin-version.sh
  - id: openwiki-source-a676eee582e309112e602198
    resource: repo://scripts/release-guard.sh
  - id: openwiki-source-02d8d50132faf294dd98577a
    resource: repo://scripts/release-tag.sh
  - id: openwiki-source-76478c25db28b99104e23105
    resource: repo://scripts/release.sh
generated: { by: "claude-code", at: "2026-11-26T15:02:34.678Z" }
---

# Release and Plugin Packaging

## Packaging

<!-- openwiki: broken internal link [/openwiki/testing/test-suite-and-ci.md] link "/openwiki/testing/test-suite-and-ci.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
The repository root holds `.claude-plugin/marketplace.json`, which lists one plugin (`pd-bot`, source `./plugins/pd-bot`). The plugin's own manifest is `plugins/pd-bot/.claude-plugin/plugin.json`. A root `VERSION` file is the single release number, and the manifest `version` must equal it. `scripts/check-plugin-version.sh` enforces that equality (it reads the `"version"` line with `sed`, since `jq` may be absent) and says explicitly that it does not verify the number is the *right* release. It runs in the main suite ([Test Suite, Lints and CI](/openwiki/testing/test-suite-and-ci.md)).

Two further checks drive the real Claude Code CLI:

- `scripts/check-plugin-validate.sh` runs `claude plugin validate --strict` over `plugins/pd-bot` and over the repo root (the marketplace file). Strict mode turns warnings, such as a skill with no frontmatter, into failures.
- `scripts/check-plugin-install.sh` is an offline install smoke test: in an isolated `CLAUDE_CONFIG_DIR` it adds the marketplace, installs `pd-bot@pd-bot`, and checks that the reported skills, agents and hook events match the source tree and that the cached copy keeps the executable bit on `hooks/*.sh` and `scripts/*.sh`. It refuses to run unless `CLAUDE_CONFIG_DIR` is set to its own directory, so it can never touch the real `~/.claude`.

Both skip with a visible message when the `claude` CLI is missing, unless `REQUIRE_CLAUDE_CLI=1` is set, which turns the skip into a failure; CI sets it so the gate cannot quietly vanish.

## Release pipeline

A release is two workflows joined by a pull request, so a human merge is always the trigger for tagging.

```
workflow_dispatch(bump) ──► release.yml ──► release PR "chore(release): vX.Y.Z"
                                                    │ owner merges
                                                    ▼
                          push to main touching VERSION ──► release-tag.yml ──► tag + GitHub release
```

1. **`release.yml`** (manual dispatch with a `patch|minor|major` choice) requires the ref to be `main`, checks out full history and tags, and runs `scripts/release-guard.sh`. It then runs `scripts/release.sh <bump>`, stops if tag `vX.Y.Z` or branch `release/vX.Y.Z` already exists, commits `VERSION`, `CHANGELOG.md` and `plugin.json` on a new `release/vX.Y.Z` branch, pushes, and opens the PR whose body is the new changelog section. A `concurrency` group serialises runs so two dispatches cannot both pass the guard. The only input goes through `env`, never into a `run:` line, and the workflow runs only code already on `main`.
2. **`release-tag.yml`** triggers on a push to `main` that changes `VERSION` (or manually), and runs `scripts/release-tag.sh`.

### release.sh

Bumps `VERSION` and the manifest version together and adds a dated `## [x.y.z]` section to `CHANGELOG.md`. The section contains the bullets that were under `## [Unreleased]` (moved, leaving it empty) followed by one `- <PR title> (PR #N)` bullet per merged PR since the last `v*` tag. PRs are read from the **first-parent** history only: merge commits (title taken from the body) and squash/rebase commits whose subject ends in ` (#N)`; commits off the first-parent line are ignored because their `(#N)` is often an issue number. The release PR itself (exactly `chore(release): vX.Y.Z`) and `chore(bookkeeping)` commits are skipped. It needs no network and no `gh`.

Safety properties: `VERSION` must be strict semver before any arithmetic; a dirty working tree is refused (except with `--dry-run`); an existing `## [new]` heading or an empty release (no `[Unreleased]` bullets and no PRs) is refused; commit titles are collapsed to one line and passed to `awk` through `ENVIRON`, never `-v` or `eval`. The last stdout line is `NEW_VERSION=x.y.z`, which the workflow validates. `RELEASE_DATE` overrides the date for tests.

### release-guard.sh

Refuses to start a second release while one is in flight, because merging the wrong release PR would tag the wrong version and a tag cannot be fixed afterwards. It fails if an open PR has a head branch starting with `release/`, or if `origin` has a `release/vX.Y.Z` branch whose tag `vX.Y.Z` does not exist on origin (a release is finished when its tag exists, regardless of how the PR was closed; a `release/` branch not named that way cannot be shown finished, so it blocks). Exit 0 means clear, 1 means a release is in progress (all findings named), and 2 means the check could not run. It **fails closed**: a failed `gh` or `git ls-remote` query is never read as "nothing is open".

### release-tag.sh

Reads `VERSION`, re-runs the plugin-version check, and builds release notes from the `## [VERSION]` section of `CHANGELOG.md` *before* creating any tag, so a missing section stops the run first. If the tag already exists locally or on origin, its commit's `VERSION` must equal the current one; otherwise the script exits 1 and never moves, deletes or overwrites a tag. Otherwise it creates an annotated tag at `HEAD` and pushes only that tag. Unless `--no-release`, it creates the GitHub release with `gh release create --verify-tag`, passing `--latest` only when this version is the highest `v*` tag and `--latest=false` otherwise. A re-run with everything in place changes nothing.

## Release notes and summaries (skills)

Two user-invoked skills prepare human-facing text; neither tags, pushes, publishes or merges.

- **`pd-release-notes`** drafts technical release notes for a milestone or a tag range: breaking changes (from `!:` titles, `BREAKING CHANGE:` footers or a label), grouping by conventional-commit type, and known limitations from PR bodies. It also writes a fact-list JSON beside the draft.
- **`pd-release-summary`** reads that fact list and drafts a plain-language summary for a chosen audience, with `What you need to do:` under each breaking change and `[NEEDS HUMAN]` on items to confirm. `scripts/check_release_summary.py <fact-list.json> <draft.md>` checks traceability: every cited `(#n)` is a PR in the fact list, every `## Breaking changes` bullet has a `What you need to do:` line, every PR in the fact list's `breaking` array appears there, and every other bullet cites a PR or carries `[NEEDS HUMAN]`. It does not verify that a statement is true to its PR.

<!-- openwiki: broken internal link [/openwiki/workflows/ticket-to-pr-skill-pipeline.md] link "/openwiki/workflows/ticket-to-pr-skill-pipeline.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
See [Ticket-to-PR Skill Pipeline](/openwiki/workflows/ticket-to-pr-skill-pipeline.md) for where these skills sit among the others.
