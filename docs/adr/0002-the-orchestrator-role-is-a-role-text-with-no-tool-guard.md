---
ticket: P3
stage: design
status: accepted
confidence: high
decided_by: owner
date: 2026-11-20
---

# 0002. The orchestrator role is a role text, with no tool guard

## Context

The role can be suggested (context text) or enforced (a hook or a permission rule that limits tools for that session). The `pd-bot:pd-orchestrate` skill has the orchestrator write its ledger, create ticket branches, start in-process agents, run the evidence check, push and open pull requests, so a rule such as no Write and no Bash would break it. `plugins/pd-bot/hooks/pr-opened.sh` tells the main session to run /code-review after `gh pr create`, which contradicts the fleet rule that the orchestrator does not review alone.

## Options

- Role text only (chosen)
- Role text plus a PreToolUse guard that asks before the main thread edits a tracked file
- A permissions.deny list in the local settings of the main workspace
- Run the session as an agent definition with a tool list

## Decision

The hook injects the role text and nothing else; there is no PreToolUse guard. The text says: check, coordinate and decide; delegate hands-on work (code edits, long test runs, reviews, research sweeps) to Pod Bay worker sessions; light checks, branches, a push and opening a pull request are allowed; a merge happens only when the owner asks, through the confirmation prompt of the merge guard. `pr-opened.sh` asks `session-role.sh --role`: in the orchestrator session its reminder says to hand the /code-review to another session; every other session keeps the old reminder.

## Why

A guard that is strict enough to matter would block work the orchestrator skill needs, and a Bash classifier is not reliable. The owner prefers to see how the role text behaves in real runs before any enforcement is added.

## Consequences

Nothing stops an orchestrator session that edits code anyway; the owner sees it in the session and in the diff. The role text must stay short and exact, because it is the only control. A later guard needs a new decision.

## How to reverse

Add a PreToolUse hook on Edit, Write and NotebookEdit that calls `session-role.sh --role`, and record that as a new ADR that supersedes this one.
