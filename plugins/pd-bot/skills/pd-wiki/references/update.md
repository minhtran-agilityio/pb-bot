# Wiki: update after merges

Part of `pd-bot:pd-wiki`. The rules of `SKILL.md` apply to every step.

When your prompt says that no person is present, stop after step 6 and report: the caller makes
the branch, the commit and the pull request.

1. Run the check. Each `STALE` line names a page, the page's last commit, and the source files that
   a later commit changed. Each `FAIL  quote:` line names a quote that is no longer in its source.
2. With a range argument, also list the merged work: `git log --first-parent --oneline <from>..<to>`
   and, for a subject with `(#n)`, `gh pr view <n> --json title,body`. A merged pull request body
   often holds the reason for a change; quote it only when the text is also in a tracked file.
3. For each stale or failed page: read the changed source
   (`git diff <the page's last commit>..HEAD -- <path>`), correct the page, and add or replace
   quotes. Change only what the diff changed. Set `checked: <date>` in the frontmatter of every page
   you examined, also one whose text is still true: the commit of that line makes the page current.
4. A `FAIL  adr:` line names an ADR file that no page has in `sources:`. Add it to the decisions
   page: the path in `sources:`, one quote of the decision, a link to the file, and what it changes
   for the reader. When the new ADR supersedes an old one, say so at the old entry. Do not copy the
   reason or the options: they stay in the ADR.
5. A new product area or a new hook gets its lines on the page that fits, or a new page with a row
   in `index.md` and a question in `golden.json`.
6. Add one line to `log.md`: `## [<date>] ingest | <range or PR numbers>: <pages changed>`.
7. Run the check with `--strict`. It must print `0 failed, 0 stale`.
8. Commit on a branch and open a pull request that holds only wiki files. Never put a wiki change
   into a ticket pull request, and never push to the default branch.
