---
title: Glossary
read_when: a word of this project is not clear
sources:
  - CLAUDE.md
  - plugins/pd-bot/skills/pd-orchestrate/references/podbay.md
  - plugins/pd-bot/skills/pd-orchestrate/references/protocol.md
  - plugins/pd-bot/skills/pd-orchestrate/references/run.md
  - apps/pod-shape/docs/storehouse-keeper.md
  - apps/pod-shape/docs/HANDOFF.md
checked: 2026-11-24
---

# Glossary

| Word | Meaning |
|---|---|
| pd-bot | The Claude Code plugin in `plugins/pd-bot/`. Its skills run as `/pd-bot:<name>` |
| Pod Bay | The tool (`podbay-dev`) that runs Claude Code sessions in tmux. pd-bot needs it |
| Workspace | A set of clean clones. `src` is the main one; `src-NN` is a parallel one |
| Session | One Claude Code conversation in one checkout. It has a `title`, a `name` and a `uuid` |
| Orchestrator | The session of `src`. It assigns, checks and decides; it does not write ticket code |
| Fleet | One orchestrator plus worker, reviewer, tester and advisor sessions that run tickets |
| Brief | The `assign` message: the ticket word for word, the outcome, the budget. Never steps |
| Depth S, M, L | How much design a ticket needs. Intake sets it; an owner label `depth:L` wins |
| Verdict | The intake result: READY, CONFIRM, GRILL or BLOCKED |
| `Verify:` row | A line in a ticket that names the command or check that proves the ticket |
| Receipt | The reviewer's JSON result for one head SHA: `pass` or `changes` |
| Report | The worker's JSON result (`report v2`) |
| Ledger | `specs/fleet/<run>/ledger.json`: the orchestrator's memory of a run. Local only |
| Outcome contract | The table of evidence that a finished ticket must show |
| Risky ticket | A ticket that names login, permissions, payments, secrets, personal data or deletion, or that changes `hooks/`, `scripts/`, `.github/` and similar paths |
| Merge gate | `fleet-gate.sh`: decides if a ticket pull request may go into `feat/<slug>` without the owner |
| Evidence check | `fleet-check.sh`: checks the end state of one ticket |
| Milestone run | Several tickets that ship together on one `feat/<slug>` branch |
| Plan-blind | An agent that must not read the plan or the diff, so it cannot share their mistake |
| ADR | A decision record in `docs/adr/NNNN-*.md` |
| `specs/` | Local scratch per ticket (intent, design, decision, plan, summary). Gitignored |
| `docs/solutions/` | Lessons from solved problems. Gitignored: local only |
| Shape Board | The desktop app in `apps/pod-shape` |
| Grill | The question round where `pd-shape` settles each unclear requirement with the owner |
| Brief (Shape Board) | `specs/<slug>/brief.md`: the result of one grill; one brief is one feature |
| Storehouse | The session `Pb Bot Storehouse`: the keeper of Shape Board UI parts |
| `UI-ASK` | The message an agent sends to the keeper to ask for a UI part |
| Design system | `apps/pod-shape/docs/DESIGN_SYSTEM.md`: the contract for every UI change |
| Pod Board | The desktop app in `apps/pod-board` that shows the state of each session |

> source: plugins/pd-bot/skills/pd-orchestrate/references/podbay.md
> | `title` | `Pb Bot #UA-04` | `SendMessage` (`to`), and when you name a session to the owner |

> source: plugins/pd-bot/skills/pd-orchestrate/references/run.md
> A depth the owner set wins over intake

> source: apps/pod-shape/docs/storehouse-keeper.md
> It is the one place every agent asks about a Shape Board UI part

> source: apps/pod-shape/docs/HANDOFF.md
> 1 grill session = 1 feature = 1 brief

Two words are easy to confuse: a **brief** in a fleet run is the `assign` message; a **brief** in
Shape Board is the file that a grill produces.
