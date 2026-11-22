<p align="center">
  <img src="assets/pd-bot-hero.jpg" alt="A miniature papercraft workshop moving coding tasks through discovery, design, planning, execution, review, and shipping gates" width="100%">
</p>

<div align="center">

# pd-bot

**A ready-to-use toolkit of skills, agents, and hooks for the [Claude Code](https://claude.com/claude-code) CLI**

*Prompt-powered workflows that carry a change from brainstorm to ship.*

</div>

---

Skills are Markdown prompt programs you summon with `/pd-bot:skill-name`. The harness is a Claude Code plugin, so every skill carries the `pd-bot:` prefix. They run a change from a ticket to a reviewed pull request, and a human merges it.

## How a change flows

```
ticket -> /pd-bot:pd-intake -> verdict + depth -> build -> review -> ship
```

- `/pd-bot:pd-intake` returns a verdict (READY, CONFIRM, GRILL or BLOCKED) and a depth (S, M or L).
- Depth S goes straight to `implement`. Depth M adds `writing-plans`. Depth L adds `brainstorming` and `xia2` first.
- A bug fix first runs `diagnosing-bugs`: the root cause is proved before any fix.
- `/code-review` and `intent-review` check the diff. `finishing-a-development-branch` pushes and opens the PR.
- `/pd-bot:pd-orchestrate` asks the owner every open question once, then runs one or more tickets with a fleet of Pod Bay sessions: one worker per ticket, a separate reviewer, and an advisor when stuck.

One principle drives it: **ceremony scales with the size of the change, and human interruption scales with ambiguity.**

See [plugins/pd-bot/skills/README.md](plugins/pd-bot/skills/README.md) for every skill and handoff, and the Hooks table in [CLAUDE.md](CLAUDE.md) for what the five hooks enforce, including `merge-guard.sh`, which lets a plain `gh pr merge <n>` into `feat/*` run and asks the owner about every other merge, a push to main or master, and an MCP merge tool; in a subagent, where nobody can answer, it denies (ADR 0004).

## Installation

### Add to a project

```bash
claude plugin marketplace add minhtran-agilityio/pb-bot
claude plugin install pd-bot@pd-bot
```

Then, in the project, run `/pd-bot:pd-setup`. It scaffolds `specs/` and `docs/solutions/`, proposes a `## Run and test` section for your `CLAUDE.md` if it has none, merges the `permissions.deny` rules into `.claude/settings.json` (needs [jq](https://jqlang.github.io/jq/)), and adds `specs/` to `.gitignore`. Every step is safe to run twice. A plugin cannot ship `permissions.deny`, which is why setup does it. Restart Claude Code after installing so it loads the skills, agents and hooks.

Needs `git`, `python3` and Pod Bay (`podbay-dev`): pd-bot runs only with Pod Bay.

### Known limits

- Not verified yet: the hooks firing in a live session, and a git-sourced marketplace install.
- Merge policy: a fleet run merges ticket PRs only into `feat/<slug>`; `merge-guard.sh` lets a plain `gh pr merge <n>` into `feat/*` run and asks the owner about every other base (ADR 0004). Setup offers no allow rule: to let the orchestrator merge into `feat/<slug>`, allow `gh pr merge` (for example `Bash(gh pr merge *)`; the guard still asks about other bases).

### Develop on this repo

The plugin source is `plugins/pd-bot/`. Load it straight from the checkout, with no copy step:

```bash
claude --plugin-dir plugins/pd-bot
```

Check it with `claude plugin validate plugins/pd-bot --strict` and `claude plugin validate . --strict`.

### Test

`bash scripts/run-tests.sh` runs what CI runs: syntax checks, the doc-truth and markdown-link lints, the frontmatter and tool-conformance checks, hermetic hook and script tests, and the Python unit tests when `pytest` is available. The `pd-bot-ci` workflow runs it on ubuntu and macOS.

## Author

**Minh Tran** - [@minhtran3124](https://github.com/minhtran-agilityio/pb-bot) - <minh.tran@asnet.com.vn>.

## License

MIT. See [LICENSE](LICENSE).
