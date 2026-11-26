---
type: operations
title: Fleet Check, Gate and Settings Scripts
description: What fleet-check.sh, fleet-gate.sh, fleet-settings.py (with fleet-trim.json) and ledger-backup.py enforce for fleet runs, including the trust model, warn versus block mode, the feat/* gate and per-workspace role settings.
tags: [fleet, evidence, scripts, gate, settings, security]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-26T15:02:34.678Z
sources:
  - id: openwiki-source-6f2fef4d4ed88eb3f13d3344
    resource: repo://plugins/pd-bot/scripts/fleet-check.sh
  - id: openwiki-source-edb2b501678858ff46c18e87
    resource: repo://plugins/pd-bot/scripts/fleet-gate.sh
  - id: openwiki-source-08ae4b27471cecfc629f21e6
    resource: repo://plugins/pd-bot/scripts/fleet-settings.py
  - id: openwiki-source-e835a7b345c56139f3df0319
    resource: repo://plugins/pd-bot/scripts/fleet-trim.json
  - id: openwiki-source-48bdf7906468e53b3191e097
    resource: repo://plugins/pd-bot/scripts/ledger-backup.py
generated: { by: "claude-code", at: "2026-11-26T15:02:34.678Z" }
---

# Fleet Check, Gate and Settings Scripts

<!-- openwiki: broken internal link [/openwiki/workflows/fleet-orchestration.md] link "/openwiki/workflows/fleet-orchestration.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
Four scripts in `plugins/pd-bot/scripts/` turn the fleet protocol ([Fleet Orchestration](/openwiki/workflows/fleet-orchestration.md)) into mechanical checks. The orchestrator runs them; none of them edits the worker's checkout or the pull request.

| Script | Question it answers | Writes |
|---|---|---|
| `fleet-check.sh` | Does the end state of one ticket show the required evidence? | `<evidence>/fleet-check.json` |
| `fleet-gate.sh` | May this ticket PR go into its `feat/*` branch without the owner? | `<evidence>/fleet-gate.json` |
| `fleet-settings.py` | How is a workspace prepared so a session runs one role unattended? | `.claude/settings.local.json` in the workspace |
| `ledger-backup.py` | Can the orchestrator's ledger survive a `git clean`? | a copy under the git directory |

## fleet-check.sh: end-state evidence

`fleet-check.sh` checks **results, never the order of work**. It takes `--repo`, `--base`, `--head` (full 40- or 64-hex object names), `--kind bug|feature|docs`, `--evidence <dir>` and optional repro, suite and QA options, and runs fixed checks in this order: **receipt, intent, report, decisions, acceptance, security, repro, suite**. Each prints `ok`, `MISMATCH` or `skip`, the results are written to `fleet-check.json`, and the last line is `fleet-check: <n> mismatch(es), mode <mode>`.

What each check requires:

- **receipt** — `receipt.json` is a single JSON object whose `head` equals `--head`, whose `verdict` is `pass`, and whose findings are all `open`/`fixed`/`accepted` with known severities, none open, and every `accepted` finding carrying `accepted_by` of `advisor` or `owner`. With `--ticket-id`, the ticket must match too.
- **intent** — `receipt.intent.verdict` is `match` and `intent.gaps` is empty.
- **report** — `report.json` is version 2, its `head` matches, and `status` is `done`.
- **decisions** — a decision with `needs_review` (anything but false or null) counts as unseen unless its title is listed in the `--decisions-seen` file, which only the trusted caller supplies.
- **acceptance** (with `--qa-dir`) — each `--qa-commit` is in the range and touches only the QA directory, no other commit touches it, there is no merge commit, and every code commit descends from the first QA commit (tests before code).
- **security** — files in the diff are matched against a risky-path pattern (`hooks`, `migrations`, `auth`, `security`, `secrets`, `scripts`, `.github`, `settings*.json`, `.sql`, `.env`, `REVIEW.md`). A risky path, or `--risky`, requires `security-receipt.json` for the same head with no open critical or high finding.
- **repro** — for `bug`, the repro command must be red on base (with the worker's test files overlaid from head) and green on head; exit 126/127 on base means the test did not run.
- **suite** — the suite command (default `bash scripts/run-tests.sh`) exits 0 on a throwaway head worktree; `--no-suite` skips it.

**Mode.** In `warn` mode (default) the script always exits 0 and reports mismatches; in `block` mode it exits 1 on any mismatch (and when it cannot write its result file). `CLAUDE.md` says it runs in block mode when the base is `feat/*`. Usage errors exit 2.

### Trust model

The script assumes the **worker controls its checkout** (refs, tags, `refs/replace`, config, hooks, attributes, the shallow file, every commit), while the evidence directory and CLI arguments come from the trusted orchestrator and JSON values inside evidence are untrusted text. So it never writes to `--repo`: it fetches `--base` and `--head` by full object name into a private bare mirror in its own temp directory and runs every git command against that mirror with a cleared environment, no hooks, no global or system config, replace objects off and only the file transport. It refuses a shallow repo or a head that does not descend from base. Evidence files are first snapshotted (symlinks are refused), and untrusted strings are reduced to printable ASCII and cut to 200 characters before printing. The repro and suite commands do run the worker's code, in a reduced `env -i` environment in throwaway worktrees, but they are **not sandboxed** (disk and network are reachable), so run the script where that is acceptable.

## fleet-gate.sh: unattended entry into a feature branch

For a milestone, ticket PRs go into `feat/<slug>`. `fleet-gate.sh` decides, and only decides, whether one such PR may merge without the owner. It requires `--base` to be a `feat/<name>` branch and passes only when three checks hold for the same head commit:

1. **evidence** — `fleet-check.json` is from a **block-mode** run on `--head`, has zero mismatches, no check that is neither ok nor skip, and exactly one `suite` check with result ok (a skipped suite is not enough).
2. **ci** — every PR check passed (`gh pr checks`; skipped checks do not count as a pass). With `--wait`, it polls every 30 s (`FLEET_GATE_INTERVAL` overrides it) while checks are pending, absent or erroring; a failed check is final, as are unrecoverable `gh` errors.
3. **pr** — the PR is open, not a draft, targets exactly `--base`, has head equal to `--head`, carries no `pd-bot:owner-merge` label, and the commit that `fleet-check` ran from is in the history of the feature branch (via the GitHub compare API), so the check covered every commit the PR brings in.

On a pass it prints, rather than runs, `gh pr merge <n> --repo <repo> --merge --match-head-commit <head>`; the orchestrator runs that as its own Bash call so `merge-guard.sh` still judges it. The label `pd-bot:owner-merge` is how the orchestrator reserves a PR for the owner. Exit codes: 0 pass, 1 mismatch, 2 usage.

## fleet-settings.py: prepare a workspace for a role

<!-- openwiki: broken internal link [/openwiki/hooks/hook-system.md] link "/openwiki/hooks/hook-system.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
Before `podbay-dev start`, the orchestrator runs `fleet-settings.py --checkout DIR --role worker|reviewer|tester|advisor|keeper --model ID --mode auto|acceptEdits` (and `--restore` after the session stops, `--show` to inspect). It writes keys into the checkout's `.claude/settings.local.json`: `model`, optional `effortLevel`, `permissions.defaultMode`, and `env.PD_BOT_ROLE`, `PD_BOT_TICKET`, `PD_BOT_RUN`. `PD_BOT_ROLE` is what `session-role.sh` reads ([Plugin Hook System](/openwiki/hooks/hook-system.md)).

It also applies the **token trim** from `fleet-trim.json`: `disableClaudeAiConnectors: true` and several unused plugins set to false, which the script documents as roughly 20k fewer tokens per request. `--keep-connectors` skips the trim. `autoCompactWindow` is written only with `--compact-window N` (100000 to 999999999) and is never copied from the trim file, so by default the owner's own setting applies.

**Guards.**

- **Model guard**: a `worker` must use a Sonnet or Haiku model unless `--depth L` or `--escalated` (a Sonnet worker already failed once on the ticket) is given.
- It **refuses** when the settings file is not ignored by git in that checkout, when `.claude` or the file is a symlink, and it never writes `bypassPermissions` (only `auto` or `acceptEdits` are accepted), so a fleet session keeps permission checks and hooks.
- All checks run before anything is written, and writes are atomic with mode 0600.

**Restore.** Previous values of every key it sets (or their absence, and whether the file existed) are saved as `pd-bot-fleet-settings.json` in the checkout's git directory, never in the work tree. A second apply keeps the first's saved values. `--restore` puts back only those keys, preserving other changes made meanwhile (for example a permission the session saved), and removes the settings file if it did not exist before and nothing else is left. Tests: `plugins/pd-bot/scripts/test_fleet_settings.py`.

## ledger-backup.py

The ledger `specs/fleet/<run>/ledger.json` is the orchestrator's memory, but `specs/` is gitignored, so `git clean -fdx` would delete it. After each ledger write, `ledger-backup.py --checkout DIR --run RUN` copies it to `<git dir>/pd-bot/fleet/<run>/ledger.json`; `--restore` copies it back when the ledger is missing. The copy is never committed, survives `git clean`, and is lost only with the checkout. It refuses symlinks and non-object JSON, and run ids must match a conservative pattern. Tests: `plugins/pd-bot/scripts/test_ledger_backup.py`.

## Tests

<!-- openwiki: broken internal link [/openwiki/testing/test-suite-and-ci.md] link "/openwiki/testing/test-suite-and-ci.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
`tests/scripts/fleet-check.test.sh` and `tests/scripts/fleet-gate.test.sh` exercise the two shell scripts; the Python scripts have pytest files next to them. All run under `scripts/run-tests.sh` ([Test Suite, Lints and CI](/openwiki/testing/test-suite-and-ci.md)).
