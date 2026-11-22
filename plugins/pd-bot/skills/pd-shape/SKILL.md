---
name: pd-shape
description: "Grill and brainstorm with the owner, in one loop, until both share one understanding of a prototype, an idea or a ticket; then write a feature brief. Use after the owner finalizes a prototype, for a GRILL ticket, or when the approach is not clear."
argument-hint: "<prototype file | #issue | issue URL | file path | free text>"
allowed-tools: Read, Write, Edit, Grep, Glob, Agent, AskUserQuestion, Bash(gh issue view *), Bash(gh issue comment *), Bash(git log *), Bash(git switch *), Bash(git add docs/adr*), Bash(git commit *), Bash(git push *), Bash(gh pr create *), Bash(python3 ${CLAUDE_PLUGIN_ROOT}/scripts/write_adrs.py *)
---

# Shape: one understanding before any code

A prototype does not show rules, data, edge cases or scope. Find what is missing and ask the owner.
Act on nothing until the owner confirms the brief. The caller names `Mode: interactive` (default) or
`Mode: auto`.

## 1. Read

- A prototype: the file the owner names (for example `prototypes/<slug>.html`), or the file that a
  `Shape:` issue links.
- A ticket: `gh issue view <n> --comments` and the code it names.
- Look up facts yourself (`pd-bot:researcher` agent for a large search). Never ask the owner for a fact.

## 2. Map the decisions

Map the open decisions as a tree, each below the decision it depends on. A **what** decision is a
requirement that is not clear. A **how** decision has two or more viable approaches. For a prototype, also Read `references/prototype-topics.md`: each topic that the prototype and the
text do not settle gives decisions. The frontier is every decision whose prerequisites are settled.

## 3. Ask the frontier

- ONE decision per question, at most 25 words; `header` is the slug plus a word.
- 2 to 4 options. Your recommendation first, its label ending in `(Recommended)`, worded so that
  "yes" accepts it. At most 4 questions per `AskUserQuestion` call; the most blocking first.
- Evidence goes in the option `preview`: `references/review-layout.md`.

Big decisions (hard to reverse, or your confidence is low; at most 3) and every how-decision follow
`references/owner-first.md`: the owner answers first.

Unanswered after two rounds: record the recommended answer with `needs_review: true`. A deep tree
(about 3 levels or 10 open decisions): call the `pd-bot:advisor` agent.

## 4. Repo-level decisions

When a decision meets all 3 conditions of `references/adr.md`, ask the owner: "Make this an ADR?"
Each yes follows that file. Every other decision stays in the brief.

## 5. Output

1. Write `specs/<slug>/brief.md` (local scratch, not committed) from `references/brief-template.md`.
   The slug rule is in `${CLAUDE_PLUGIN_ROOT}/templates/specs/intent.md`.
2. Ask: "Does this brief say what you mean?" Fix it until the owner says yes. That yes is the stop.
3. A ticket: also post the decisions as ONE comment titled "Confirmed decisions"
   (`gh issue comment <n> --body-file <file>`), and update `specs/<slug>/intent.md` and
   `specs/<slug>/decision.md` (`${CLAUDE_PLUGIN_ROOT}/templates/specs/decision.md`).
4. Next: a prototype or idea goes to `/pd-bot:pd-tickets specs/<slug>/brief.md`;
   a ticket returns the decision list to the caller; a depth L design goes to `pd-bot:pd-xia2`, then
   `pd-bot:pd-writing-plans`.

## Auto mode

For `--no-ask` runs and the design stage of the autopilot workflow. A prototype needs an owner: in
auto mode, stop and say so.

- Never ask. Return each recommended answer in `decisions[]` (`{title, context, options, decision,
  why, consequences, how_to_reverse, confidence, needs_review: true}`).
- Design stage: return the design in the shape of `references/design-template.md`, as one decision
  with `stage: design`.
- Write no ADR file, no `specs/` file and no comment unless the caller asks.
- Set `needs_advisor: true` with `advisor_question` when you are BLOCKED, have low confidence, or a
  decision affects other tickets or a public contract. Do not spawn agents
  (`docs/adr/0001-workflow-capabilities.md`).
