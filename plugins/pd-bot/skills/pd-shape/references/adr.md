# Repo-level decisions as ADRs

An ADR records a decision for the whole repository: its direction, how it works, or what users see.
A brief records the decisions of one feature. Keep them apart.

## The test

Propose an ADR only when the decision meets all 3:

1. It is hard to reverse.
2. It affects the architecture, the data, security or what users see beyond this feature.
3. It had two or more real options.

## When the owner says yes

1. Put each accepted decision in a decisions file
   `{"run": "<slug>", "decisions": [...]}` with `stage: grilling`, `adr: true`,
   `owner_confirmed: true`, `decided_by: owner` and the fields of
   `${CLAUDE_PLUGIN_ROOT}/skills/pd-decision-record/SKILL.md` ("How an agent returns it").
2. Branch from the fetched default branch, not from the current HEAD. Use a separate worktree:
   `git fetch origin <default>`, then
   `git worktree add -b adr/<slug> <scratch path>/adr-<slug> origin/<default>`. Number the ADR there:
   `python3 ${CLAUDE_PLUGIN_ROOT}/scripts/write_adrs.py --input <file> --adr-dir <worktree>/docs/adr`
   (run from the worktree). Exit 2 means bad input: fix the file.
3. In the worktree: `git add docs/adr`, `git commit`, `git push -u origin adr/<slug>`, then
   `gh pr create` into the default branch. Never push to the default branch. Then remove the worktree
   (`git worktree remove <path>`); your own branch and working tree stay as they were.
4. Add the ADR paths to the brief's `ADRs` section. The owner merges the pull request.
