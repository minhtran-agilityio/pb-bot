# Spike results, 2026-11-22

**Verdict: pass.** 3 runs in a row each gave valid `p2g-questions` JSON on every question turn and
ended with `specs/shape-board/brief.md`. `pd-shape` needed no change, so the `Mode: app` fallback
is not needed.

Setup: Linux, Claude Code 2.1.330, pd-bot plugin 0.6.0 (installed; its `pd-shape` and `pd-tickets`
are the same as `main` @ e344cfe), model `claude-sonnet-5-5` at effort `medium`. The prototype was
the Shape Board prototype (`shape-board-prototype.html`, 267 KB, one esbuild bundle). Each run used
its own throwaway clone of pb-bot outside every Pod Bay workspace. The runs ran one after another.

The 3 `pd-shape` runs and the first `pd-tickets` check ran before review round 1, with the flag set
without `--permission-mode` (their init event reports `permissionMode: default`). After review, the
flag set pins `--permission-mode dontAsk` (see `README.md`). The `pd-tickets` check was run again
with it. The `pd-shape` runs were not run again: their tools are the same, and `dontAsk` only removes
permissions.

## pd-shape, 3 runs

| Run | Turns | Question turns | Invalid blocks | Questions only in prose | AskUserQuestion calls | Errors | Time | Cost (USD, session total) |
|---|---|---|---|---|---|---|---|---|
| 1 | 10 | 9 | 0 | 0 | 0 | 0 | 92.5 s | 0.29 |
| 2 | 9 | 8 | 0 | 0 | 0 | 0 | 103.7 s | 0.35 |
| 3 | 10 | 9 | 0 | 0 | 0 | 0 | 97.6 s | 0.32 |

Driver output: `PASS 3/3`.

Cost: `total_cost_usd` in a `result` event is cumulative over `--resume`, so a session's cost is
the last turn's value. Round 1 summed it over the turns and reported USD 2.10, 2.37 and 2.36,
about 7 times too much. The values above are the last turn's values in the same transcripts
(run 1 per turn: 0.153, 0.166, 0.175 ... 0.292).

The stricter gates added in round 2 (block in the turn's last message, no question only in prose,
no write outside `specs/`) also hold for the recorded runs: every question turn has
`block_in_final_message: true`, no turn without a block has a `?` line, and the only write per run is
`specs/shape-board/brief.md`.

- Every question turn had exactly one block, and the block was in the last message of the turn.
  The last turn of each run had no block: the run ended after "Does this brief say what you mean?"
  was answered.
- The leak check marks a prose line that ends in `?`. It found 1 line in all 3 runs: in run 1, turn 2,
  "Do you want to confirm the choice?" was a lead-in to a valid block that asked the same question.
  No turn asked a question in prose without a block.
- The owner-first questions of `pd-shape` ("What would you choose for X, and why?", asked in plain
  text) came as blocks with neutral options and no "(Recommended)" tag, as rule 4 of the appended
  prompt says. Then came the recommendation, the counter-argument and a block with the final choice.
- The ADR question came as a block in runs 1 and 3 ("Make ... an ADR?"). The driver answered
  "No, brief only", so the git and PR part of the ADR step was not run.
- The only file written was `specs/shape-board/brief.md`. In each run the model tried `Bash` once
  (`wc -l` on the prototype) and got `No such tool available: Bash`. `--disallowedTools Bash`
  removes the tool, even though `pd-shape`'s `allowed-tools` lists Bash commands.
- The first turn takes 30 to 40 s because the model reads the skill and searches the 267 KB bundle.
  Later turns take 4 to 22 s.

## Slash commands in `-p` mode

- The `system/init` event lists both `pd-bot:pd-shape` and `pd-bot:pd-tickets` in `slash_commands`.
- `/pd-bot:pd-shape prototypes/<file>.html` as the `-p` message runs the skill (3 runs above).
- `/pd-bot:pd-tickets specs/shape-board/brief.md` runs the skill too, even though it has
  `disable-model-invocation: true`. The first check took 2 turns, 46.2 s and USD 0.21:
  1. It ran its read steps (`publish-check.sh`, `gh repo view`, `gh api user`, `gh label list`, the
     milestone GET, `gh issue list`). Then it asked 3 questions in one valid block: milestone,
     retry rule, and app location. The driver answered "No milestone".
  2. It showed the ticket table (8 tickets and 1 epic) and asked for approval in one valid block:
     "Create 9 issues on the PUBLIC repo ...?", with the options `Create 9 issues` (the table in
     `preview`) and `Stop`.
- So with `AskUserQuestion` disabled, the approval step asks through `p2g-questions` and waits. It
  did not try to create anything. The driver stopped there and never sent "go".
- GitHub after the check: `gh issue list --state all` returns 10 issues, as before. None of the 6
  labels it planned exists. No permission denial was recorded in either turn.
- The check run again with `--permission-mode dontAsk` (init event: `permissionMode: dontAsk`): 2
  turns, 46.6 s, USD 0.10, `pass=True`. Every read call ran with 0 denials. Turn 1 asked 3 questions
  in one valid block, and turn 2 asked "Create these 12 tickets and 1 epic on PUBLIC repo ...?" in one
  valid block. GitHub write attempts: 0. GitHub afterwards: 10 issues, none of the planned labels.

## For the app

- The brief that `pd-tickets` read put the app in `apps/shape-board`, because the auto-answer chose
  the first option. The owner chose `apps/pod-shape`. The app shows that question to the owner, so
  this does not happen in real use.
- The protocol text, the schema and the flags are in `README.md`.
