# Groups: order, overlap and the parallel limit

Part of `pd-bot:pd-orchestrate` (`run.md`).

The groups run in order; the tickets of one group run at the same time. Two tickets that change
the same file at the same time end in a conflict that nobody sees until both are finished. So the
grouping needs to know what each ticket is expected to change.

## Ask for the paths

Add this line to the prompt of each intake agent:

```text
Also return one line `Touches:` with the files and directories this ticket is expected to change,
relative to the repository root, comma-separated. Name a file when you know it, else its directory.
Write `Touches: unknown` instead of a guess.
```

## Plan the groups

Pipe one object per ticket into the script (or write them to a file and pass `--input <file>`):

```json
[{"n":1,"deps":[2],"paths":["src/a.py","docs/"]},{"n":2,"deps":[]}]
```

The command is `python3 ${CLAUDE_PLUGIN_ROOT}/skills/pd-orchestrate/scripts/plan_groups.py`;
`run.md` gives the `--max-parallel` value to pass.

- `paths` is the ticket's `Touches:` list. Two paths overlap when they are equal or one is a
  directory that holds the other. Two tickets with an overlapping path never share a group: the
  script places the tickets by dependency depth, then by input order, and the one placed later
  goes to the later group.
- A path is a plain file or directory name, relative to the repository root. A pattern
  with a star, a question mark or a bracket, an absolute path and a path with `..` exit 2: name the directory instead.
- `Touches: unknown`: never pass the word `unknown` as a path (exit 2). Leave `paths` out when the
  ticket is small and clearly apart from the others. Otherwise pass `["."]`, the whole repository,
  which makes the ticket run alone.
- `--max-parallel <n>` is the most tickets one group may hold; without it a group has no limit.
- stdout is `[{"n":1,"group":2},{"n":2,"group":1}]`, in input order.
- Each stderr line names a ticket that moved to a later group and why: an overlap with another
  ticket, or a full group. Show these lines to the owner in the summary.
- Exit 2 names a dependency cycle or bad input. Correct bad input yourself and run it again; show
  a cycle to the owner and stop.
