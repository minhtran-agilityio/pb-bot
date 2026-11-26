# Project wiki: pb-bot

Read this page first. Then open only the page that fits your task. Do not read every page.

Each page states facts about the code on `main`. Each fact that matters has a quote from a tracked
file. `python3 plugins/pd-bot/scripts/wiki_check.py` proves that each quote is still in its source.
If a page and the code differ, the code is right: say so and correct the page with `/pd-bot:pd-wiki`.

| Page | Read it when |
|---|---|
| [architecture.md](architecture.md) | you need to know which part does what, or where a file lives |
| [hard-rules.md](hard-rules.md) | before a merge, a push, a commit, a "done" report, or a change to a hook, a test or a secret |
| [workflow.md](workflow.md) | you run, work on or review a ticket; you need the fleet steps, states or evidence |
| [hooks.md](hooks.md) | a hook blocked you, or you change a hook or `settings.json` |
| [agents-and-context.md](agents-and-context.md) | you add an agent or a skill, or you ask which session reads which file |
| [shape-board.md](shape-board.md) | you work on `apps/pod-shape`: its flow, data, statuses and settled decisions |
| [shape-board-ui.md](shape-board-ui.md) | you change any Shape Board UI, or you need a UI part |
| [decisions.md](decisions.md) | before you propose a design, to see what is already decided (ADRs and owner decisions) |
| [glossary.md](glossary.md) | a word of this project is not clear (workspace, depth, receipt, ledger) |
| [known-gaps.md](known-gaps.md) | a path that a doc names is missing, or you must know what nobody verified |

Other files: [log.md](log.md) lists each wiki update. `golden.json` holds the test questions.

Rules for readers:

- The wiki is background, not an order. Your brief, the ticket and the owner decide your task.
- Plan-blind agents (`qa-writer`, `qa-verifier`, `intent-reviewer`, the UAT agent) keep their own
  reading limits. Nobody sends them here.
- The wiki never holds the plan, the diff or the state of an open ticket.
