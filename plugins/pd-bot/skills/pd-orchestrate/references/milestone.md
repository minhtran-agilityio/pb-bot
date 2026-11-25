# Milestone run

A run on one feature branch. Part of `pd-bot:pd-orchestrate`. Read it when the owner names a
milestone, or several tickets that ship together. A single ticket has no feature branch: its pull
request goes into `main` after the full review, and the owner merges it.

## Owner decisions this file follows

- Every ticket pull request goes into the feature branch `feat/<slug>`. It goes in without the
  owner only when the merge gate passes (`run.md`, step 5).
- When every ticket is in, one pull request goes from `feat/<slug>` into `main`. The owner merges it.
- Before that, the whole feature is tested with the scenarios a normal user runs. A failure becomes
  a bug ticket. All bug tickets of a round are created first, then fixed. At most 2 fix rounds.
- A fix where the worker decided a requirement that was not clear does not go in by itself: it gets
  the label `pd-bot:owner-merge`.
- Every fix pull request is named in the feature pull request, so a human sees it.
- The test scenarios live on one artifact page. Only a session that writes no milestone code edits
  it, and each edit states its reason. Sessions in other workspaces do not edit the page.
- Tests and checks never run in your session: their output would fill your context. They run in a
  reviewer or tester session, which sends you a short result.

## 1. Before the first assign

1. Create the feature branch (`run.md`, Milestone base) and record it in the ledger as `base`.
2. Write the scenario page. You are the session that holds it: you write no milestone code. From
   the epic and the ticket texts only (no plan, no code), write the flows a normal user runs with
   the feature, across tickets: for each scenario an id (`S1`, `S2`, ...), the tickets it covers,
   the steps, and the expected result. End the page with a section `Changes` (date, who asked, what
   changed, why). Publish it with the Artifact tool and record its link in the ledger as
   `scenarios`. Without the Artifact tool, ask the owner where the scenarios go; do not put them in
   the feature branch.

## 2. During the tickets

- Run the tickets as `run.md` says, with the feature branch as their base.
- When the first ticket pull request has gone in, open the feature pull request as a draft:
  `gh pr create --draft --base main --head feat/<slug>` (before that the branch has no commit that
  `main` lacks, and GitHub refuses it). The body names the scenario page and has the sections
  `Tickets`, `Fixes from the feature test` and `Still failing`. Record its number in the ledger as
  `feature_pr`. Add each ticket pull request to `Tickets` when it goes in.
- A worker or reviewer who finds a scenario wrong or missing sends you a `question` with the change
  and the reason. You decide, edit the page, and add a line to `Changes`. Never let a session that
  writes code edit the page.

## 3. Whole-feature review

Start it when every ticket of the milestone is in the feature branch, before the feature test, so
its fixes are tested too.

Start a fresh reviewer session on Opus 5.5 in a workspace of its own, with a checkout of `feat/<slug>` at its
tip. It follows `pd-bot:pd-review` for the range from the base `main` to the tip: `/code-review` on
the feature pull request, the `pd-bot:intent-reviewer` agent against every ticket's verbatim text
and the owner's answers, the full suite on the tip. It sends you a receipt. Each real finding
becomes a ticket of its own, fixed as in section 4, step 4. Then wait for CI on the tip.

## 4. The feature test

1. Start a tester session in a workspace of its own (`podbay.md`, Start a session for a role) with
   a checkout of `feat/<slug>` at its tip. It saw no plan, no diff and no report, and it writes no
   code. Send it `feature-test-request` with the scenario page, the ticket texts and the owner's
   answers. Its brief: "Run every scenario on this checkout and do not stop at the first failure.
   Follow `pd-bot:pd-uat` when the feature has a running app, else run the `pd-bot:qa-verifier`
   agent. When the Playwright MCP tools are missing, send a `question` before you use another way.
   Keep the logs in your session. Send `feature-test-result`: one JSON object
   `{"round": <n>, "scenarios": [{"id": "S1", "status": "pass | fail | not_run", "evidence": "one line"}]}`."
   Use the same tester session for every round. A scenario that could not run is `not_run`, never
   a pass.
2. Sort each failure yourself, from the one line of evidence (ask the tester for more only when
   that line cannot decide it):
   - **bug**: the code does not do what the scenario and the ticket ask;
   - **scenario wrong**: the scenario asks for something no ticket asked; fix the page, with the
     reason in `Changes`;
   - **requirement not clear**: the ticket does not decide it.
3. Create every bug ticket of the round before any fix starts: one issue per bug, with the
   scenario id, the steps, the expected and the actual result, and a link to the feature pull
   request. A `requirement not clear` failure is a bug ticket too, marked so in its text.
4. Fix them as ordinary bug tickets (`run.md`): a failing repro test first, a reviewer, the checks,
   a pull request into `feat/<slug>`. Before the gate, put the label `pd-bot:owner-merge` on the
   pull request of a `requirement not clear` ticket: the worker decides the requirement and fixes
   it, the owner merges it. Add every fix pull request to `Fixes from the feature test`.
5. Send the tester a new `feature-test-request` for the new tip. It runs every scenario again, not
   only the failed ones, so a fix that broke another flow shows.

The rounds: test 1, fix round 1, test 2, fix round 2, test 3. Test 3 is the last: create a bug
ticket for each failure it finds (step 3), do not fix it, and list it under `Still failing` with
its ticket. A pull request that waits for the owner because of `pd-bot:owner-merge` is not in the
branch yet: list its scenarios under `Still failing` too.

Record each round in the ledger: `test_rounds: [{"round": 1, "failed": ["S2"], "tickets": [51]}]`.

## 5. Hand the feature to the owner

1. Write the results into the feature pull request: CI on the tip, the whole-feature review, each
   test round, `Still failing`, the open `needs_review` decisions, and the pull requests that wait
   for the owner because of `pd-bot:owner-merge`.
2. Mark it ready (`gh pr ready <n>`). The owner merges it into `main`. You never do.
