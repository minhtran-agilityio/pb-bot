# Wiki: first version

Part of `pd-bot:pd-wiki`. The rules of `SKILL.md` apply to every step.

1. Stop if `docs/wiki/index.md` exists: use `update`.
2. Check that git does not ignore the folder: `git check-ignore -q docs/wiki/index.md` must exit 1.
   When it exits 0, show the owner the `.gitignore` line to add (for example `!/docs/wiki/`) and
   wait for a yes.
3. Find the sources: `git ls-files`, then read `CLAUDE.md`, `AGENTS.md`, `README.md`, the ADR
   folder, the design docs, and the entry points of the code. List the gitignored folders too, so
   you can say on `architecture.md` what another machine does not have.
4. Choose the pages. Start from this set and drop a page that would be empty:
   `architecture.md`, `hard-rules.md`, `workflow.md`, `decisions.md` (an index of the ADRs: for each one a quote of the
   decision, a link to the file and what it changes; plus owner decisions that live in other
   tracked files), `glossary.md`, `known-gaps.md`,
   and one page per product area with business logic.
5. Write each page from `${CLAUDE_PLUGIN_ROOT}/skills/pd-wiki/templates/page.md`, then `index.md`,
   `log.md` and `golden.json` from the same folder.
6. Write `golden.json`: 10 to 20 questions that a new agent would ask, each with the page that
   answers it and the exact words the page must hold.
7. Run the check (`SKILL.md`, check). Correct every FAIL line. Never weaken a question to get a pass.
8. Propose one line for the Knowledge section of `CLAUDE.md` and write it after the owner agrees:
   "`docs/wiki/` is the project wiki. Read `docs/wiki/index.md`, then only the page that fits your
   task." Do not import the pages with `@path`: an import loads in every session.
9. Commit on a branch `docs/wiki-<date>` and open a pull request. The owner merges it.
