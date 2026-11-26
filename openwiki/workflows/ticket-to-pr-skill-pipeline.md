---
type: workflow
title: Ticket-to-PR Skill Pipeline
description: The skill chain that takes one ticket from an intake readiness verdict and depth (S/M/L) through shaping, planning, step-by-step implementation, review and acceptance checks to a pull request, including the bug-fix path and the specs flow files.
tags: [skills, intake, shape, plan, implement, review, workflow, depth]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-26T15:02:34.678Z
sources:
  - id: openwiki-source-4f30989bba285253df686ebb
    resource: repo://plugins/pd-bot/skills/pd-diagnosing-bugs/SKILL.md
  - id: openwiki-source-a10137558dcc6eaaad29cb2c
    resource: repo://plugins/pd-bot/skills/pd-finishing-a-development-branch/SKILL.md
  - id: openwiki-source-9775768dd2c8a1c4ae28094d
    resource: repo://plugins/pd-bot/skills/pd-implement/SKILL.md
  - id: openwiki-source-2d26a432e1ff06f2dad01845
    resource: repo://plugins/pd-bot/skills/pd-intake/SKILL.md
  - id: openwiki-source-288222dbf1e25f42fd593c3d
    resource: repo://plugins/pd-bot/skills/pd-shape/SKILL.md
  - id: openwiki-source-a4fb76bf2a35c574dcb30014
    resource: repo://plugins/pd-bot/skills/pd-uat/SKILL.md
  - id: openwiki-source-275c6175fd29724271dab368
    resource: repo://plugins/pd-bot/skills/pd-writing-plans/SKILL.md
  - id: openwiki-source-22793ca8a34c02a7656916a5
    resource: repo://plugins/pd-bot/skills/README.md
generated: { by: "claude-code", at: "2026-11-26T15:02:34.678Z" }
---

# Ticket-to-PR Skill Pipeline

<!-- openwiki: broken internal link [/openwiki/workflows/fleet-orchestration.md] link "/openwiki/workflows/fleet-orchestration.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
<!-- openwiki: broken internal link [/openwiki/workflows/knowledge-store-adr-and-solutions.md] link "/openwiki/workflows/knowledge-store-adr-and-solutions.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
<!-- openwiki: broken internal link [/openwiki/workflows/agents-and-model-selection.md] link "/openwiki/workflows/agents-and-model-selection.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
pd-bot's skills are reusable prompt programs run as `/pd-bot:<name>`. Each has a narrow scope, hard rules and a handoff to the next skill; `plugins/pd-bot/skills/README.md` is the workflow map. The chain below is the *logical* pipeline for one ticket. At run time the [fleet](/openwiki/workflows/fleet-orchestration.md) executes it with separate sessions, and a worker chooses its own path through these skills guided by its outcome contract. The stores these skills write to are in [Knowledge Store](/openwiki/workflows/knowledge-store-adr-and-solutions.md); the subagents they dispatch are in [Subagents and Model/Effort Selection](/openwiki/workflows/agents-and-model-selection.md).

```
intake  ->  shape (M, L)  ->  xia2 (L)  ->  writing-plans  ->  qa-writer (M, L)
   ->  implement  ->  security-reviewer (risky)  ->  /code-review + intent-review
   ->  uat (app) | qa-verifier (no app, M/L)  ->  finishing-a-development-branch  ->  human merges
```

The minimum path for a clear, in-place change is intake → writing-plans (one or two steps) → implement → finishing. Every change runs on its own branch.

## 1. Intake: the readiness gate (`pd-intake`)

Run first on every ticket. It reads the ticket (`gh issue view <n> --comments`, or free text), looks up facts itself rather than asking the owner for them, and writes only `specs/<slug>/intent.md`; it does not plan or implement. It returns a block with the verbatim intent, slug, verdict, depth, dependencies, questions and a reason.

**Verdict.**

- `READY`: the goal fits one sentence, each acceptance criterion is checkable by a command or visible behavior, scope and out-of-scope are stated, dependencies are done, no human decision is left, and a `Verify:` row exists.
- `CONFIRM`: one to three small decisions remain; each is a question with a recommended answer worded so that "yes" accepts it. A missing `Verify:` row is one of them (a drafted row is the recommended answer).
- `GRILL`: the ticket is important or its design tree is deep (auth or security, migration or deletion, a public contract, money or quota, irreversible actions, cross-module architecture, or label `grill`); it hands off to `pd-shape`.
- `BLOCKED`: too large (propose a split) or a dependency is missing (name it).

**Depth** decides the route: **S** → writing-plans (one or two steps) then implement; **M** → shape (design) first; **L** → shape, then xia2 research, then the same. A bug fix goes to `pd-diagnosing-bugs` first. In `Mode: auto` (callers that cannot answer, such as the orchestrator or a worker) intake never asks the owner: it takes each recommended answer and records it in `decisions[]`, and can set `needs_advisor`.

## 2. Shape and research (`pd-shape`, `pd-xia2`)

`pd-shape` runs one loop of grilling and brainstorming until the owner and agent share one understanding, and acts on nothing until the owner confirms. It reads `docs/solutions/INDEX.md` first, maps open decisions as a tree (a *what* decision is an unclear requirement; a *how* decision has two or more viable approaches), asks the frontier of decisions, and lets the owner answer first on big decisions (at most 3). A decision that is hard to reverse, affects the repo and has two or more real options may become an ADR, asked as "Make this an ADR?". Output: `specs/<slug>/brief.md` (scratch) once the owner says the brief says what they mean; at depth M and L also `specs/<slug>/design.md`, and at depth L the design as a decision with `stage: design` and `adr: true`. For a ticket it posts one "Confirmed decisions" comment after showing the owner its text. A prototype goes on to `/pd-bot:pd-tickets`; a depth L design goes to `pd-xia2`, which researches how things actually work locally (and externally only if the change has an external surface) and produces a research brief. A deep tree (about 3 levels or 10 open decisions) calls the advisor. A prototype needs an owner, so auto mode refuses it.

## 3. Planning (`pd-writing-plans`)

At every depth it writes `specs/<slug>/plan.md` from a template; at depth S it also writes a short `design.md` (shape writes it at M and L). Each step must be small enough for one fresh subagent and carries **Files**, **Covers** (acceptance criteria IDs from `intent.md`), **Effort** (`low|medium|high` with a reason), **Done-when** (a command and its expected result — "looks right" does not count) and a **Parallel group** letter (steps touching different files share a group; steps touching the same file go in separate, ordered groups). Checks come first: for each behavior, the command that proves it.

## 4. Blind acceptance tests (`qa-writer`)

<!-- openwiki: broken internal link [/openwiki/workflows/fleet-orchestration.md] link "/openwiki/workflows/fleet-orchestration.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
At depth M and L the orchestrator has the `qa-writer` agent write acceptance tests from the ticket, its criteria and the public interface only, before any code exists, committed as `Add acceptance tests for #<n>`. The implementer cannot change them, which makes them the honest measure of the work ([Fleet Orchestration](/openwiki/workflows/fleet-orchestration.md), outcome contract).

## 5. Implementation (`pd-implement`)

It reads the plan and runs the steps **one at a time**, in group order, all in the session's checkout on the working branch (isolation is the Pod Bay workspace, not a git worktree, so steps never run in parallel). Each step gets a fresh `implementer` subagent. The skill itself then runs the step's Done-when command; on failure it sends the output to a fresh subagent for one fix, at most three rounds, after which the step is reported `BLOCKED`. After the last step of a group it re-runs that group's Done-when commands. It returns, per step, status, commits, files touched and Done-when output. Review is not part of this skill.

## 6. Review and acceptance

After implementation: the `security-reviewer` agent for risky tickets, `/code-review`, and `pd-intent-review` (the `intent-reviewer` agent against the owner's verbatim request, blind to the plan, reporting gap, drift or excess). For tickets with a running app, `pd-uat` checks the app from only the issue body and "Confirmed decisions" comment: it numbers acceptance bullets AC1..ACn, writes 3 to 8 scenarios, and uses Playwright MCP tools (not npm Playwright), with pass or fail decided by assertions and never a screenshot; it reports BLOCKED, not FAIL, when the app will not start or credentials are missing. `pd-verify` handles tickets with a `Verify:` row. For tickets with no app at depth M and L, the `qa-verifier` agent runs commands and reads delivered files. In the fleet, the reviewer session does these.

## 7. Finishing (`pd-finishing-a-development-branch`)

It finds the branch and base, runs the repository's test command (from `CLAUDE.md` → `## Run and test`), retries at most twice and never pushes a failing branch, pushes without force, opens one PR with `gh pr create`, and writes `specs/<slug>/summary.md` with real command output. A UI change needs before/after screenshots on the PR. The PR body has the fixed sections Summary, Why, Changes, Decisions, Testing, Risk, How to revert and Notes. It never merges, force-pushes, uses `git add -A`, runs `git clean`, discards work or amends commits; a human merges. In the fleet the orchestrator, not the worker, pushes and opens the PR using the same body sections.

## The bug-fix path (`pd-diagnosing-bugs`)

<!-- openwiki: broken internal link [/openwiki/operations/fleet-checks-gate-and-settings.md] link "/openwiki/operations/fleet-checks-gate-and-settings.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
Intake routes an `Input-type: bug fix` here at every depth; the skill is user-only (it blocks model invocation, so intake Reads its `SKILL.md`). Before Phase 1 it creates `fix/<slug>`. Six gated phases prove the root cause before any fix: (1) a fast deterministic **feedback loop** that goes red, (2) **reproduce and minimise**, (3) three to five ranked **hypotheses** each with a prediction (diagnosis.md committed before the first probe), (4) **instrument** one variable per probe with tagged `[DEBUG-xxxx]` logs that are removed once a hypothesis is confirmed — all written to `specs/<slug>/diagnosis.md`. The `task-reviewer` agent must return `spec_verdict: pass` and `quality_verdict: approved` before Phase 5. Phase 5 writes a red test at the diagnosis's `Seam:` and commits it before the fix; phase 6 cleans up. Depth M and L go through writing-plans and implement with `diagnosis.md` as input. This realizes the hard rule "a bug needs a failing repro test first"; the fleet's evidence check verifies it by running the test on base and head ([Fleet Check, Gate and Settings Scripts](/openwiki/operations/fleet-checks-gate-and-settings.md)).

## Cross-cutting conventions

- **Auto vs interactive mode.** Most skills take `Mode: interactive` (default) or `Mode: auto`. In auto mode a skill never asks the owner, picks the recommended option, returns `decisions[]` (title, context, options, decision, why, consequences, how_to_reverse, confidence, needs_review) instead of writing ADR or `specs/` files, and may set `needs_advisor` with a question for its caller to run.
- **`specs/<slug>/` flow files.** Five files at every depth: `intent.md`, `design.md`, `decision.md`, `plan.md`, `summary.md`, sharing a `Ticket · Depth · Date` line and linked by IDs (`AC<n>`, `D<n>`).
<!-- openwiki: broken internal link [/openwiki/testing/test-suite-and-ci.md] link "/openwiki/testing/test-suite-and-ci.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
- **Permissions by skill.** Each skill's `allowed-tools` is deliberately narrow (for example `pd-intake` pre-allows only `gh issue view`, `ls` and `test -n`; `pd-shape` allows `Edit` only under `specs/`), and a lint rejects wide grants ([Test Suite, Lints and CI](/openwiki/testing/test-suite-and-ci.md)).
<!-- openwiki: broken internal link [/openwiki/operations/release-and-plugin-packaging.md] link "/openwiki/operations/release-and-plugin-packaging.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
- **Handoffs.** `pd-tickets` turns a brief into GitHub issues and an optional milestone for `pd-orchestrate`; `pd-compound` and `pd-session-retro` capture lessons; `pd-release-notes` and `pd-release-summary` draft release text ([Release and Plugin Packaging](/openwiki/operations/release-and-plugin-packaging.md)).
