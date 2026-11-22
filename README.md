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
- `/pd-bot:pd-autopilot` asks the owner every open question once, then runs the whole flow for one or more tickets.

One principle drives it: **ceremony scales with the size of the change, and human interruption scales with ambiguity.**

See [plugins/pd-bot/skills/README.md](plugins/pd-bot/skills/README.md) for every skill and handoff, and the Hooks table in [CLAUDE.md](CLAUDE.md) for what the five hooks enforce, including `merge-guard.sh`, which lets a plain `gh pr merge <n>` into `feat/*` run and asks the owner about every other merge, a push to main or master, and an MCP merge tool; in a subagent, where nobody can answer, it denies (ADR 0004).

## Installation

### Add to a project

```bash
claude plugin marketplace add minhtran-agilityio/pb-bot
claude plugin install pd-bot@pd-bot
```

Then, in the project, run `/pd-bot:pd-setup`. It scaffolds `specs/` and `docs/solutions/`, proposes a `## Run and test` section for your `CLAUDE.md` if it has none, merges the `permissions.deny` rules into `.claude/settings.json` (needs [jq](https://jqlang.github.io/jq/)), and adds `specs/` to `.gitignore`. Every step is safe to run twice. A plugin cannot ship `permissions.deny`, which is why setup does it. Restart Claude Code after installing so it loads the skills, agents and hooks.

Needs `git` and `python3`. The ship workflow also needs the `Workflow` tool.

### Known limits

- Not verified yet (tracked for the dogfood run, #352): an unattended `ship.js` run from the plugin cache, the hooks firing in a live session, and a git-sourced marketplace install.
- Merge policy: the workflow merges gated ticket PRs into `feat/<slug>`. It also merges the final PR when its base is `feat/*` (not the default branch) and every gate passed; every PR into another base is a human merge, because `merge-guard.sh` lets a workflow agent merge only into `feat/*`. On a `feat/*` base, a ticket gate that fails only on review findings or `needs_review` decisions asks the advisor once; the advisor may accept those items at ticket level, and the final PR body lists them under `## Accepted by advisor`. Setup offers no allow rule: the owner who wants ticket PRs merged by the run must allow `gh pr merge` into `feat/<slug>*` (for example a `Bash(gh pr merge *)` allow rule; the guard still asks about other bases). In default permission mode the merge stage is otherwise denied; the run then reports the missing permission as the reason and does not work around it.
- `/pd-bot:pd-autopilot` launches the `autopilot-run` workflow by name (`pd-bot:autopilot-run`), which works from any working directory. If a session does not know the name, the Workflow tool refuses a `scriptPath` it cannot read: add the plugin directory with `/add-dir`, or pass `ship.js` inline through `script`.
- After a plugin update, a running session can keep a stale `pd-bot:autopilot-run`. Restart Claude Code (the main fix; `/reload-plugins` is named by the plugin CLI help but not confirmed here). Or, when `plugins/pd-bot/workflows/ship.js` of this repo is inside the working directory or an added directory (`/add-dir`), launch it with `scriptPath`. The completion report names the script that ran, from the `Script file:` line of the Workflow result.
- The workflow was renamed from `ship` to `autopilot-run` (PR 509). The by-name launch is not yet verified under the new name: after a restart, run a `--no-ask` dry run and confirm the Workflow tool finds `pd-bot:autopilot-run`.

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
