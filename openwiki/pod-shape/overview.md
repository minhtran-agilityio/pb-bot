---
type: system
title: Shape Board App
description: Architecture of Shape Board (apps/pod-shape), a personal macOS Tauri 2 + React app that imports Claude prototypes, grills them through a sandboxed `claude -p` loop running the pd-shape skill, and stores sessions, briefs and ADR drafts in SQLite.
tags: [pod-shape, tauri, react, sqlite, grill, claude-cli, adr]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-26T15:02:34.678Z
sources:
  - id: openwiki-source-a86f9a738a2ed58ee4e69c1d
    resource: repo://apps/pod-shape/docs/HANDOFF.md
  - id: openwiki-source-c38cfd9c6b37864fb3112e54
    resource: repo://apps/pod-shape/src-tauri/migrations/0002_artifacts.sql
  - id: openwiki-source-b61f6cddeb8b79a8407ee320
    resource: repo://apps/pod-shape/src-tauri/migrations/0003_grill.sql
  - id: openwiki-source-ce6a173c8ec555ebe8f9b506
    resource: repo://apps/pod-shape/src-tauri/src/db.rs
  - id: openwiki-source-bc124f58192158b043c4ff9e
    resource: repo://apps/pod-shape/src-tauri/src/grill/engine.rs
  - id: openwiki-source-23dd5394dd1f98588ef7819d
    resource: repo://apps/pod-shape/src-tauri/src/grill/flags.rs
  - id: openwiki-source-142e7e875600061cd757d94c
    resource: repo://apps/pod-shape/src-tauri/src/lib.rs
generated: { by: "claude-code", at: "2026-11-26T15:02:34.678Z" }
---

# Shape Board App

<!-- openwiki: broken internal link [/openwiki/pod-shape/design-system-rules.md] link "/openwiki/pod-shape/design-system-rules.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
<!-- openwiki: broken internal link [/openwiki/architecture/overview.md] link "/openwiki/architecture/overview.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
Shape Board (`apps/pod-shape/`) is a personal, macOS-only desktop app (Tauri 2 Rust backend, React + TypeScript + Tailwind v4 UI). Its purpose, per `docs/HANDOFF.md`, is to take a prototype (a Claude artifact) through a **grill** and then turn the confirmed brief into GitHub milestones and issues. The intended object tree is Project (one GitHub repo) → Artifact → Feature (one brief) → Milestone → Issue, with issue status read live from GitHub. It uses the owner's Claude subscription by running the Claude Code CLI (no API key) and the `gh` CLI (the app stores no tokens). UI rules are in [Shape Board Design System](/openwiki/pod-shape/design-system-rules.md); how it relates to the other deliverables is in the [Architecture Overview](/openwiki/architecture/overview.md).

## Build plan and what the code covers

HANDOFF §6 splits the work into one PR per step: spike, app skeleton (step 1), artifact import (step 2), grill (step 3), generate issues (step 4) and tree + status (step 5). The Rust code and registered Tauri commands in `lib.rs` cover steps 1–3: projects and checks, artifacts, and the grill with ADR publishing. The command list contains no issue-generation or tree-status command, so steps 4 and 5 are still plan, not code.

## Backend structure (`src-tauri/src/`)

| Module | Responsibility |
|---|---|
| `lib.rs` | `run()`: plugin setup, single-instance lock, DB open, grill engine start and recovery, main window with a navigation guard, artifact watcher, the Tauri command list |
| `db.rs` | SQLite open and migrations |
| `checks.rs` | Add-project dialog checks: `gh`, `claude`, and the repo folder; repo-name normalisation; cancellation |
| `artifacts/` | Artifact intake: `paths` (hook locations), `log` (hook log reader), `sync` (import), `watch` (file watcher), `store` (versions), `preview` (sandboxed preview protocol) |
| `grill/` | The grill engine: `engine`, `flags` (CLI argument building and permission fence), `proc` (process groups), `stream` (stream-json parsing), `p2g` (question-block parsing), `store`, `brief`, `adr` |
| `nav.rs` | Navigation guard for the main webview |

### Storage and migrations

SQLite lives at `~/Library/Application Support/io.pdbot.shapeboard/shape-board.db` (WAL mode, foreign keys on). Migrations are `migrations/NNNN_*.sql`, listed in the `MIGRATIONS` array in `db.rs` and each run in its own transaction; `PRAGMA user_version` records how many ran, so opening an already-migrated database re-runs nothing. The rule is to add a new file for a change and never edit one that shipped. The three migrations create `projects` (0001), `artifacts`, `artifact_versions`, `hook_files` and `hook_state` (0002), and `grill_sessions`, `grill_messages`, `briefs`, `features` and `runs` (0003). A session's state is constrained to `running`, `waiting_owner`, `brief_ready`, `confirmed`, `stopped` or `failed`.

### Artifact intake (step 2)

The main source is a local PostToolUse hook in the owner's personal Claude config that saves each artifact the owner publishes (an HTML copy plus a log line); the app only reads. A file watcher (`artifacts::watch`) runs a sync at start and on every change, emitting an `artifacts-changed` event. Drag-and-drop or pasted HTML is the fallback. Each new file creates a new version (HTML stored up to 5 MiB, with a SHA-256; a hook file is imported once). An artifact with no project is in the "New artifacts" inbox until added or ignored. The preview runs in an iframe from a custom `preview://` scheme using single-use tokens, and the sandbox is `allow-scripts` without `allow-same-origin`, so untrusted HTML cannot reach Tauri commands; `nav.rs` also stops untrusted HTML becoming a main-frame document, and new windows and downloads are denied.

### The grill engine (step 3)

Each grill turn is **one `claude -p` process** (`grill/engine.rs`), with state in the database. The first turn uses `--session-id`, later turns `--resume`. Instead of typing `/pd-bot:pd-shape`, the app reads the installed plugin's `pd-shape/SKILL.md`, strips its frontmatter and sends the body as the first message. This is deliberate: a skill's `allowed-tools` would widen the permission fence when started by its slash command, even in `dontAsk` mode. The app reuses the skill rather than reimplementing its rules.

`grill/flags.rs` builds the fence for every turn: `--permission-mode dontAsk` with `--permission-prompts none`, `--setting-sources ""` (no user or repo settings can add allow rules), `--strict-mcp-config`, model `sonnet` at effort `medium`, and `--append-system-prompt` with the question protocol. Allowed tools are scoped reads (`Read`, `Glob`, `Grep` on the repo, the prototype folder and the plugin folder) and `Edit` only on `specs/<slug>/brief.md` and `design.md` (once the brief has a slug, only that feature's). `AskUserQuestion`, `Bash`, `Agent`, `WebFetch`, `WebSearch`, `NotebookEdit` and `Skill` are denied, as are secret globs (`.env*`, `.git/**`, keys, credentials). The user's message goes after `--`, so an answer starting with `-` is never read as a flag. The flag tests assert that no turn ever contains `bypassPermissions` or a "dangerously" flag.

Because `AskUserQuestion` is disabled, Claude prints questions as a fenced `p2g-questions` JSON block; `p2g` validates it, the UI shows question cards, and answers return via `--resume`. A question with header `Brief` asks whether the brief says what the owner means; header `ADR` asks whether to record an ADR. After each successful turn `next_state` computes the session state: a `Brief` question with an existing, unconfirmed brief gives `brief_ready`; any other block gives `waiting_owner`; no block with a confirmed brief gives `confirmed`. A changed `brief.md` is stored as a new brief version.

**Failure and recovery.** A turn has a timeout (900 s) and Stop terminates the process group; an interrupted turn leaves a `pending_message` that Resume re-sends wrapped as "the last turn was interrupted… continue", never replaying the old answers verbatim because Claude may already have acted on them. At app start `recover()` kills any `claude` process group left in `runs` (checking the recorded process start time so a reused pid is not killed) and shows sessions that were `running` as `stopped`; on exit `shutdown()` kills the live group. A second app instance is refused by a single-instance lock.

**ADR publishing.** The grill never gives Claude a shell. When the owner answers yes to an `ADR` question and confirms a dialog, one ADR-step turn lets Claude write only a decisions file in a scratch folder. The app then runs `write_adrs.py`, verifies the change adds only new `docs/adr/NNNN-*.md` files, and *prepares* it for the owner to read; only on `adr_decide(publish=true)` does it commit on `adr/<slug>`, push just that branch and open a PR with `gh pr create`. It never merges. This is why the git work lives in the app (`grill/adr.rs`): a probe showed that a `*` in a Bash permission rule also matches extra arguments, so Bash rules cannot fence git.

## Frontend

`src/main.tsx` and `App.tsx` mount the screens (`ProjectsScreen`, `ProjectScreen`, `GrillScreen`, `ArtifactPreviewDialog`). `src/lib/api.ts` wraps the Tauri commands, `grill.ts`, `grillHelpers.ts` and `useGrill.ts` hold the grill client state (reloaded on each `grill-changed` event with a session id), and `theme.ts` handles light/dark. Components live under `components/ui` (shadcn primitives) and `components/app` (app parts such as `SessionList`, `Composer`, `BriefCard`, `RunLog`).

## Running and testing

<!-- openwiki: broken internal link [/openwiki/testing/test-suite-and-ci.md] link "/openwiki/testing/test-suite-and-ci.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
`npm install`, then `npm run tauri dev` (Vite on port 1430) or `npm run tauri build`; `npm run typecheck` for TypeScript and `cargo test --manifest-path src-tauri/Cargo.toml` for Rust (migration idempotence, flag fences, engine state table, and so on). The debug-only environment variable `SHAPE_BOARD_CLAUDE` swaps in a stand-in `claude` binary for tests and is ignored in release builds. An end-to-end example lives in `src-tauri/examples/grill_e2e.rs`. CI is `.github/workflows/pod-shape-ci.yml` ([Test Suite, Lints and CI](/openwiki/testing/test-suite-and-ci.md)).
