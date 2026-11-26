---
title: Agents and context
read_when: you add an agent or a skill, or you ask which session reads which file
sources:
  - CLAUDE.md
  - AGENTS.md
  - REVIEW.md
  - plugins/pd-bot/hooks/session-rules.sh
  - plugins/pd-bot/hooks/orchestrator-role.md
  - plugins/pd-bot/skills/pd-orchestrate/SKILL.md
  - plugins/pd-bot/skills/pd-orchestrate/references/run.md
  - plugins/pd-bot/skills/pd-work/SKILL.md
  - plugins/pd-bot/skills/pd-review/SKILL.md
  - plugins/pd-bot/skills/pd-recall/SKILL.md
  - plugins/pd-bot/agents/implementer.md
  - plugins/pd-bot/agents/qa-writer.md
  - plugins/pd-bot/agents/qa-verifier.md
  - plugins/pd-bot/agents/intent-reviewer.md
  - plugins/pd-bot/agents/advisor.md
  - plugins/pd-bot/skills/pd-work/references/toolbox.md
  - apps/pod-shape/CLAUDE.md
  - .claude/agents/ui-designer.md
checked: 2026-11-24
---

# Agents and context

Sizes are bytes of the files on `main` at `1720e70` plus the wiki commit, measured with `wc -c`. One token is about 4
bytes of English text; that ratio is an estimate, not a measurement.

## What every session in this repository loads

| What | Size | How |
|---|---|---|
| `CLAUDE.md` | 13,655 | Claude Code loads it in full at the start. A subagent gets it too, by default |
| `plugins/pd-bot/rules/behavior.md` | 3,893 | the `session-rules.sh` hook prints it (main session) |
| skill and agent frontmatter | at most 15,026 | Claude Code lists the name and description of each one (the number is the whole frontmatter) |

`AGENTS.md` (3,847 bytes) is not loaded by Claude Code; it reads `CLAUDE.md`. `AGENTS.md` asks the
reader to open both:

> source: AGENTS.md
> Read both before you change a skill, an agent, a hook or a script.

## What each role reads next

| Role | Reads | Size | When |
|---|---|---|---|
| Orchestrator | role text; `pd-orchestrate/SKILL.md`; `protocol.md`, `podbay.md`, `run.md`, `stuck.md`, `milestone.md` | 1,445 + 3,791 + 47,028 | at the start of a run, all five references |
| Worker | the brief; `pd-work/SKILL.md`; `protocol.md`; `toolbox.md` | 18,325 + the brief | on `assign` |
| Reviewer | the ticket text; `pd-review/SKILL.md`; `protocol.md`; `REVIEW.md` from the base; `checks.md`; `order.md` | 21,389 + the ticket | on `assign` and `check-request` |
| Advisor session | `pd-advise/SKILL.md`; `protocol.md`; the problem brief | 14,767 + the brief | on `advise-request` |
| Shape Board feature agent | `apps/pod-shape/CLAUDE.md`; `HANDOFF.md`; `DESIGN_SYSTEM.md` | 4,425 + 20,308 + 56,386 | before any UI work |
| `ui-designer` agent | `DESIGN_SYSTEM.md` in full; `tokens.css`; `icons.ts`; the prototype when the part is on it | 70,387 + 288,226 for the prototype | per request |

> source: plugins/pd-bot/skills/pd-orchestrate/SKILL.md
> Before the first ticket, read the files in `references/`

> source: plugins/pd-bot/skills/pd-work/SKILL.md
> Nobody gives you steps: read the ticket and the code, then choose the shortest path that leaves the evidence the contract asks for.

> source: .claude/agents/ui-designer.md
> `docs/DESIGN_SYSTEM.md` in full

The orchestrator keeps its own context small on purpose: test output stays in the reviewer session.

> source: CLAUDE.md
> so test output stays out of the orchestrator's context

## Subagents and their reading limits

| Agent | Model | May read | Must not read |
|---|---|---|---|
| `implementer` | Sonnet 5.5 | `CLAUDE.md`, `.claude/rules/*.md`, the code | - |
| `test-runner` | Haiku 5.5 | `CLAUDE.md` section `Run and test`, the tests | it never edits |
| `researcher` | Haiku 5.5 | the repository, the web | it never edits |
| `advisor` | Opus 5.5 | what the one question needs (Glob, Grep, Read) | it never writes |
| `task-reviewer` | Opus 5.5 | the paths the caller gives | no Bash |
| `intent-reviewer` | Sonnet 5.5 | the owner's request and the diff | the plan |
| `qa-writer` | Opus 5.5 | the ticket, the public interface, `CLAUDE.md`, `README.md`, tests | application source, the plan, the design |
| `qa-verifier` | Sonnet 5.5 | the request, delivered files, user-facing docs | diffs, `git log`, `specs/`, `docs/adr/`, the plan |
| `security-reviewer` | Opus 5.5 | the ticket and the whole diff | `.env`, any secrets file |

> source: plugins/pd-bot/agents/qa-writer.md
> You must not read application source code, the plan, or the design beyond its public interface.

> source: plugins/pd-bot/agents/qa-verifier.md
> This blindness is enforced by this prompt, not by a hook.

> source: plugins/pd-bot/agents/intent-reviewer.md
> Do not read the plan, and ignore any plan text in the prompt.

## How agents find knowledge today

- Decisions: `docs/adr/` (tracked).
- Lessons: `docs/solutions/` (local only). `pd-shape`, `pd-xia2` and `pd-compound` read it.
- `/pd-bot:pd-recall` searches both, but only when the owner types it:

> source: plugins/pd-bot/skills/pd-recall/SKILL.md
> disable-model-invocation: true

- `REVIEW.md` holds review rules. The reviewer reads it from the base branch.

## How agents read this wiki

The design keeps the fixed cost near zero and never weakens a blind agent.

1. **One pointer, always loaded.** `CLAUDE.md` section Knowledge names `docs/wiki/index.md` in one
   line. The pages are not imported with `@path`: an import loads at the start of every session.
2. **Index first, then one page.** A reader opens `index.md` (under 4 KB) and then the one page whose
   "Read it when" line fits. `wiki_check.py` fails when the index is over 4 KB or a page is over 12 KB.
3. **Workers:** the toolbox of `pd-work` has a row that sends a worker to the index when it does not
   know how a part is built or which rule applies.
4. **Blind agents are not sent here.** `qa-writer`, `qa-verifier`, `intent-reviewer` and the UAT
   agent keep the limits above. The wiki holds no plan and no open ticket, so a `qa-verifier` that
   opens it as a user-facing doc learns nothing that it must not know.
5. **Writers:** only `/pd-bot:pd-wiki` changes the pages, in a pull request of its own.

> source: plugins/pd-bot/skills/pd-work/references/toolbox.md
> read `docs/wiki/index.md`, then only the page that fits

Cost of this wiki when it was first written: the index is 2.0 KB and the largest page is 7.0 KB. A reader
that needs one page loads at most 9 KB, once, only when it asks.
