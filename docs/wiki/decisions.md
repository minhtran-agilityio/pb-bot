---
title: Decisions
read_when: before you propose a design, to see what is already decided
sources:
  - docs/adr/0001-the-pod-bay-session-of-the-main-workspace-is-the-orchestrato.md
  - docs/adr/0002-the-orchestrator-role-is-a-role-text-with-no-tool-guard.md
  - docs/adr/0003-decisions-are-tracked-again-in-docs-adr-numbered-from-0001.md
  - docs/adr/0004-the-merge-guard-asks-the-owner-on-doubt-and-stops-only-habit.md
  - docs/adr/0005-pd-bot-runs-only-with-pod-bay-and-the-fleet-is-the-only-run-path.md
  - plugins/pd-bot/skills/pd-shape/references/adr.md
  - apps/pod-shape/docs/HANDOFF.md
  - apps/pod-shape/docs/DESIGN_SYSTEM.md
  - apps/pod-shape/docs/storehouse-keeper.md
checked: 2026-11-24
---

# Decisions

Do not reopen a decision on this page. To change one, write a new ADR that supersedes it.

## ADR and wiki: who owns what

- An ADR in `docs/adr/` is the record of one decision: the context, the options, the reason, and
  how to reverse it. It is the source of truth. It does not change after the merge.
- This page is an index of the ADRs. For each ADR it holds one quote of the decision, a link to
  the file, and what the decision changes for your work. It does not tell the reason again.
- If this page and an ADR disagree on a decision, the ADR wins. If an ADR and the code disagree
  on behavior, the code wins, and the difference goes on [known-gaps.md](known-gaps.md).
- Read the ADR file when you need the reason or the options. Do not copy them to a wiki page.
- `wiki_check.py` fails when an ADR file in `docs/adr/` is not in the `sources:` of a page, and
  when a quote on this page is no longer in its ADR.

## When a decision becomes an ADR

> source: plugins/pd-bot/skills/pd-shape/references/adr.md
> Propose an ADR only when the decision meets all 3:

1. It is hard to reverse.
2. It affects the architecture, the data, security or what users see beyond this feature.
3. It had two or more real options.

An ADR goes to `main` in a pull request of its own. `plugins/pd-bot/scripts/write_adrs.py` numbers it.

## The ADRs in `docs/adr/`

### 0001. The Pod Bay session of the main workspace is the orchestrator (2026-11-20)

Full record: [0001](../adr/0001-the-pod-bay-session-of-the-main-workspace-is-the-orchestrato.md)

> source: docs/adr/0001-the-pod-bay-session-of-the-main-workspace-is-the-orchestrato.md
> A second SessionStart hook, `plugins/pd-bot/hooks/session-role.sh`, adds the orchestrator role text only to the Pod Bay session of the main workspace.

- Risk: Pod Bay does not document these options. If it renames them, no session gets the role.
- Off switch: `{"orchestrator": "off"}` in `.claude/pd-bot.json`, or `PD_BOT_ROLE=none`.

### 0002. The orchestrator role is a role text, with no tool guard (2026-11-20)

Full record: [0002](../adr/0002-the-orchestrator-role-is-a-role-text-with-no-tool-guard.md)

> source: docs/adr/0002-the-orchestrator-role-is-a-role-text-with-no-tool-guard.md
> The hook injects the role text and nothing else; there is no PreToolUse guard.

- Consequence: nothing stops an orchestrator that edits code. The owner sees it in the diff.

### 0003. Decisions are tracked again in docs/adr, numbered from 0001 (2026-11-20)

Full record: [0003](../adr/0003-decisions-are-tracked-again-in-docs-adr-numbered-from-0001.md)

> source: docs/adr/0003-decisions-are-tracked-again-in-docs-adr-numbered-from-0001.md
> `.gitignore` ignores /docs/* and re-includes /docs/adr/. New ADRs start at 0001

- Consequence: an ADR number from the old repository (0008, 0012, 0041, 0043) names no file here.

> source: docs/adr/0003-decisions-are-tracked-again-in-docs-adr-numbered-from-0001.md
> A decision record that is not in the repository is lost for every other clone and for a reviewer.

### 0004. The merge guard asks the owner on doubt and stops only habitual merges (2026-11-22)

Full record: [0004](../adr/0004-the-merge-guard-asks-the-owner-on-doubt-and-stops-only-habit.md)

> source: docs/adr/0004-the-merge-guard-asks-the-owner-on-doubt-and-stops-only-habit.md
> Only `feat/*` passes without a question; every other base, and a base that cannot be read in 15 s, asks.

- Threat model: an agent that merges by habit. Not an agent that hides the command.
- Consequence: false questions stay, by design. In a subagent a question is a denial.

### 0005. pd-bot runs only with Pod Bay, and the fleet is the only run path (2026-11-22)

Full record: [0005](../adr/0005-pd-bot-runs-only-with-pod-bay-and-the-fleet-is-the-only-run-path.md)

> source: docs/adr/0005-pd-bot-runs-only-with-pod-bay-and-the-fleet-is-the-only-run-path.md
> Removed: the `ship.js` Workflow script, the `pd-autopilot` skill

- Kept: the temporary detached worktrees of `pd-review` and `fleet-check.sh`, and `clean-worktrees.sh`.
- Consequence: `pd-implement` steps no longer run in parallel. `fleet-check.sh` blocks only on a `feat/*` base.

## Owner decisions that are not ADRs

These are in tracked files of Shape Board. They bind the same way.

| Date | Decision | Source |
|---|---|---|
| 2026-11-22 | Shape Board is macOS only for V1; code lives in `apps/pod-shape` | `HANDOFF.md` section 4 |
| 2026-11-22 | The app does not commit the prototype HTML to GitHub | `HANDOFF.md` section 4 |
| 2026-11-22 | No "running" issue status in V1: four statuses | `HANDOFF.md` section 4 |
| 2026-11-22 | The "Cozy" design (v2.0) replaces the old UI | `DESIGN_SYSTEM.md` changelog |
| 2026-11-23 | A design pull request merges only after an independent review of that exact commit | `storehouse-keeper.md` step 5 |
| 2026-11-23 | Every `.html` file in an artifact folder is a version, in time order | `HANDOFF.md` section 11.4 |

> source: apps/pod-shape/docs/HANDOFF.md
> **Answered:** pd-bot has no clear signal, so "running" is dropped from V1

> source: apps/pod-shape/docs/storehouse-keeper.md
> the independent reviewer has sent a PASS for that exact head SHA (a new commit needs a new PASS)

> source: apps/pod-shape/docs/DESIGN_SYSTEM.md
> Owner decision that differs from the v2.0 design system artifact (WRkcjobJX9VCEi74o3mVjN): V1 keeps **four** issue statuses

The table "Agreed decisions (do not reopen)" in `apps/pod-shape/docs/HANDOFF.md` section 3 has the
full list for Shape Board. [shape-board.md](shape-board.md) repeats the ones that shape the code.
