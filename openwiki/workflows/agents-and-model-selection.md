---
type: reference
title: Subagents and Model/Effort Selection
description: The nine pd-bot role agents with their frontmatter-enforced tools, models and efforts, plus the rules for choosing models for subagents and fleet sessions, the hard limits on worker models, and when the advisor is called.
tags: [agents, models, effort, advisor, permissions, fleet]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-26T15:02:34.678Z
sources:
  - id: openwiki-source-a2371d6362e5db4bc834ad03
    resource: repo://CLAUDE.md
  - id: openwiki-source-f7e7f55f14640c566f6decc9
    resource: repo://plugins/pd-bot/agents/advisor.md
  - id: openwiki-source-23672a59f6a4bba96e598289
    resource: repo://plugins/pd-bot/agents/implementer.md
  - id: openwiki-source-045548f98e025142fdee9eb4
    resource: repo://plugins/pd-bot/agents/intent-reviewer.md
  - id: openwiki-source-197d4705395adaeae3e0fc34
    resource: repo://plugins/pd-bot/agents/qa-verifier.md
  - id: openwiki-source-da829e82f47d5fdb372143e8
    resource: repo://plugins/pd-bot/agents/qa-writer.md
  - id: openwiki-source-6d96fa78390f8e70219f47d9
    resource: repo://plugins/pd-bot/agents/researcher.md
  - id: openwiki-source-9fe1e0ebf0a7ed2bcefc0f07
    resource: repo://plugins/pd-bot/agents/security-reviewer.md
  - id: openwiki-source-cf692877ecc52ebf1014e1b9
    resource: repo://plugins/pd-bot/agents/task-reviewer.md
  - id: openwiki-source-16ba5652c3b82fddcad8bfcd
    resource: repo://plugins/pd-bot/agents/test-runner.md
  - id: openwiki-source-08ae4b27471cecfc629f21e6
    resource: repo://plugins/pd-bot/scripts/fleet-settings.py
  - id: openwiki-source-3beed8213a5f4eeda00c658d
    resource: repo://tests/scripts/qa-security-agents.test.sh
  - id: openwiki-source-f3e105a14f6dd9591552b5d3
    resource: repo://tests/scripts/task-reviewer-readonly.test.sh
generated: { by: "claude-code", at: "2026-11-26T15:02:34.678Z" }
---

# Subagents and Model/Effort Selection

<!-- openwiki: broken internal link [/openwiki/workflows/ticket-to-pr-skill-pipeline.md] link "/openwiki/workflows/ticket-to-pr-skill-pipeline.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
<!-- openwiki: broken internal link [/openwiki/workflows/fleet-orchestration.md] link "/openwiki/workflows/fleet-orchestration.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
The plugin ships nine role agents in `plugins/pd-bot/agents/`. Each is a Markdown file whose **frontmatter fixes the tools, model and effort**, so the runtime rather than the prompt limits what the agent can do. The rules for which model to use when are in the root `CLAUDE.md` ("Models") and are enforced where possible by scripts. Skills and sessions that use these agents are described in [Ticket-to-PR Skill Pipeline](/openwiki/workflows/ticket-to-pr-skill-pipeline.md) and [Fleet Orchestration](/openwiki/workflows/fleet-orchestration.md).

## The agents

| Agent | Tools | Model / effort | Role |
|---|---|---|---|
| `advisor` | Glob, Grep, Read | Opus 5.5 / high | Answers one question with a recommendation, reasons, risks and a confidence level; never fixes, never writes |
| `implementer` | all tools (no `tools:` line) | Sonnet 5.5 / medium | Implements one plan step with a minimal, scoped diff; defers all stack specifics to the project's `CLAUDE.md` and `.claude/rules/` |
| `intent-reviewer` | Glob, Grep, Read, Bash (read-only commands only) | Sonnet 5.5 / medium | Checks a diff range against the owner's verbatim request, blind to the plan; reports gap / drift / excess |
| `qa-writer` | Bash, Read, Write, Glob | Opus 5.5 / medium | Writes blind acceptance tests before any code exists |
| `qa-verifier` | Bash, Read, Glob, Grep | Sonnet 5.5 / medium | Checks a ticket with no app against the request, running commands and reading delivered files; never edits |
| `security-reviewer` | Glob, Grep, Read, Bash | Opus 5.5 / high | Read-only security gate for risky tickets (login, permissions, payments, secrets, personal data, bulk deletion) |
| `task-reviewer` | Glob, Grep, Read | Opus 5.5 / medium | One read-only per-task review with separate spec and quality verdicts |
| `test-runner` | Glob, Grep, Read, WebFetch, WebSearch, Bash | Haiku 5.5 | Runs the tests relevant to recent changes and reports; never edits |
| `researcher` | Glob, Grep, Read, Bash, WebFetch, WebSearch | Haiku 5.5 | Read-only fact-finding returning facts with `file:line` or URL |

### Behavioral contracts in the prompts

- **Blindness.** `qa-writer` may read only the ticket, its criteria, the public interface, `CLAUDE.md`, `README.md`, `.claude/rules/` and existing test files, and must not read application source code, the plan or the design; if the public interface is not enough for a criterion it returns `ready: false` instead of peeking. `intent-reviewer` must not read the plan and ignores any plan text in its prompt, because a plan can carry the same misreading the review exists to catch. `qa-verifier` is blind to the plan, the diff and the `Verify:` row.
- **Evidence discipline.** Under the repo rule that "not observed" is not "absent", `advisor` and `intent-reviewer` must name the paths or commands they searched before saying something does not exist, and report an unsourced claim as `unknown`.
- **Scope stops.** `implementer` implements the task and nothing else, and reports `BLOCKED` instead of changing a schema, public contract, auth, a dependency, `settings.json` or `hooks/`, or removing a feature the spec did not name. Its handoff is a status (`DONE`, `BLOCKED`, `NEEDS_CONTEXT`), commits, files touched, the Done-when result, deviations and blockers.
- **Read-only by construction.** `task-reviewer` and `advisor` list only read tools. `intent-reviewer` has `Bash` but its prompt limits it to `git diff`, `git log`, `git show` and `gh issue view`, and says never to read `.env`; for the security reviewer the prompt itself states that its read-only behavior is "enforced by this prompt, not by a hook".

<!-- openwiki: broken internal link [/openwiki/testing/test-suite-and-ci.md] link "/openwiki/testing/test-suite-and-ci.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
Two contract tests pin this. `tests/scripts/task-reviewer-readonly.test.sh` requires `tools: Glob, Grep, Read`, model `claude-opus-5-5`, and fails if the frontmatter lists any mutating or network tool. `tests/scripts/qa-security-agents.test.sh` checks that `qa-writer` carries no `Edit`, `Agent`, web or MCP tool and states the "must not read application source code" rule, and that `security-reviewer` runs on Opus with no write, edit, agent, web or MCP tool. The two tests are part of the suite ([Test Suite, Lints and CI](/openwiki/testing/test-suite-and-ci.md)).

## Model and effort selection rules

The root `CLAUDE.md` sets defaults and a few hard limits; everything else is judgment.

- **Defaults.** The main session runs Sonnet 5.5; the fleet orchestrator runs Opus 5.5. Workers run Sonnet 5.5 at depth S and M and Opus 5.5 at depth L. The fleet reviewer session runs Sonnet 5.5 at effort `high` at depth S and M, and Opus 5.5 at depth L, on a risky ticket and for the whole-feature review. Coding uses `implementer`, testing `test-runner`, search `researcher`, intent review `intent-reviewer`, and checking a no-app ticket `qa-verifier`.
- **The orchestrator decides.** It chooses the model and effort per subagent or session from how hard the task is. The main session always passes `model` to an agent that has no model in its frontmatter, because such an agent would otherwise inherit the main session's model. `implementer` and the others here do set a model.
- **Effort guidance.** Sonnet 5.5: `medium` for well-specified work, `high` for harder work, and `low` only for mechanical stages because it can skip verification. Opus 5.5: start at `medium`; use `xhigh` or `max` only where a quality gain was measured. To get less thinking, lower effort rather than adding prompt instructions.
<!-- openwiki: broken internal link [/openwiki/operations/fleet-checks-gate-and-settings.md] link "/openwiki/operations/fleet-checks-gate-and-settings.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
- **Hard limits** (not adjustable by judgment): the Fable 5.2 conditions, the depth L rule (design runs on Opus 5.5 at depth L), and the worker rule — a worker at depth S or M runs Sonnet 5.5 (raise effort, not model) unless a Sonnet worker already failed once on that ticket. `fleet-settings.py` enforces the worker rule mechanically: it refuses a model that is not a Sonnet or Haiku id or alias for `--role worker` unless `--depth L` or `--escalated` is passed ([Fleet Check, Gate and Settings Scripts](/openwiki/operations/fleet-checks-gate-and-settings.md)).
- **Parallelism.** Independent tasks go to subagents or sessions in parallel (several `Agent` calls in one message); tasks run in sequence only when one needs another's result.

## When the advisor is called

`CLAUDE.md` lists the triggers: ticket depth L or an important ticket (design or plan review before implementing); the same check failing twice after a fix; an agent reporting `BLOCKED` or low confidence; a worker session that reports blocked or stops answering; a decision that would become an ADR with `needs_review`; and a `pd-shape` session reaching a deep design tree. The default advisor is the Opus 5.5 `advisor` agent. Fable 5.2 is used only when Opus advice did not unblock the work, for a high-stakes architecture decision, or when the owner asks; in a fleet run it is a separate session in its own Pod Bay workspace (`pd-advise`) that researches and returns a solution, and the Opus orchestrator still decides. The advisor prompt notes that the caller, not the advisor, chooses its model.
