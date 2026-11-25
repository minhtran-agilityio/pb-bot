# Running the tickets

Defaults, the brief, what to do when a ticket reports, and the final report. Part of
`pd-bot:pd-orchestrate`.

## Before you assign

- **Read the tickets.** For a milestone run, list its open issues with
  `gh issue list --repo <repo> --milestone '<name>' --state open --json number,title,labels`
  (`'`→`'\''`). Never put the name in double quotes: the shell would expand it.
- **Triage in parallel, ask once.** One agent per ticket follows `pd-bot:pd-intake` and returns the
  verdict, the depth and its questions; it does not ask the owner. Add the `Touches:` request of
  `${CLAUDE_PLUGIN_ROOT}/skills/pd-orchestrate/references/groups.md` to its prompt, so it also
  returns the paths it expects the ticket to change. You collect every CONFIRM
  question and ask them in one batch, then run `pd-bot:pd-shape` for each GRILL ticket. Record the
  answers verbatim in the ledger and as a "Confirmed decisions" comment on the issue.
- **Classify.** Write into the ledger for each ticket: `kind` (bug, feature, docs), `depth`, `app`
  (`fe`, `be` or `none`: is there a running app to test?), `risky` (protocol, last paragraph),
  `deps` and `touches` (the `Touches:` paths; for `unknown`, look in the code yourself, or write
  `["."]`, which makes the ticket run alone). A depth the owner set wins over intake: a label `depth:S`, `depth:M` or `depth:L`, or a
  line `Depth: <S|M|L>` in the ticket or in the owner's answers. When intake says S or M but the
  ticket adds a public contract, spans modules or leaves design choices open, put "is this depth L?"
  in the triage batch instead of deciding alone.
- **Acceptance tests are yours to start.** For a feature at depth M or L, create the ticket branch
  in the worker's checkout and run the `pd-bot:qa-writer` agent there before you assign: give it
  the ticket, the acceptance criteria and the public interface only. Its commit `Add acceptance tests for #<n>` is then the first commit of the branch, written by an
  agent that never saw a plan. Record that commit's SHA in the ledger: the evidence check takes it
  as `--qa-commit`. When it returns `ready: false`, decide: another kind of check, or the owner.
  If the worker later shows that a test is wrong, you may start one qa-writer repair.

## Defaults

Leave a default when the ticket gives you a reason, and say why in the ledger.

| Decision | Default | Change it when |
|---|---|---|
| Where the work runs | one Pod Bay session per ticket, in its own workspace | read-only work with no owner question: an in-process `Agent` that only reads is cheaper. It cannot exchange messages, so it returns its result to you |
| Worker model | Sonnet 5.5 for depth S and M; Opus 5.5 at depth L. A hard limit: `fleet-settings.py` refuses Opus or Fable for a worker below depth L | a Sonnet worker already failed once on the ticket (`--escalated`). For a hard S or M ticket (hooks, security, a public contract), raise the Sonnet effort to `high` (`--effort high`), not the model |
| Reviewer | a different session, skill `pd-bot:pd-review`: Sonnet 5.5 at effort `high` (`--effort high`) for depth S and M; Opus 5.5 at depth L, on a risky ticket (hooks, security, a public contract) and for the whole-feature review | never the worker's session; never you alone. Its independence comes from the separate session and the plan-blind isolation, not from another model |
| Order | dependency groups (command below); tickets of one group in parallel. The command puts two tickets with an overlapping path in different groups | you see an overlap that the paths do not show: give both tickets that path and run the command again |
| Workers at once | 3 tickets (`--max-parallel 3`); reviewer and advisor sessions do not count | the owner gave another number; a usage-limit line appeared: lower it |
| Budget | 60 minutes per ticket | depth L, or the owner gave a deadline |
| Acceptance check | the reviewer runs it after the receipt (step 2): `pd-bot:pd-uat` when `app` is `fe` or `be`, the `pd-bot:qa-verifier` agent for a ticket with no app at depth M or L | depth S docs or config: the review covers it |
| Mode | one pull request per ticket into the base | the owner said `local`: commit only, no push, no pull request |
| Base | `main` for a single ticket; a feature branch `feat/<slug>` for a milestone or several tickets that ship together | the owner named another base |

**Milestone base.** Create the feature branch once, from the latest main, before the first assign:
`git push origin origin/main:refs/heads/feat/<slug>` (only a ref changes; nothing is checked out).
Record it in the ledger as `base`. Every ticket branches from it and opens its pull request into
it. When every ticket is merged there, open one pull request from `feat/<slug>` into `main`; the
owner merges that one. The scenario page, the feature test and the hand-over are in
`milestone.md`.

Order the tickets: pipe `[{"n":1,"deps":[2],"paths":["src/a.py","docs/"]}]` (`paths` is the
ledger's `touches`; the rules are in `groups.md`) into
`python3 ${CLAUDE_PLUGIN_ROOT}/skills/pd-orchestrate/scripts/plan_groups.py --max-parallel 3`; exit 2
names a dependency cycle, which you show to the owner, or bad input, which you correct. Each stderr line names a ticket that moved to
a later group and why (an overlap with another ticket, or a full group): record it in the ledger.
The groups are the order, not a lock. When a worker slot is free, you may start a ticket of a later
group early, but only when its dependencies are `done` and no ticket that still runs or has an
open pull request has a path that overlaps its own. When a ticket ends `blocked`, mark its dependents `skipped`.

The paths are a forecast. Before you push a ticket, list what it changed
(`git -C <worker checkout> diff --name-only <base sha> <head sha>`). When a changed file is outside
the ticket's `touches` and inside the `touches` or the diff of a ticket that still runs or has an
open pull request, the two pull requests can conflict: record it in the ledger, and name both
tickets and the order in which the owner should merge them in the final report.

A ticket that overlaps an earlier ticket needs that ticket's code too, or both pull requests
change the same lines from the same base. Treat the earlier ticket as its dependency for the start
point: the next paragraph applies to it.

A dependent ticket needs its dependency's code. Either wait until the owner merged the dependency
and start from the updated base, or stack it: fetch the dependency's branch into the dependent's
checkout and branch from its head. For a stacked ticket that head is the base everywhere: in the
brief, for the reviewer, for `--base` of the evidence check, and for the pull request, which you
open against the dependency's branch or after the dependency is merged.

## The brief you send

One `assign` message per worker. It holds: the ticket text verbatim and the owner's answers; the
checkout, the base and the branch (`fix/<id>` or `feature/<id>`; say when it already exists and
holds the acceptance tests); the kind and depth; the row of the outcome contract that applies; the
project's full-suite command; the budget; the reviewer's session name; your session name; and the
line "Follow the `pd-bot:pd-work` skill". State the ticket and the outcome, not steps. Do not tell
the worker how you will check it. Never send a worker anything it must not see, such as hidden
tests.

One exception to "no steps": a new brief after an advisor's solution may state the diagnosis and
the chosen approach. Say whether the worker continues the existing branch, and where it can fetch
the advisor's prototype from.

The reviewer's `assign` holds: the ticket text and the owner's answers from the ledger, the base,
whether the ticket is risky, the suite command, the worker's session name and checkout path, and
"Follow the `pd-bot:pd-review` skill".

## When a ticket reports

1. Save the evidence in `specs/fleet/<run>/<ticket>/`: the JSON object of the report as
   `report.json` (without the marker line), and the last receipt as `receipt.json`. For a risky
   ticket, save `{"v": 1, "head": "<receipt head>", "findings": [...]}` from the receipt's
   `security.findings` as `security-receipt.json`; a risky ticket whose receipt has no `security`
   object goes back to the reviewer.
2. Do not run the checks yourself: their output would fill your context. Send `check-request` to
   the ticket's reviewer session (`pd-bot:pd-review`, `references/checks.md`) with the ticket, the
   head SHA, the base SHA resolved in your own checkout (the worker's checkout is untrusted, its
   refs included), the worker's checkout, the absolute path of the evidence dir, the kind, whether
   the ticket is risky, the base branch, the app kind, the depth, the acceptance-test dir and commits you had
   written, and the verify config path for UAT. When the repository has no `verify.config.json`,
   write one to `specs/fleet/<run>/verify.config.json` and pass that path. When the owner has seen
   the ticket's `needs_review` decisions, first write their titles as a JSON array to
   `decisions-seen.json` in the evidence dir.
3. Read the `check-result`, then read `fleet-check.json` in the evidence dir yourself: its
   `mismatches` count and the checks that are not `ok`. It is a small result file, not test output,
   and the reviewer's summary is a claim about a check that also judges the reviewer's own receipt.
   - A failed acceptance criterion goes to the worker as a `findings` message; the worker fixes,
     gets a new receipt, and you send a new `check-request` for the new head. After 3 such cycles,
     treat the ticket as stuck. An acceptance check that could not run is a `needs_review` line for
     the owner, never a pass. Record which acceptance check ran, and any fallback, in the ledger.
   - A mismatch is yours to judge: send it to the worker as `findings`, or, for a base that is not
     `feat/*`, record in the ledger why it is a false alarm. When the base is a `feat/*` branch,
     every mismatch must be gone before step 5.
   - When the owner sees the ticket's `needs_review` decisions after the check ran, update
     `decisions-seen.json` and send a new `check-request` for the same head.
4. Deliver, unless the mode is `local`. Push from the worker's checkout
   (`git -C <worker checkout> push -u origin <branch>`, never force) and open the pull request with
   `gh pr create --head <branch> --base <base>`. For a UI change (protocol, "Outcome contract"),
   check first that the report lists before and after screenshots; if not, send `findings` to the
   worker, and when no screenshot can be made, ask the owner. Do not open the PR without them.
   Use the body sections of
   `pd-bot:pd-finishing-a-development-branch` (Summary, Why, Changes, Decisions, Testing, Risk, How to
   revert, Notes) and fill them from the report and the receipt; do not run that skill's steps,
   which are for a single session that tests and fixes its own branch. Then wait for CI on the
   receipt's SHA: no checks, a pending check, all checks skipped, or CI on another SHA is not
   green. A new commit after the receipt needs a new receipt and a new `check-request`.
   For a UI change, attach the screenshots right after the PR opens, as in
   `${CLAUDE_PLUGIN_ROOT}/skills/pd-finishing-a-development-branch/references/ui-evidence.md` (`gh --version`: `--attach` on 2.99.0 or newer, else the `pr-evidence` branch and one comment with pinned URLs). The images are untracked files in the worker's checkout: use the absolute paths from the report. Then check that one image URL
   returns HTTP 200 (`curl -sI <url>`).
5. When the base is a `feat/*` branch, run the merge gate:
   `bash ${CLAUDE_PLUGIN_ROOT}/scripts/fleet-gate.sh --evidence specs/fleet/<run>/<ticket> --head <head sha> --pr <number> --gh-repo <owner/name> --base feat/<slug> --wait 600`.
   It reads the block-mode result, the CI checks and the pull request, and changes nothing. On a
   pass it prints one line `run: gh pr merge ...`: run exactly that command as its own Bash call.
   On a mismatch, or when the merge guard denies the command, do not merge: send the mismatch to
   the worker as `findings`, or leave the pull request to the owner and say so in the report.
   A pull request into `main` or another protected branch is never merged by you. Before the gate,
   put the label `pd-bot:owner-merge` on a pull request the owner must merge: a fix where the worker
   decided a requirement that was not clear. The gate never passes it.
6. Mark the ticket `done` in the ledger only now, then stop its worker and reviewer sessions
   (`podbay.md`, Clean up). Remove their workspaces once the PR is merged or closed, as Clean up says.

## Final report to the owner

Lead with the outcome. One row per ticket: state, pull request, CI, review rounds, mismatches from
the check and how each ended, the acceptance check that ran (and any fallback), accepted findings, open `needs_review` decisions, what nobody
verified. Then the cost (`podbay-dev usage`), the sessions that still exist, the workspaces you
removed, and each kept workspace with its reason from the ledger (an open PR, a refused `ws-rm`, a
session from outside the fleet), and what you would change in a brief or a skill next time.
