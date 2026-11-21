# Pod Bay for the orchestrator

Pod Bay runs Claude sessions in tmux, each in a checkout of a workspace: `src`, or a parallel
workspace `src-NN` of clean clones. This is the whole CLI as of `podbay-dev 0.1.22` (checked with
`--help` on every subcommand, 2026-11-19). The binary may also be named `podbay`. If a command here
fails with a usage error, the CLI changed: run it with `--help` and go on.

A command prints JSON when it succeeds and one line that starts with `podbay:` when it fails (a
wrong session name gives `no agent session named ...`). Every command takes `--on <machine>` to run on another machine of the
network; without it the command runs here.

## Three names of one session

`podbay-dev ls` gives each session three identifiers. Use the right one:

| Field in `ls` | Example | Use it for |
|---|---|---|
| `title` | `Pb Bot #UA-04` | `SendMessage` (`to`), and when you name a session to the owner |
| `name` | `claude-pb-bot-04` | `detail`, `stop`, `queue add`, and the tmux target `=<name>:` |
| `uuid` | `b1d2918a-…` | `describe`, and `start --resume <uuid>` |

Other `ls` fields: `status` (`busy` or `idle`), `asks` (true: a dialog waits for a human),
`waiting_for`, `dir` (the checkout path), `changes`, `links` (pull requests, issues, artifacts),
`description`, `attach` (the tmux command for a human).

## Which session is the orchestrator

A SessionStart hook of the plugin (the session-role hook) gives the orchestrator role text to one session: the Pod Bay
session of the main workspace (`src`). It reads the tmux options that Pod Bay sets on each session
it starts:

| tmux option | Example | Meaning |
|---|---|---|
| `@tn_kind` | `claude` | Pod Bay started this session |
| `@tn_context` | `pb-bot/pb-bot`, or `pb-bot/pb-bot@04` | `<project>/<repo>` in `src`; the suffix `@NN` means workspace `src-NN` |
| `@claude_dir` | the checkout path | must be the session's project directory |

Sessions in `src-NN` get no role from the hook: your brief gives a worker, a reviewer or an advisor
its role. Pod Bay does not document these options. When they change, the hook is silent; check with
`tmux show-options -t '<name>'`. The owner switches the role off with `{"orchestrator": "off"}` in
the repository's `.claude/pd-bot.json`, or with `PD_BOT_ROLE=none` for one session;
`PD_BOT_ROLE=orchestrator` gives the role to a session that Pod Bay did not start. `PD_BOT_ROLE`
wins over the repository's off switch, so set it for one session and never in a shell profile.

## Commands you use

| Need | Command |
|---|---|
| Sessions on this machine (`--all`: every machine) | `podbay-dev ls` |
| Checkouts and workspaces: `ws`, `path`, `branch`, `dirty`, `ahead`, `behind`, `has_conversation` | `podbay-dev checkouts` |
| A new workspace of clean clones; prints `"ws": "src-NN"` | `podbay-dev ws-new <project>` |
| A session in a checkout; prints its `title`, `name`, `uuid` | `podbay-dev start <project> <repo> --ws <src-NN> --trust` |
| Bring a stopped session back with its conversation | `podbay-dev start <project> <repo> --ws <src-NN> --resume latest` (or `--resume <uuid>`) |
| Say what a session is for; shows in `ls`; empty text clears it | `podbay-dev describe <uuid> '<text>'` |
| Last prompt, last reply and the screen of a session | `podbay-dev detail <name> --lines 30` |
| Type a message into a session when it is next idle | `podbay-dev queue add <name> '<text>'` |
| What is queued; remove, change or reorder one | `podbay-dev queue ls`, `queue rm <id>`, `queue edit <id> '<text>'`, `queue move <id> up` or `down` |
| Send a queued message even while a human is attached; requeue one of unknown outcome | `podbay-dev queue now <id>`, `podbay-dev queue retry <id>` |
| Update a checkout from its remote | `podbay-dev pull <project> <repo> --ws <src-NN>`: fast-forwards when no session runs there, asks an idle session to pull, refuses under a busy one |
| Stop a session; its conversation is kept | `podbay-dev stop <name>` |
| Delete a workspace; it refuses when work is not pushed. Only as Clean up says | `podbay-dev ws-rm <project> <src-NN>` |
| Tokens used: `last_5h`, `last_24h`, `last_7d`, `by_model` | `podbay-dev usage` |
| The machines of the network and whether each is online | `podbay-dev machines` |

`<project>` and `<repo>` are the `project` and `repo` fields of `checkouts` (for example `pb-bot`
and `pb-bot`). `--root` is `projects` by default; pass `--root rfp` only for a checkout
whose `root` field says so. `--trust` lets the checkout's hooks and settings run; use it for
workspaces of the owner's own repository.

`SendMessage` is the normal way to talk to a session: it reaches the agent as a message and can
notify you when the session is idle. `queue add` types text into the session as if a human did; use
it for a slash command, or when a session does not accept cross-session messages.

## Commands you run only when the owner asks

| Command | Why it is the owner's |
|---|---|
| `ws-rm` of a workspace that Clean up does not cover | deletes a workspace the run did not finish with |
| `podbay-dev rm <project> <repo> --ws <src-NN>` | deletes one checkout; never add `--force`, which deletes work that is not pushed |
| `podbay-dev clone <gh \| glab \| url> <owner/repo or URL> <project>` (`--name`), `podbay-dev forges`, `podbay-dev repos <forge> [owner] --search <text>` | brings a new repository onto the machine |
| `podbay-dev login`, `logout`, `passwd`, `remove <machine>`, `setup`, `service install \| uninstall \| start \| stop \| status`, `update`, `traffic` | the network, its password and the machine's service: never part of a run |

## Start a session for a role

1. Pick a workspace whose checkout has no session (`ls` shows no session with that `dir`), or make
   one. One ticket per workspace; a reviewer or an advisor gets its own workspace too, so no
   session shares a working tree with another.
2. Start the session and keep its `title`, `name` and `uuid`.
3. Set the model: `tmux send-keys -t '=<name>:' '/model <model id>' Enter`, then read the status
   line with `detail` and confirm the model name. A session keeps its default model otherwise.
   `/model` also saves that model as the machine's default for new sessions. Note the default
   before the run (`model` in `~/.claude/settings.json`) and list it in the final report, so the
   owner can set it back.
4. Keep every session of a run in the same permission mode as yours. A session in another mode
   holds your messages until its user approves them. Workers, reviewers and advisors run tests and
   other project commands: in a mode that prompts for each one, they stop at every prompt, so tell
   the owner before the run when your mode is not auto.
5. `describe` the session with its role and ticket, so the owner sees it in the Pod Bay app.
6. Send the brief with `SendMessage` to the `title` and `notify_when_idle: true`. Record the
   session in the ledger.

## Wait

- One idle notice arrives for each `notify_when_idle` send. Do other work until then; if there is
  none, yield. Never loop on `ls` or `detail`.
- Look at a session with `detail` only when it is silent past its budget, or an idle notice came
  with no message from it. `ls` shows `asks: true` when a dialog waits.
- What you may see, and what it means:

| Screen | Meaning | Do |
|---|---|---|
| a permission prompt | the session waits for a human | tell the owner which session and which command; do not answer the prompt for them |
| an `AskUserQuestion` dialog | the session asks the owner | tell the owner; it is their answer |
| "held for approval" on your send | permission modes differ | restart the session in your mode, or ask the owner |
| an idle prompt, no report | the agent stopped early | send one message that asks for the report or the blocker |
| a usage-limit line | the account is out of quota | stop assigning; tell the owner when it resets |

## Other machines

`ls --all`, `checkouts --all` and `--on <machine>` reach other machines, and `detail` works there
when that machine shares details. A run stays on one machine all the same: idle notices and
delivery confirmation for messages exist only between sessions of the same machine.

## Clean up

Stop each session as soon as its work is finished, without waiting for the end of the run:
- a ticket's worker and reviewer when the ticket becomes `done`, `blocked` or `skipped` (not
  earlier: a failed acceptance check reopens it);
- a Fable advisor session when you have checked its `advise-result` and have no question left;
- any other session you started when it has reported and nothing waits on it.

`podbay-dev start ... --resume latest` brings a stopped session back with its conversation if a
ticket reopens. At the end of the run no session you started is still running.

Then remove the workspaces of a finished ticket yourself; the owner does not have to ask. Each run
leaves a full checkout per worker, reviewer and advisor, and nobody else removes them. A worker or
reviewer workspace of a ticket qualifies only when all of these hold:
- the ticket's pull request is MERGED or CLOSED (`gh pr view <n> --repo <repo> --json state`); an
  open PR keeps both workspaces, because a review comment can still reopen the ticket;
- the reviewer's last receipt is saved in `specs/fleet/<run>/<ticket>/`;
- no other ticket in the ledger is assigned to that workspace;
- `podbay-dev ls` shows no session in it that you did not start for this run.

A Fable advisor workspace qualifies when you have checked its `advise-result` and no ticket waits on
it. For each workspace that qualifies, in this order:
1. `podbay-dev stop <name>` for its session, if one still runs;
2. `podbay-dev ws-rm <project> <src-NN>`.

When `ws-rm` refuses (work that is not pushed), keep the workspace and write the reason in the
ledger. Never add `--force`, never delete files by hand, and never `ws-rm` a workspace for another
reason. Never remove `src` (your own workspace) or a workspace that a session from outside this
fleet uses. Record each removal in the ledger as well (`workspaces`, `protocol.md`).

Check the open PRs again before your final report, and once more when the owner says a PR was
merged or closed. List every workspace that stays, with its reason, in your final report.
