# Review order: keep your context small

Every file you read and every command output stays in your context, and `/code-review` runs as a
fork of your session, so its cost grows with your context. tokscope, 2026-11-20 to 25: reviewer
sessions ran it at about 254k context per request (857 requests, about $80 at list price).

1. Right after you fetch the head, before any test run or other read, run `/code-review` on the
   range. Level: `low` at depth S, `medium` at depth M; higher only at depth L or on a risky ticket.
2. Run test commands (the suite, the bug test on the base and the head) through the
   `pd-bot:test-runner` agent: give it the worktree path and the exact commands, and keep only its
   result (command, exit code, pass and fail counts, failing test names). Its run is a run you
   started, so its output counts as your own evidence.
3. Then the other agents (`pd-bot:intent-reviewer`, `pd-bot:security-reviewer`) and your own reads.
