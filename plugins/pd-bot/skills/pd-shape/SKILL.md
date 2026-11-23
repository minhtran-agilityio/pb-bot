---
name: pd-shape
description: "Grill and brainstorm with the owner, in one loop, until both share one understanding of a prototype, an idea or a ticket; then write a feature brief. Use after the owner finalizes a prototype, for a GRILL ticket, or when the approach is not clear."
argument-hint: "<prototype file | #issue | issue URL | file path | free text>"
allowed-tools: Read(./**), Read(~/.claude/plugins/**), Grep(./**), Glob(./**), Edit(./specs/**), Agent, AskUserQuestion, Bash(gh issue view *)
---

# Shape: one understanding before any code

A prototype does not show rules, data, edge cases or scope. Find what is missing and ask the owner.
Act on nothing until the owner confirms the brief. The caller names `Mode: interactive` (default) or
`Mode: auto`.

## 1. Read

- `docs/solutions/` first (`docs/solutions/INDEX.md`): a solved problem can settle a decision.
- A prototype: the file the owner names (for example `prototypes/<slug>.html`), or the file that a
  `Shape:` issue links.
- A ticket: `gh issue view <n> --comments` and the code it names.
- Look up facts yourself (`pd-bot:researcher` agent for a large search). Never ask the owner for a fact.

## 2. Map the decisions

Map the open decisions as a tree, each below the decision it depends on. A **what** decision is a
requirement that is not clear. A **how** decision has two or more viable approaches. For a prototype, also Read `references/prototype-topics.md`: each topic that the prototype and the
text do not settle gives decisions. The frontier is every decision whose prerequisites are settled.
When the request spans parts that share no files or data, split it into separate designs and shape
each one on its own.

## 3. Ask the frontier

Read `references/questions.md` for the question format.

Big decisions (hard to reverse, or your confidence is low; at most 3) and every how-decision follow
`references/owner-first.md`: the owner answers first.

Unanswered after two rounds: record the recommended answer with `needs_review: true`. A deep tree
(about 3 levels or 10 open decisions): call the `pd-bot:advisor` agent.

## 4. Repo-level decisions

When a decision meets all 3 conditions of `references/adr.md`, ask the owner: "Make this an ADR?"
Each yes follows that file. Every other decision stays in the brief.

This skill pre-allows only `gh issue view` among shell commands, reads inside the repo and the
installed plugins, and writes under `specs/`. Every git, `gh issue comment`, `gh pr create` and
`write_adrs.py` call shows the owner a permission prompt, one per command; so does any other read
or write (issue #123).
<!-- pd-bot:owner-approves git; gh issue comment; gh pr create; python3 ${CLAUDE_PLUGIN_ROOT}/scripts/write_adrs.py -->

## 5. Output

1. Depth M and L: write `specs/<slug>/design.md` from `references/design-template.md` (local scratch, in
   interactive and in auto mode). At depth L also record the design as one decision with
   `stage: design` and `adr: true`; `pd-bot:pd-decision-record` writes the ADR.
2. Write `specs/<slug>/brief.md` (local scratch, not committed) from `references/brief-template.md`.
   The slug rule is in `${CLAUDE_PLUGIN_ROOT}/templates/specs/intent.md`.
3. Ask: "Does this brief say what you mean?" Fix it until the owner says yes. That yes is the stop.
4. A ticket: also post the decisions as ONE comment titled "Confirmed decisions"
   (`gh issue comment <n> --body-file <file>`), and update `specs/<slug>/intent.md` and
   `specs/<slug>/decision.md` (`${CLAUDE_PLUGIN_ROOT}/templates/specs/decision.md`).
5. Next: a prototype or idea goes to `/pd-bot:pd-tickets specs/<slug>/brief.md`;
   a ticket returns the decision list to the caller; a depth L design goes to `pd-bot:pd-xia2`, then
   `pd-bot:pd-writing-plans`.

## Auto mode

`Mode: auto` (the caller cannot answer): Read `references/auto-mode.md` and follow it. A prototype
needs an owner: in auto mode, stop and say so.
