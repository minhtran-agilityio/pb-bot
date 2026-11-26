---
title: Known gaps
read_when: a path that a doc names is missing, or you must know what nobody verified
sources:
  - .gitignore
  - CLAUDE.md
  - REVIEW.md
  - AGENTS.md
  - plugins/pd-bot/skills/README.md
  - plugins/pd-bot/skills/pd-intake/SKILL.md
  - plugins/pd-bot/agents/implementer.md
  - docs/adr/0003-decisions-are-tracked-again-in-docs-adr-numbered-from-0001.md
  - docs/adr/0005-pd-bot-runs-only-with-pod-bay-and-the-fleet-is-the-only-run-path.md
  - apps/pod-shape/docs/storehouse-keeper.md
checked: 2026-11-26
---

# Known gaps

Facts found on `main` at `1720e70`. Each one has the command that shows it. Correct this page when a
gap is closed.

## G1. Tracked files point at lessons that only one machine has

`docs/solutions/` is gitignored. A fresh clone has no such folder (`git ls-files docs/` lists only
`docs/adr/` and `docs/wiki/`). But 35 tracked files name it (`git grep -l 'docs/solutions' | wc -l`).

> source: CLAUDE.md
> Start at `docs/solutions/INDEX.md`; read `docs/solutions/critical-patterns.md` when you plan.

> source: REVIEW.md
> docs/solutions/scripts/bash-empty-array-and-jsonl-parsing-gotchas.md

Effect: on another machine, `pd-recall`, `pd-shape` and `pd-xia2` find no lesson, and the five
"Source" cells of `REVIEW.md` open nothing. The check rule in each `REVIEW.md` row is still complete
without its source.

## G2. Some docs cite ADR numbers of the old repository

> source: plugins/pd-bot/skills/README.md
> ### Fleet (free agent sessions, ADR 0041)

> source: plugins/pd-bot/skills/pd-intake/SKILL.md
> Do not spawn agents (`docs/adr/0001-workflow-capabilities.md`).

`docs/adr/` holds 0001 to 0006 only, and no file `0001-workflow-capabilities.md`
(`git ls-files docs/adr`). ADR 0003 says that old numbers do not resolve here.

## G3. `.claude/rules/` is named but this repository has none

> source: plugins/pd-bot/agents/implementer.md
> The project's `CLAUDE.md` and `.claude/rules/*.md` are the source of truth

`git ls-files | grep -c '^.claude/rules'` prints 0. In pb-bot the implementer has only `CLAUDE.md`.
That text is correct for a consuming project that has such rules.

## G4. `AGENTS.md` says that all of `docs/` is local

> source: AGENTS.md
> - `docs/` and `specs/` are local only and gitignored, except `docs/adr/` and `docs/wiki/`, which git tracks.

This line was corrected in the pull request that added the wiki. Before it, the line named no exception.

## G5. A fleet run cannot be read from another machine

The ledger and the evidence of a run are under `specs/fleet/`, which is gitignored. The pull request
body and the issue comments are the only trace on GitHub.

## G6. The evidence check blocks only on a feature branch

> source: docs/adr/0005-pd-bot-runs-only-with-pod-bay-and-the-fleet-is-the-only-run-path.md
> The fleet gates depend on `fleet-check.sh`, which blocks only on a `feat/*` base today; making it block on every base is a follow-up.

## G7. Mac to Linux messages can be lost

> source: apps/pod-shape/docs/storehouse-keeper.md
> If a Mac session gets no answer, it asks the owner, not a second time by message only.

## What this wiki did not verify

- No session was run to test the wiki. The check is `wiki_check.py`: it proves that each quote is in
  its source and that each golden answer is on its page. It does not prove that an agent answers better.
- Token counts are estimates (bytes divided by 4).
- What Pod Bay does inside (`podbay-dev`) is taken from `podbay.md`, not from a run.
- Shape Board source code under `src/` and `src-tauri/src/` was listed, not read line by line. The
  page [shape-board.md](shape-board.md) rests on the three docs and the migration list.
- Lessons in `docs/solutions/` and tickets in `specs/` are not in git, so nothing from them is here.
