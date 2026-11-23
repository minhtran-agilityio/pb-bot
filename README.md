<p align="center">
  <img src="assets/pd-bot-hero.jpg" alt="A miniature papercraft workshop moving coding tasks through discovery, design, planning, execution, review, and shipping gates" width="100%">
</p>

<div align="center">

# pd-bot

**A ready-to-use toolkit of skills, agents, and hooks for the [Claude Code](https://claude.com/claude-code) CLI**

*Prompt-powered workflows that carry a change from ticket to reviewed pull request, run by a fleet of [Pod Bay](#requirements) sessions.*

</div>

---

Skills are Markdown prompt programs you summon with `/pd-bot:skill-name`. The harness is a Claude Code plugin, so every skill carries the `pd-bot:` prefix. They run a change from a ticket to a reviewed pull request, and a human merges it.

## How a change flows

```
ticket -> /pd-bot:pd-intake -> verdict + depth -> build -> review -> ship
```

- `/pd-bot:pd-intake` returns a verdict (READY, CONFIRM, GRILL or BLOCKED) and a depth (S, M or L).
- Depth S runs `writing-plans` (one or two steps), then `implement`. Depth M adds `shape` (design) first. Depth L adds `shape` and `xia2` (research) first.
- A GRILL ticket runs `shape`, which settles the open decisions with the owner before any plan.
- A bug fix first runs `diagnosing-bugs`: the root cause is proved before any fix.
- `/code-review` and `intent-review` check the diff. `finishing-a-development-branch` pushes and opens the PR.

One principle drives it: **ceremony scales with the size of the change, and human interruption scales with ambiguity.**

See [plugins/pd-bot/skills/README.md](plugins/pd-bot/skills/README.md) for every skill and handoff, and the Hooks table in [CLAUDE.md](CLAUDE.md) for what the seven hooks enforce, including `merge-guard.sh`, which lets a plain `gh pr merge <n>` into `feat/*` run and asks the owner about every other merge, a push to main or master, and an MCP merge tool; in a subagent, where nobody can answer, it denies (ADR 0004).

## Run tickets with a fleet

`/pd-bot:pd-orchestrate <milestone | #n ... | free text> [base=<branch>] [local]` is the only way to run tickets with agents. The main session is the orchestrator: it triages every ticket, asks the owner every open question once, then gives each ticket to its own Pod Bay session. Each session works in its own workspace (a clean clone), so no git worktree is needed.

| Role | Skill | What it does |
|---|---|---|
| Orchestrator | `pd-orchestrate` | Assigns tickets, keeps the ledger `specs/fleet/<run>/ledger.json`, pushes and opens the PRs |
| Worker | `pd-work` | Builds one ticket and reports with evidence |
| Reviewer | `pd-review` | A different session: re-runs the tests, judges the diff against the request, runs the acceptance check and `fleet-check.sh` |
| Advisor | `pd-advise` | A Fable 5.2 session that researches a problem when the orchestrator is stuck |

For a milestone, the tickets go into one `feat/<slug>` branch, and the owner merges that branch into main. `/pd-bot:pd-tickets` turns a brief into tickets that `pd-orchestrate` can start with no new question.

## Installation

### Add to a project

```bash
claude plugin marketplace add minhtran-agilityio/pb-bot
claude plugin install pd-bot@pd-bot
```

Then, in the project, run `/pd-bot:pd-setup`. It scaffolds `specs/` and `docs/solutions/`, proposes a `## Run and test` section for your `CLAUDE.md` if it has none, merges the `permissions.deny` rules into `.claude/settings.json` (needs [jq](https://jqlang.github.io/jq/)), and adds `specs/` to `.gitignore`. Every step is safe to run twice. A plugin cannot ship `permissions.deny`, which is why setup does it. Restart Claude Code after installing so it loads the skills, agents and hooks.

### Requirements

- Pod Bay (`podbay-dev`, with `tmux`): pd-bot runs only with Pod Bay.
- `git`, `python3`, and `gh` for issues and pull requests.
- `jq` for `/pd-bot:pd-setup`.

### Known limits

- Not verified yet: the hooks firing in a live session.
- Merge policy: a fleet run merges ticket PRs only into `feat/<slug>`; `merge-guard.sh` lets a plain `gh pr merge <n>` into `feat/*` run and asks the owner about every other base (ADR 0004). Setup offers no allow rule: to let the orchestrator merge into `feat/<slug>`, allow `gh pr merge` (for example `Bash(gh pr merge *)`; the guard still asks about other bases).

## Pod Board

`apps/pod-board/` is a menu bar app that shows every Pod Bay session by state (`Needs you`, `Working`, `Idle`) and notifies you when a session needs you or has finished. It only reads `podbay-dev`. It is not part of the plugin: build it yourself, see [apps/pod-board/README.md](apps/pod-board/README.md).

## Develop on this repo

The plugin source is `plugins/pd-bot/`. Load it straight from the checkout, with no copy step:

```bash
claude --plugin-dir plugins/pd-bot
```

Check it with `claude plugin validate plugins/pd-bot --strict` and `claude plugin validate . --strict`.

### Test

`bash scripts/run-tests.sh` runs what CI runs: syntax checks, the doc-truth and markdown-link lints, the frontmatter and tool-conformance checks, hermetic hook and script tests, and the Python unit tests when `pytest` is available. The `pd-bot-ci` workflow runs it on ubuntu and macOS. `bash scripts/check-shellcheck.sh` runs shellcheck over the harness shell scripts; CI runs it with `REQUIRE_SHELLCHECK=1`.

## Author

**Minh Tran** - [@minhtran-agilityio](https://github.com/minhtran-agilityio) - <minh.tran@asnet.com.vn>.

## License

MIT. See [LICENSE](LICENSE).
