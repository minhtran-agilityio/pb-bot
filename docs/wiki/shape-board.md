---
title: Shape Board logic
read_when: you work on apps/pod-shape, its flow, data, statuses and settled decisions
sources:
  - apps/pod-shape/CLAUDE.md
  - apps/pod-shape/docs/HANDOFF.md
  - apps/pod-shape/docs/DESIGN_DOC.md
  - apps/pod-shape/docs/DESIGN_SYSTEM.md
  - apps/pod-shape/spike/README.md
checked: 2026-11-24
---

# Shape Board logic

Shape Board is a personal desktop app for the owner's Mac. For the UI rules read
[shape-board-ui.md](shape-board-ui.md).

> source: apps/pod-shape/docs/HANDOFF.md
> It takes a prototype (a Claude artifact) through a "grill" step and then creates GitHub milestones and issues from it.

## Which document wins

| Topic | Source of truth |
|---|---|
| Behavior, data, protocol | `apps/pod-shape/docs/DESIGN_DOC.md` (Vietnamese), with the newer owner decisions in its first note |
| Looks | `apps/pod-shape/docs/DESIGN_SYSTEM.md` |
| Build order, settled decisions, the artifact hook | `apps/pod-shape/docs/HANDOFF.md` |

> source: apps/pod-shape/docs/HANDOFF.md
> If the design doc and the design system disagree, follow the design doc for behavior and the design system for looks.

## The flow and the tree

Flow: add a repo, add an artifact, grill it, generate issues, see everything in one tree.

> source: apps/pod-shape/docs/HANDOFF.md
> The tree: Project (= 1 GitHub repo) → Artifact → Feature (= 1 brief) → Milestone → Issue.

Business rules:

- One grill session gives one feature and one brief. One artifact can be grilled many times.
- The brief is `specs/<slug>/brief.md`. It is local scratch and is not committed. The owner confirms it in the app.
- "Generate issues" is enabled only after the owner confirms the brief.
- A new feature with 2 or more issues gets a milestone. A small update is 1 issue.
- A new file or a new paste for the same artifact is a new version. A new version does not start a new grill.

> source: apps/pod-shape/docs/HANDOFF.md
> A new feature with 2 or more issues gets a milestone. A small update is 1 issue.

## Issue status

V1 has four statuses, read live from GitHub. There is no "running" status.

> source: apps/pod-shape/docs/HANDOFF.md
> Issue status is read live from GitHub: open, has PR, merged, closed.

| Status | How the app knows |
|---|---|
| open | the issue is open and has no linked pull request |
| has PR | an open pull request is linked to the issue |
| merged | a merged pull request closed the issue |
| closed | the issue is closed with no merged pull request |

The status in SQLite is a cache. GitHub is the source of truth. One GraphQL call reads a whole project.

## How the app talks to Claude and GitHub

The app holds no API key and no token. It runs two CLIs that are logged in on the Mac.

> source: apps/pod-shape/docs/HANDOFF.md
> The app runs the Claude Code CLI in the background: `claude -p` with `--session-id` on the first turn and `--resume <id>` after that

- Grill logic is the `pd-shape` skill. The app does not rewrite its rules in Rust.
- `AskUserQuestion` is off. Claude prints each question as a fenced `p2g-questions` JSON block; the app
  shows cards and sends the answers back with `--resume`.
- The app knows the brief is ready by a new or changed `specs/<slug>/brief.md` after a turn, or by a
  question whose `header` is `Brief`. The `p2g-brief` block of the first design is dropped
  (`DESIGN_DOC.md`, the change of build step 3).
- Issues come from `/pd-bot:pd-tickets specs/<slug>/brief.md`. The app finds the new issues by their
  `source:` line, not by the text Claude prints.
- GitHub access is the `gh` CLI.

> source: apps/pod-shape/docs/HANDOFF.md
> Reuse the `pd-shape` skill from pd-bot. Do not rewrite its rules in Rust.

Safety rules of the runner:

> source: apps/pod-shape/docs/HANDOFF.md
> Never use `--dangerously-skip-permissions`. Allow only the tools each step needs, with `--allowedTools`.

> source: apps/pod-shape/spike/README.md
> --permission-mode dontAsk

The app blocks every GitHub write until the owner approves in the review screen.

## Grill session states

| State | Meaning |
|---|---|
| `running` | Claude answers; the owner can press Stop |
| `waiting_owner` | a question waits for the owner |
| `brief_ready` | a brief waits for the owner's confirmation |
| `confirmed` | the owner confirmed; the feature exists |
| `stopped` | the owner stopped; `--resume` continues it |
| `failed` | an error, such as a usage limit; the app shows the real error |

> source: apps/pod-shape/docs/DESIGN_DOC.md
> | `waiting_owner` | Có câu hỏi chờ anh |

## Data

> source: apps/pod-shape/CLAUDE.md
> SQLite at `~/Library/Application Support/io.pdbot.shapeboard/shape-board.db`.

> source: apps/pod-shape/CLAUDE.md
> Add a new file for a change; never edit one that shipped.

Tables of the design: `projects`, `artifacts`, `artifact_versions`, `grill_sessions`,
`grill_messages`, `features`, `briefs`, `milestones`, `issues`, `runs`. On `main` at `1720e70` the
migrations are `0001_projects.sql`, `0002_artifacts.sql` and `0003_grill.sql` (`grill_sessions`,
`grill_messages`, `briefs`, `features`, `runs`); `milestones` and `issues` are not built yet. A brief
belongs to its grill session, not to a feature: the owner's confirmation makes the feature
(`DESIGN_DOC.md`, the change of build step 3).

Artifact rules from migration 0002:

- `artifacts.state` is `new` (in the "New artifacts" box), `added` (in a project) or `ignored`.
- An artifact version stores its HTML (at most 5 MB) and a `sha256`. A file with the same hash as
  the latest version is skipped.

## Artifact import

The main way is a hook on the owner's Mac. It saves each published artifact to
`~/claude-artifacts/<artifactId>/` and adds a line to `~/.claude/artifact-log.tsv`.

> source: apps/pod-shape/docs/HANDOFF.md
> The hook only writes. The app only reads.

- The HTML files are the source of truth. The log is only an index.
- Fallback: drag-and-drop an `.html` file, or paste the link and the HTML.
- Limit: an artifact made on claude.ai or in a cloud session does not pass through the hook.

## Preview safety

> source: apps/pod-shape/docs/HANDOFF.md
> Use an iframe with `sandbox="allow-scripts"` and without `allow-same-origin`, so the pasted prototype cannot reach Tauri commands

## Not in V1

> source: apps/pod-shape/docs/HANDOFF.md
> No "run podbay" button. No auto mode. No sync across machines. No fetching from claude.ai

## Build steps

One step is one small pull request: 0 spike (done), 1 app skeleton, 2 artifact, 3 grill, 4 generate
issues, 5 tree and status. `HANDOFF.md` section 6 has the done point of each step. On `main` at
`1720e70` the screens are `ProjectsScreen`, `ProjectScreen`, `ArtifactPreviewDialog` and `GrillScreen` (steps 1 to 3).
