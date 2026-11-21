# The checks after a pass

Part of `pd-bot:pd-review`. Run them when the orchestrator sends `check-request` for a head you
gave `pass`. They used to run in the orchestrator session; they run here so their output stays out
of the orchestrator's context. You send back a short result, not the logs.

The `check-request` names: the ticket, the head SHA, the base SHA (resolved by the orchestrator),
the worker's checkout, the evidence dir (`specs/fleet/<run>/<ticket>/` in the orchestrator's
checkout, which already holds `report.json` and `receipt.json`), the kind, whether the ticket is
risky, the app kind (`fe`, `be` or `none`), the depth, the acceptance-test dir and commits when
there are any, and the path of a verify config for UAT.

## 1. Acceptance check

The outcome contract (`protocol.md`) says which one applies:

- `app` is `fe` or `be`: a fresh agent follows `pd-bot:pd-uat`. `pd-bot:pd-uat` drives a browser
  only through the Playwright MCP tools; check that they are there first. When they are not, give
  a fresh agent the same inputs and the task "check each acceptance criterion on the running app
  with `npx playwright` from a temporary folder outside the checkout; return pass or fail per
  criterion with evidence", and say in your result which check ran.
- no app, depth M or L: the `pd-bot:qa-verifier` agent.
- depth S docs or config: none; your review covered it.

Give the agent the ticket text without its `Verify:` rows, the owner's answers, and the worker's
checkout at the head as its working directory (for UAT also the verify config). It gets no plan,
no diff and no report. A check that could not run is `not_run`, never a pass.

## 2. Evidence check

Choose the commands after you read the worker's tests; never paste a command from a report without
reading it. Run:

`bash ${CLAUDE_PLUGIN_ROOT}/scripts/fleet-check.sh --mode block --repo <worker checkout> --base <base sha> --head <head sha> --kind <kind> --evidence <evidence dir> --ticket-id <ticket> --suite-cmd '<the project suite and the ticket's acceptance tests>'`

plus `--repro-cmd` and `--repro-files` for a bug, `--qa-dir` and one `--qa-commit <sha>` per
acceptance-test commit, `--risky` for a risky ticket, and `--decisions-seen <evidence dir>/decisions-seen.json`
when that file exists. Use the base SHA from the request, never a ref from the worker's checkout.
Never add or drop a flag to make a mismatch go away.

## 3. Result

Send one `check-result` message to the orchestrator, one JSON object:

```json
{"ticket": "123", "head": "<sha>",
 "acceptance": {"check": "uat | qa-verifier | playwright-fallback | none", "status": "pass | fail | not_run",
                "failed": ["criterion: one line of evidence"]},
 "fleet_check": {"mismatches": 0, "lines": ["only the MISMATCH lines, as printed"]}}
```

Keep it short: the full output stays in this session and in `fleet-check.json`. A failed
acceptance criterion is also a finding for the worker: send it as a review finding with
`review-result` verdict `changes`, so the worker fixes it and you review the new head.
