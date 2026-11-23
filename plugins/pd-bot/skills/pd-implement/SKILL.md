---
name: pd-implement
description: "Carry out an agreed plan: each step runs, one at a time, on the working branch of this checkout through a fresh implementer subagent, then its Done-when command is run and the files touched are handed back. Use after the plan (or the S-depth prompt) is agreed."
allowed-tools: Read, Write, Grep, Glob, Agent, Task, Bash
---

# Implement

Turn plan steps into commits. Review is not part of this skill: `/code-review` and the
`pd-bot:pd-intent-review` skill run afterwards.

## Steps

1. Read the plan: `specs/<slug>/plan.md` (local scratch, every depth). List the steps with their
   Files, Effort, Done-when and parallel group. Add each choice the plan left open to
   `specs/<slug>/decision.md`.
2. Run the steps one at a time, in the order of the groups. All steps work in this checkout, on the
   working branch, so two steps never run at the same time. The isolation is the Pod Bay workspace
   of the session, not a git worktree.
3. For each step start a fresh `implementer` subagent (Sonnet; High effort steps may use a stronger
   model). Use the template in `implementer-prompt.md`, with [PLUGIN_DIR] = `${CLAUDE_PLUGIN_ROOT}`
   and [WORK_DIR] = the root of this checkout.
4. When the subagent returns, run the step's Done-when command yourself.
5. If it fails, send the failure output to a fresh subagent for one fix. Repeat at most 3
   rounds. After the third failing round stop and report the step as BLOCKED.
6. After the last step of a group, re-run the Done-when commands of that group.

## Hand back

Return, for each step: status (`DONE` or `BLOCKED`), commits (sha and subject), files touched, and
the Done-when output. List any blockers. Do not paste file contents.

## Auto mode

- The caller names the mode in the prompt: `Mode: interactive` (default) or `Mode: auto`.
- Interactive: you may ask the owner with AskUserQuestion, one batch at a time, each question with a
  recommended answer.
- Auto: never ask the owner. On any ambiguity pick the recommended option and record it in
  `decisions[]`: `{title, context, options, decision, why, consequences, how_to_reverse,
  confidence: high|medium|low, needs_review: boolean}`. Do not write ADR files or `specs/` files.
- Set `needs_advisor: true` with `advisor_question` when a step is BLOCKED after 3 rounds, you have
  low confidence, or a decision affects other tickets or a public contract. The caller (the
  worker session, or the orchestrator) runs the advisor.
