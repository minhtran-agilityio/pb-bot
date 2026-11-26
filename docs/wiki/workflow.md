---
title: Workflow
read_when: you run, work on or review a ticket; you need the fleet steps, states or evidence
sources:
  - CLAUDE.md
  - plugins/pd-bot/skills/README.md
  - plugins/pd-bot/skills/pd-intake/SKILL.md
  - plugins/pd-bot/skills/pd-orchestrate/SKILL.md
  - plugins/pd-bot/skills/pd-orchestrate/references/protocol.md
  - plugins/pd-bot/skills/pd-orchestrate/references/run.md
  - plugins/pd-bot/skills/pd-orchestrate/references/stuck.md
  - plugins/pd-bot/skills/pd-orchestrate/references/milestone.md
  - plugins/pd-bot/skills/pd-work/SKILL.md
  - plugins/pd-bot/skills/pd-review/SKILL.md
---

# Workflow

## 1. Intake: is the ticket ready, and how deep is it

`/pd-bot:pd-intake` runs first on every ticket. It returns a verdict and a depth.

| Verdict | Meaning | Next |
|---|---|---|
| READY | goal, checkable criteria, scope and a `Verify:` row are there | plan |
| CONFIRM | 1 to 3 small decisions remain | ask, each with a recommended answer |
| GRILL | important, or a deep design tree (auth, data deletion, public contract, money) | `pd-shape` |
| BLOCKED | too large, or a dependency is missing | split or wait |

> source: plugins/pd-bot/skills/pd-intake/SKILL.md
> a `Verify:` row exists (a line starting `Verify:` anywhere in the ticket body or in a Confirmed decisions comment; no format check).

| Depth | Path |
|---|---|
| S | `pd-writing-plans` (one or two steps), then `pd-implement` |
| M | `pd-shape` (design) first |
| L | `pd-shape`, then `pd-xia2` (research), then the same; design on Opus 5.5 and an advisor review |

A bug fix goes to `pd-diagnosing-bugs` before any plan.

## 2. The stages of one ticket

> source: CLAUDE.md
> -> finishing-a-development-branch   push and open a PR; a human merges

Order: intake, shape (M, L), xia2 (L), writing-plans, qa-writer (M, L: blind acceptance tests before
code), implement, security-reviewer (risky tickets), `/code-review` and intent-review, uat (app
tickets) or qa-verifier (no app, M and L), then push and a pull request.

In a fleet run this order is not fixed. Only the end state is checked:

> source: plugins/pd-bot/skills/pd-orchestrate/references/protocol.md
> What the end state of a ticket must show. The order of work is free.

## 3. A fleet run (`/pd-bot:pd-orchestrate`)

1. Triage: one agent per ticket follows intake. The orchestrator asks the owner every open question in
   one batch and records the answers word for word in the ledger and on the issue.
2. Grouping: `plan_groups.py` puts tickets into groups. Two tickets that touch the same path never
   share a group.
3. For a feature at depth M or L, the orchestrator has `qa-writer` commit the acceptance tests first.
4. Assign: one worker session per ticket, in its own workspace, with a brief.
5. The worker commits, then sends `review-request` to a reviewer in another session.
6. The reviewer sends a receipt (`pass` or `changes`) for one head SHA.
7. The orchestrator sends `check-request`; the reviewer runs the acceptance check and `fleet-check.sh`.
8. The orchestrator pushes the branch and opens the pull request, then waits for CI.
9. The owner merges into `main`. Into a `feat/*` branch, the orchestrator merges after `fleet-gate.sh` passes.

> source: plugins/pd-bot/skills/pd-orchestrate/references/run.md
> State the ticket and the outcome, not steps. Do not tell the worker how you will check it.

> source: plugins/pd-bot/skills/pd-orchestrate/references/run.md
> Do not run the checks yourself: their output would fill your context.

Defaults (the orchestrator may leave one and must write the reason in the ledger):

| Decision | Default |
|---|---|
| Workers at once | 3 tickets (`--max-parallel 3`) |
| Budget | 60 minutes per ticket |
| Review rounds | 3, then `escalate` |
| Base | `main` for one ticket; `feat/<slug>` for a milestone |
| Reviewer | a different session: Sonnet 5.5 at depth S and M, Opus 5.5 at depth L or on a risky ticket |

> source: plugins/pd-bot/skills/pd-orchestrate/references/run.md
> | Workers at once | 3 tickets (`--max-parallel 3`); reviewer and advisor sessions do not count |

Ticket states in the ledger:

> source: plugins/pd-bot/skills/pd-orchestrate/references/protocol.md
> States: `queued`, `assigned`, `in_review`, `fixing`, `escalated`, `verified`, `pr_open`, `done`, `blocked`, `skipped`.

## 4. The evidence each ticket must show

| Ticket | Evidence |
|---|---|
| every ticket | a `pass` receipt for the head SHA with intent `match`; the full suite green; decisions recorded |
| bug | a test that fails on the base and passes on the head |
| feature, depth M or L | each acceptance criterion maps to a test; the tests come from `qa-writer`, before code |
| app ticket (FE or BE) | UAT on the running app by an agent that saw only the ticket and the owner's answers |
| UI change | before and after screenshots of each changed screen on the pull request (after only, when the screen is new). The worker makes them; a pull request with a UI change and none does not open |
| no app, depth M or L | `qa-verifier` rows |
| risky | a security review with no open critical or high finding |
| depth L | a design note and an advisor review before the first code commit |

> source: plugins/pd-bot/skills/pd-orchestrate/references/protocol.md
> A ticket is risky when its text names login, sessions, permissions, payments, secrets, personal data or bulk or irreversible deletion

A changed path under `hooks/`, `scripts/`, `migrations/`, `auth/`, `security/`, `secrets/` or
`.github/` also makes a ticket risky. So do `settings.json`, `REVIEW.md`, a `.sql` file and an `.env` file.

`fleet-check.sh` runs in warn mode. It blocks only when the base is a `feat/*` branch.

## 5. When something is stuck

Stuck means: the same check failed twice after a fix, 3 review rounds are used, an agent sent
`escalate`, a session is silent past its budget, or two approaches failed.

> source: plugins/pd-bot/skills/pd-orchestrate/references/stuck.md
> Climb one step at a time:

1. Think, and correct the brief.
2. One question to the `pd-bot:advisor` agent (Opus 5.5, read-only).
3. A `pd-advise` session on Fable 5.2 in a new workspace.
4. The owner.

> source: plugins/pd-bot/skills/pd-orchestrate/references/stuck.md
> Only an advisor or the owner may accept an open finding.

## 6. A milestone

Every ticket pull request goes into `feat/<slug>`. Then one pull request goes from `feat/<slug>` into
`main`, and the owner merges it. Before that, a tester session runs the user scenarios on the tip.

> source: plugins/pd-bot/skills/pd-orchestrate/references/milestone.md
> The rounds: test 1, fix round 1, test 2, fix round 2, test 3. Test 3 is the last

A fix where the worker decided an unclear requirement gets the label `pd-bot:owner-merge`; the merge
gate never passes it.

## 7. Where a run keeps its state

Only on the orchestrator's machine, in gitignored files: `specs/fleet/<run>/ledger.json` and
`specs/fleet/<run>/<ticket>/` (receipt, report, check results). Another machine does not see a run.
The orchestrator also copies the ledger into the git directory of its checkout (`ledger-backup.py`),
so `git clean` does not lose it; that copy is never committed.

> source: plugins/pd-bot/skills/pd-orchestrate/SKILL.md
> `specs/fleet/<run>/ledger.json` is your memory. Write it at every state change.
