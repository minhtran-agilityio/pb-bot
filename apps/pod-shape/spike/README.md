# Shape Board spike (step 0): the p2g-questions protocol

This spike checks one question before any app code exists: can the app drive `pd-shape` and
`pd-tickets` through `claude -p` and `--resume`, when it disables `AskUserQuestion` and Claude asks
its questions as fenced `p2g-questions` JSON blocks? The app is planned in `apps/pod-shape/`.

## Files

| File | What it is |
|---|---|
| `append-system-prompt.md` | The text the app passes with `--append-system-prompt`. It is the whole protocol |
| `p2g-questions.schema.json` | JSON Schema of the JSON inside a block: the AskUserQuestion input shape |
| `p2g.py` | Stdlib helpers: find blocks in assistant text, validate them against the schema rules, flag question lines outside a block. `python3 -I p2g.py --selftest` |
| `spike.py` | The driver. `shape` grills a prototype N times; `tickets` runs `pd-tickets` on a brief up to its approval step |

## Run it

Each run clones the repo into `--work-dir` (a throwaway clone with its push URL set to a dead path),
copies the prototype into `prototypes/` there, and talks to Claude in that clone only. Transcripts
(`turn-NN.jsonl`), message text and the clone's `specs/` stay in `--work-dir`. Do not commit them:
they hold account data (session ids, cost).

```bash
python3 -I apps/pod-shape/spike/spike.py shape --repo "$PWD" \
  --prototype <prototype.html> --work-dir <scratch dir> --runs 3
python3 -I apps/pod-shape/spike/spike.py tickets \
  --clone <scratch dir>/shape-run-1/clone --brief specs/<slug>/brief.md --work-dir <scratch dir>
```

A script answers in place of the owner. It picks the first option of each question, and "No" on an
ADR question (the ADR step needs git, which the spike denies). In `pd-tickets` it answers only the
milestone question ("No milestone") and questions with no go-like option. It stops at the approval
question (an option such as "Create 9 issues"), which passes. Any other go-like option or option
with a `preview` stops it as "unplaced", which fails. It never sends "go".

## The flag set for `claude_runner`

First turn: `--session-id <uuid>`. Every later turn: `--resume <uuid>`. The last argument is the
message: the slash command on the first turn, `{"answers": {"<question>": "<label or text>"}}` after
that.

```text
claude -p
  --output-format stream-json --verbose
  --strict-mcp-config                      # load no MCP server
  --permission-mode dontAsk                # pinned: only --allowedTools run (see below)
  --permission-prompts none                # a tool call that would prompt is denied, not hung
  --model sonnet --effort medium
  --append-system-prompt "<append-system-prompt.md>"   # pass it on every turn (see below)
  --allowedTools  <per step>
  --disallowedTools <per step>
  < /dev/null                              # else claude -p waits 3 s for stdin and warns
```

| Step | `--allowedTools` | `--disallowedTools` |
|---|---|---|
| grill (`/pd-bot:pd-shape <prototype>`) | `Read Glob Grep Write(./specs/**) Edit(./specs/**)` | `AskUserQuestion Bash Agent WebFetch WebSearch NotebookEdit` |

**Finding, build step 3 (2026-11-23).** A bare `Read` on `--allowedTools` reads **any** file, even under
`dontAsk`. A haiku probe read a file outside the working folder and outside `--add-dir`. So the grill row
above lets a prompt injected through the prototype read `~/.ssh/*` into the chat and the transcript. The
app (`src-tauri/src/grill/flags.rs`) scopes the read tools instead: `Read(./**) Glob(./**) Grep(./**)`,
`Read(//<prototype folder>/**)` and `Read(~/.claude/plugins/**)` (pd-shape reads its own references), and
it denies `.env` files. A second probe showed that these rules allow those folders and deny an outside
file. The app also gives Claude no Bash rule at all, because a `*` inside a Bash rule matches extra
arguments: `Bash(git -C <dir>* status *)` allowed `git -C <dir> -c core.pager=cat status`.

**Two more findings, build step 3 (security review, probes 5-7).**

- A skill's `allowed-tools` widens the permissions when a turn starts with its slash command, even
  under `dontAsk`. A probe skill with `allowed-tools: Read, Write, Edit`, typed as `/probe`, read a
  canary file outside the repo and wrote one there. pd-shape lists bare `Read, Write, Edit`. So the
  app never types `/pd-bot:pd-shape`. It sends the text of pd-shape's `SKILL.md`, without the
  frontmatter, as the first message, and it denies the `Skill` tool. The Bash probe above held only
  because `Bash` was a bare tool name on the deny list.
- `Write(./specs/**)` matches nothing. `Edit(./specs/**)` allows Write and Edit under `specs/` and
  nowhere else. The spike's brief writes worked only because of pd-shape's bare `Write` grant.

The app also passes `--setting-sources ""`, so allow rules in a trusted repo's `.claude/settings.json`
or in the user's settings cannot widen the rules either.
| tickets, up to approval (`/pd-bot:pd-tickets <brief>`) | `Read Glob Grep` and the read-only `gh` and `publish-check.sh` calls in `spike.py` (`TICKETS_ALLOWED`), the milestone GET spelled exactly | `AskUserQuestion Write Edit Agent` and the `gh` and `git` write patterns in `spike.py` (`TICKETS_DENIED`) |

### Why the permission mode is pinned

The GitHub safety rule is: **pinned `--permission-mode dontAsk` plus an allowlist with no write on
it.** The deny list is a second line, not the control.

- `--permission-prompts none` denies only a call that would prompt; the permission mode decides the
  rest. Without `--permission-mode`, the mode comes from the user's and the project's settings. This
  checkout's `.claude/settings.local.json` sets `defaultMode: auto`, so the app must not leave it to
  settings. (The recorded runs without the flag reported `permissionMode: default`; the flag removes
  the doubt.)
- Probes with `claude -p --model haiku`: under `dontAsk`, `touch <file>` outside the allowlist was
  denied and no file was made. A probe skill with `allowed-tools: Bash(touch *)`, typed as a slash
  command, was denied the same `touch`: in `dontAsk` a skill's `allowed-tools` pre-allows nothing.
  So `pd-tickets`' own pre-allowed `gh issue create`, `gh label create` and milestone create cannot
  run unless the app puts them on `--allowedTools`. Commands that Claude Code treats as read-only
  (`echo`, `ls`) still run without being listed.
- A deny list alone cannot cover `gh api`: an implicit POST with any `-f key=value`, `-X` before the
  path, or a GraphQL mutation. Denying `gh api` as a whole would also block the milestone GET, because
  deny beats allow. `TICKETS_DENIED` names each write the skill pre-allows, every `gh` write
  subcommand family, and the `gh api` write spellings above.
- For the create step (handoff step 4), the app adds the write commands to `--allowedTools` only on
  the turn that carries the owner's "go".

A bare tool name in `--disallowedTools` removes the tool: a call returns `No such tool available`,
even inside a skill whose `allowed-tools` lists it. Set `PD_BOT_ROLE=none` in the environment, so the
pd-bot SessionStart hook adds no orchestrator role.

## Must `--append-system-prompt` be passed on every `--resume`?

Not for the snapshot, but pass it anyway. `claude --help` (2.1.330) says the system prompt is
recorded on the first request and every resume "sends the record as-is, even when a later launch
passes different text, until the conversation is compacted". A canary test agreed: turn 1 appended
"the secret word is ZEBRA-42"; a resume with no `--append-system-prompt` answered `ZEBRA-42`; a
resume that appended `OTTER-77` still answered `ZEBRA-42`. After a compaction the prompt is
rendered again from the current launch's flags, so a turn without the flag would lose the protocol.
Passing the same text on every turn is free and covers that case. The compaction case was not tested.

## Results

See [RESULTS.md](RESULTS.md).
