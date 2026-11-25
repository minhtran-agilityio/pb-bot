# Changelog

All notable changes to this harness are recorded here. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions are tracked in the root `VERSION`
file. Versions change only in the release PR (see the release section of `AGENTS.md`); the bump (**patch** for fixes and docs,
**minor** for a new skill or hook or a changed contract, **major** for a breaking change) is chosen when releasing.

## [Unreleased]

- Changed: the Shape Board keeper (`Pb Bot Storehouse`) and its UI reviewer run on demand, a fresh session per batch, stopped when they send `KEEPER-DONE` or `REVIEWER-DONE`, instead of living for days. Both run Sonnet 5.5 at effort high, and the keeper runs `ui-designer` by its agent type, not `general-purpose`. tokscope, 2026-11-22 to 25: the long-lived keeper cost about $99 (Opus context 500k to 850k) and the reviewer about $38. `podbay.md` gains "Project roles started on demand": the orchestrator starts such a role when a request finds no session of it. `fleet-settings.py` accepts `--role keeper`. New brief: `apps/pod-shape/docs/ui-reviewer.md`. A new keeper sends any `UI-READY` it finds missing (merged request issues without a `UI-READY sent` comment).

## [0.6.1] — 2026-11-25

- Changed: sessions compact sooner. `autoCompactWindow` is 300000 in this repo's `.claude/settings.json` and in what `pd-setup` writes (`merge-trim.sh --compact-window 300000`): the orchestrator and owner sessions compact at about 267k. It is 500000 in `fleet-trim.json` for workers and reviewers, which lose more when compacted mid-task. In tokscope, the 4 to 5 longest sessions per role carry most of the cost above 267k (worker 69%, orchestrator 71%); probed with `/context` on Claude Code 2.1.332: the window went from 1M to 300k with a 33k buffer. tokscope, 2026-11-20 to 25: the longest orchestrator session averaged 506k context per request, peak 966k.
- Added: the orchestrator ledger keeps a `next` per open ticket, is read first at session start as well as after a context summary, and has a backup in the checkout's git directory (`plugins/pd-bot/scripts/ledger-backup.py`, `--restore` when the ledger is missing; it survives `git clean`). The orchestrator stays in its session after a compaction instead of starting a new one.
- Changed: waiting is message-driven. `pd-orchestrate` (`podbay.md`, Wait) and `pd-work` wait with `notify_when_idle` and `Yield`, never a `ScheduleWakeup` or `/loop` cycle; a wake-up is only a fallback heartbeat of 1200 s or more. Each wake-up re-reads the whole context: tokscope, 2026-11-20 to 25, one loop session woke 151 times at 600 s with about 454k context.
- Changed: the fleet reviewer session runs Sonnet 5.5 at effort `high` for depth S and M; Opus 5.5 stays for depth L, a risky ticket (hooks, security, a public contract) and the whole-feature review. tokscope, 2026-11-20 to 25: 28 Opus reviewer sessions cost about $125 at list price. `fleet-settings.py --effort low|medium|high|xhigh` sets the session's `effortLevel` (Claude Code setting; `--restore` puts the owner's value back), so a Sonnet effort of `high` can be set for a worker or a reviewer.
- Changed: a fleet worker at depth S or M runs Sonnet. `fleet-settings.py` refuses any model that is not Sonnet or Haiku (Opus, Fable, or an alias such as `best`) for `--role worker` unless `--depth L` or `--escalated` (a Sonnet worker already failed once on the ticket). tokscope, 2026-11-20 to 25: 9 Opus workers cost $192 against $11 for 14 Sonnet workers.
- Added: token trim. `plugins/pd-bot/scripts/fleet-trim.json` sets `disableClaudeAiConnectors` and turns off the `render`, `slack`, `google-multi-account` and `chrome-devtools-mcp` plugins. `fleet-settings.py` writes it into each fleet workspace and `--restore` puts the owner's values back (`--keep-connectors` skips it); `pd-setup` step 7 offers it to a project (`merge-trim.sh`); this repo's `.claude/settings.json` (now tracked) turns off the same plugins and denies every claude.ai connector except Claude Docs and Context7, so owner sessions keep them (probe with Claude Docs alone: 38 tools, 37k start context). Fleet workers keep the `context7` plugin. A headless probe on Claude Code 2.1.332 went from 691 tools and 53k start context to 27 tools and 31k. The `env` key `ENABLE_CLAUDEAI_MCP_SERVERS` in a settings file does not do this; only the process environment or the setting does.
- Changed: `pd-review` runs `/code-review` first, before any test run or other read, at `low` (depth S) or `medium` (depth M), higher only at depth L or on a risky ticket, and runs test commands through the `pd-bot:test-runner` agent so their output stays out of the reviewer's context. `/code-review` runs as a fork of the session, so its cost follows the context at the time of the call. tokscope, 2026-11-20 to 25: reviewer sessions ran it inline at about 254k context per request (857 requests, about $80 at list price).
- Security (changed how the owner approves an ADR): `pd-shape` no longer pre-allows wide rules (#123). Its `allowed-tools` had `Bash(git -C *)`, `Bash(git push *)`, `Bash(git commit *)`, `Bash(git add docs/adr*)`, `Bash(git log *)`, `Bash(git fetch *)`, `Bash(git worktree *)`, `Bash(gh pr create *)`, `Bash(gh issue comment *)`, the `write_adrs.py *` rule and a bare `Read`, `Write` and `Edit`. A `*` matches any extra arguments, and a typed slash command applies a skill's `allowed-tools` even in `dontAsk`. Probed with haiku on Claude Code 2.1.330: `git -C <dir> -c core.fsmonitor=<cmd> status` ran `<cmd>`, `git log --output=<file>` wrote a file, and a bare `Read`/`Write` read and wrote files outside the repo, all with no prompt. So prompt injection in a prototype or an issue could run commands, push to `main`, or read files such as `~/.ssh`. `pd-shape` now pre-allows only `Read(./**)`, `Read(~/.claude/plugins/cache/*/pd-bot/**)` (its own references; another plugin's files are denied), `Grep(./**)`, `Glob(./**)`, `Edit(./specs/**)` (this also covers Write), `Agent`, `AskUserQuestion` and `Bash(gh issue view *)`. In the ADR step, the owner now sees one permission prompt for each git, `gh` and `write_adrs.py` command (about 8 per ADR), not one yes for the whole step, and sees the staged ADR diff before `git commit` and the comment text before `gh issue comment`. Under `dontAsk` with no `--allowedTools` for them, those commands are denied.
- Security: other skills lose wide rules that they did not need. `pd-compound` and `pd-asd-ste100` replace `Bash(python3 *)` with their own scripts. `pd-xia2`, `pd-intent-review`, `pd-intake`, `pd-release-notes` and `pd-session-retro` drop `Bash(git log|show|diff|branch|describe *)`: Claude Code runs plain read-only git with no rule and refuses its writing flags, which those rules let through. `pd-work` and `pd-review` pre-allow `Bash(bash scripts/run-tests.sh)` exactly; the old `run-tests.sh*` also matched other files.
- Added: `scripts/check_allowed_tools_wide.py`, run by `run-tests.sh`, fails when a skill pre-allows a wide rule: bare `Bash`, `Read`, `Write`, `Edit`, `Grep` or `Glob`; any `git … *`; an interpreter followed by `*` or an option (`python3 *`, `python3 -c*`); a `*` joined to a word; and any other Bash rule with a `*` unless it is `ls`/`test`, a `gh <group> view|list|diff|checks *`, or an interpreter plus a script path under `${CLAUDE_PLUGIN_ROOT}/` or `${CLAUDE_SKILL_DIR}/`. A file rule must be a `./` path, and a write rule (`Edit`, `Write`) is narrow only under `./specs/` or `./docs/` and never on a `package.json`; every other folder can hold a file that runs (CI, hooks, scripts, a SKILL.md, `.husky`, settings). A quoted, list, multi-line or repeated `allowed-tools` value fails, and so does a frontmatter with a control or Unicode line-separator character or a SKILL.md with no frontmatter. WebFetch, WebSearch, Agent, Task, Skill, SendMessage and MCP tools are not judged yet (known limit). The wide rules that existed before are listed in `scripts/allowed-tools-wide-baseline.json` for the owner to decide (the fleet skills' `Bash(git *)`, `pd-orchestrate`'s `gh issue *`, `gh pr *`, `tmux send-keys *`, `podbay *` and `podbay-dev *`, `pd-release-notes`' `mkdir *`, `pd-tickets`' create rules, and bare `Bash`, `Read`, `Write` and `Edit` in most skills). That file can only shrink: an entry that `origin/main`'s copy does not list fails, and the check fails when git or `origin/main` is missing (only a base that has no baseline file yet passes, with a note). `pd-shape` can never be in it. `check_skill_tool_conformance.py` accepts read-only git with no grant and reads a `<!-- pd-bot:owner-approves … -->` marker in a SKILL.md body for commands that the skill leaves to the owner's prompt on purpose.
- docs(readme): match the README to the current codebase (PR #62)
- Add the Shape Board step 0 spike for the p2g-questions protocol (PR #63)
- Set up Pb Bot Storehouse, the Shape Board UI design-system keeper (PR #66)
- Shape Board Cozy redesign: design system v2.0 (PR #67)
- [UI] Badge (closes #71) (PR #80)
- [UI] Tabs (closes #68) (PR #79)
- feat(ui): add Banner part (warning notice), closes #76 (PR #83)
- [UI] DropZone (closes #75) (PR #82)
- [UI] PipelineSteps (closes #70) (PR #84)
- [UI] Textarea (closes #73) (PR #81)
- [UI] ArtifactPreviewFrame (closes #77) (PR #85)
- [UI] Select (closes #72) (PR #86)
- [UI] ArtifactCard (closes #69) (PR #89)
- [UI] NewArtifactsBox (closes #74) (PR #90)
- [UI] AddArtifactDialog (closes #78) (PR #91)
- [UI] NewArtifactsBox labels (closes #93) (PR #94)
- [UI] Dialog fits the viewport; AddArtifactDialog nits (closes #92) (PR #95)
- [UI] Banner wraps children in a div, not a p (closes #87) (PR #96)
- [UI] Textarea mirror: drop left:-9999px, copy font longhands (closes #88) (PR #97)
- [DS] Design system v2.2 and the design PR merge rule (PR #98)
- Shape Board step 2: import artifacts from the hook, versions and sandboxed preview (PR #99)
- [UI] Type sizes match the prototype; ArtifactCard Preview icon (closes #101) (PR #112)
- [UI] Kbd (closes #103) (PR #114)
- [UI] PageHeader: optional steps slot (closes #108) (PR #116)
- docs(pod-shape): §2 type table and HANDOFF follow v2.3 (PR #120)
- [UI] Textarea minRows; AddArtifactDialog HTML starts at 8 rows (closes #102) (PR #111)
- [UI] BriefCard (closes #105) (PR #113)
- [UI] SessionList and SessionStatus (closes #106) (PR #115)
- [UI] RightPanel and ReadyBox (closes #107) (PR #118)
- [UI] ChatMessage and ChatNote (closes #104) (PR #117)
- [UI] SuggestedAnswers (closes #109) (PR #121)
- [UI] Composer (closes #110) (PR #125)
- Shape Board: step 2 follow-ups from the Mac check (#100) (PR #129)
- CI: run the Shape Board app checks (PR #127)
- SuggestedAnswers: ignore keys on combobox/menu/tab, drop empty-header separator (closes #126) (PR #130)
- [DS] Design system v2.4 (Grill parts) and the clean-up rule (PR #128)
- pd-shape: no wide allowed-tools; CI gate for wide rules (#123) (PR #132)
- Shape Board step 3: the Grill screen (PR #133)
- Docs: find the keeper and the UI reviewer by name prefix (PR #135)
- PR rule: a UI change needs before/after screenshots on the PR (#154) (PR #155)
- Quota fixes: Sonnet for S/M workers and reviewers, token trim, review order, wait rules, context ceiling (PR #161)

## [0.6.0] — 2026-11-22

- Changed: one skill, `pd-shape`, replaces `pd-grilling`, `pd-own` and `pd-brainstorming`. It grills each unclear requirement and brainstorms each approach with two or more viable options in one loop. On at most 3 big decisions and on every approach choice the owner answers first, then sees the agent's recommendation and a counter-argument. For a finalized prototype it checks 8 topics a prototype cannot show (goal, rules, edge cases, data, permissions, non-functional, out of scope, priority) and writes `specs/<slug>/brief.md` for `pd-tickets`; the owner confirms the brief before anything else happens. It proposes an ADR only for a repo-level decision (hard to reverse, beyond the feature, two real options) and, after the owner says yes, opens an ADR-only pull request. At depth M and L it writes `specs/<slug>/design.md` in interactive and in auto mode (depth L also the design ADR). `pd-intake` (GRILL), `pd-orchestrate`, `pd-work` and `pd-advise` now name `pd-shape`. Dropped from `pd-own`: the teach-back step.
- Removed (breaking): pd-bot runs only with Pod Bay, and `/pd-bot:pd-orchestrate` (the fleet) is the only way to run tickets with agents (`docs/adr/0005-pd-bot-runs-only-with-pod-bay-and-the-fleet-is-the-only-run-path.md`). Removed `workflows/ship.js`, the `pd-autopilot` skill (profile, preflight, board start), the run board `plugins/pd-bot/board/`, `run_report.py`, `ship-worktree-cleanup.sh`, the `pd-using-git-worktrees` skill and their tests. `plan_groups.py` and `groups.md` moved to `pd-orchestrate`. `pd-implement` runs its steps one at a time on the working branch instead of one git worktree per step. `pd-tickets` names `/pd-bot:pd-orchestrate` as the next command. The Pod Bay session board is `apps/pod-board/`.
- Added: Pod Board marks a session that finished. A session that worked for at least 2 minutes and then stays idle for 8 seconds gets one notification with no sound, a green `finished` tag at the top of the Idle lane, a line in the summary row, a group in the menu bar popover and `✓n` beside the menu bar icon. The mark goes when the owner opens the session, peeks at it or clicks its notification, when it works again, or after 12 hours.
- Added: Pod Board cards show a session's pull requests, issues and artifacts as buttons that open the page in the browser. The app opens only an `https://` address that the current Pod Bay read holds.
- Added: a click on a Pod Board notification opens its session in the Claude app. Checked with real clicks on macOS.
- Changed: `merge-guard.sh` is rewritten around "doubt means ask" (ADR 0004): 63 lines instead of 407, no bash parser. A plain `gh pr merge <n>` or `glab mr merge <n>` whose base is `feat/*` runs; every other merge-like command, a `git push` that names main or master, and an MCP merge tool (new `hooks.json` matcher) ask the owner. In a subagent or workflow agent an ask is a denial. Only `feat/*` passes without a question (the protected-base list is gone), `--match-head-commit` is no longer required, and a base that cannot be read in 15 s asks. Hidden merges (escapes, scripts, interpreters) are out of scope. A text that holds the word merge in a gh command asks; put it in a file (`git commit -F`, `--body-file`). ship.js now opens each PR with `gh api repos/<o>/<r>/pulls -X POST --input <json file>` (title, branch and body in the file, no command substitution) and posts the verify comment with `--body-file`, and merges the final PR on its own only into a `feat/*` base.
- feat(pod-board): mark finished sessions, link buttons, notification click (PR #56)
- feat(hooks): merge-guard v2, doubt means ask (ADR 0004) (PR #57)
- docs(review): out-of-scope hidden-command guard bypasses (after #57) (PR #58)
- chore!: remove ship.js, pd-autopilot, the run board and git worktree isolation (PR #60)
- feat(skills): merge grilling, own and brainstorming into pd-shape (PR #59)

## [0.5.0] — 2026-11-21

- Added: the board's `/fleet` page and `/fleet.json` show a milestone ledger's `scenarios`, `feature_pr` and `test_rounds` (`null`, `null` and `[]` when absent) (#16). The `pd-orchestrate` line of the skills map shows the reviewer's `check-request`/`check-result` path and the milestone flow (#15). The `session-role.sh` row of the CLAUDE.md hooks table says that `PD_BOT_ROLE=orchestrator` gives the role and any other non-empty value gives none (#18). These come from the ship.js comparison round (#30).
- Added: `fleet-settings.py` sets a fleet session's model, permission mode and role (`PD_BOT_ROLE`, `PD_BOT_TICKET`, `PD_BOT_RUN`) in the workspace checkout's `.claude/settings.local.json` before `podbay-dev start`, and `--restore` puts back only those keys (their old values are kept in the git directory, never in the work tree), so a rule the session saved meanwhile stays. It keeps every other key, refuses a file git does not ignore, and never sets `bypassPermissions`. Checked in a real Pod Bay session: it started on Haiku 5.5 in auto mode with the role variables set. `podbay.md` uses it instead of typing `/model` into the session, which also changed the machine's default model.
- Changed: the fleet orchestrator no longer runs tests or checks in its own session; their output filled its context. After a `pass` it sends `check-request` to the ticket's reviewer session, which runs the acceptance check and `fleet-check.sh` in block mode (`pd-review/references/checks.md`) and answers with a short `check-result`. A milestone's feature test runs in a tester session of its own (`feature-test-request`, `feature-test-result`).
- Added: `pd-orchestrate/references/milestone.md`, the milestone run on one feature branch: a scenario page (an artifact only the orchestrator edits, with a reason per change), a draft feature pull request into `main` once the first ticket is in, a whole-feature review, a feature test of every scenario in a tester session, all bug tickets of a round created before any fix, at most 2 fix rounds (test, fix, test, fix, test), the label `pd-bot:owner-merge` on a fix for a requirement that was not clear, and the owner merges into `main`. `pd-orchestrate` may use the Artifact tool.
- Added: `fleet-check.sh` checks the reviewer's intent verdict (`intent.verdict` in the receipt must be `match`) and the report's `needs_review` decisions (a mismatch unless its title is in the `--decisions-seen <file>` list of titles the owner saw). The receipt gains an `intent` object; `match` with a non-empty `gaps` list is a mismatch. `fleet-check.json` now records the `--base` it checked.
- Added: `fleet-gate.sh` decides whether a fleet ticket pull request may go into its `feat/*` branch without the owner: a block-mode `fleet-check.json` for the head with the suite run and passed, green CI, and an open pull request on exactly the run's `feat/<slug>` (`--base`) at that head with no `pd-bot:owner-merge` label, where the checked base is in the feature branch's history (so no unchecked commit goes in). The orchestrator puts that label on a fix where the worker decided a requirement that was not clear. It changes nothing; on a pass it prints the one `gh pr merge` command, which `merge-guard.sh` still judges. `pd-orchestrate` can now run a milestone on a feature branch.
- Changed: the `pd-orchestrate` orchestrator now removes the worker and reviewer workspaces of a finished ticket itself, instead of leaving every workspace for the owner (a fleet run left `src-01`..`src-16`). A workspace qualifies when the ticket's PR is MERGED or CLOSED, the review receipt is saved, no other ticket uses it, and no session from outside the fleet runs there. The orchestrator runs `podbay-dev stop <name>`, then `podbay-dev ws-rm <project> <src-NN>`. A refused `ws-rm` (work not pushed) keeps the workspace with the reason in the ledger; it is never forced or deleted by hand, and `src` is never removed. The ledger records each workspace as removed or kept, and the final report lists each kept one with its reason. New contract test `tests/scripts/orchestrate-cleanup-contract.test.sh` (P11).
- feat(pd-orchestrate): the orchestrator removes a finished ticket's workspaces itself (P11) (PR #49)
- feat(fleet): milestone runs on a feature branch (PR #12)
- cmp-ship-r1: tickets #15, #16, #18 (cherry-picked from #30) (PR #54)

## [0.4.0] — 2026-11-21

- Fixed: `ship.js` never removed its stage worktrees, so each run left a full checkout and build per stage (about 170 GB across four Mnemo checkouts). At the end of a run that succeeded, the new `scripts/ship-worktree-cleanup.sh` removes the run's detached stage worktrees with `git worktree remove` (never forced). It keeps the ticket-branch worktrees, a worktree with changes or untracked files, and one whose commits are on no branch, then removes the run dir if it is empty. A blocked or failed run keeps everything. The result's new `cleanup` field and a `worktrees:` log line list what was kept, its size and the exact removal command, and the pd-autopilot completion report prints them. `clean-worktrees.sh` is unchanged (P10).
- feat(pod-board): peek at a session's last reply; a menu bar popover (PR #28)
- docs(pod-board): a short guide to what it is, how to build and use it (PR #29)
- feat(pod-board): collapse and expand the sidebar (PR #34)
- fix(ship): remove a successful run's detached stage worktrees; report what is kept (P10) (PR #33)

## [0.3.0] — 2026-11-21

- Fixed: two tests that failed on a busy machine (P7). The clean-worktrees "item 2" time-budget case now uses a 10s budget, because the scan of every process's working directory alone took over 3s with about 2000 processes. The board-start `--no-open` case now waits up to 10s for the board to answer, because node can start after the 3s that `board-start.sh` waits. Tests only; no hook or script changed.
- Fixed: the pd-autopilot preflight step was denied by `merge-guard.sh` when an agent appended `; echo "exit=$?"`: the letters gh (in "preflight"), pr (in "myprojects") and the `$` match the guard's pre-filter. `SKILL.md` step 1 and `references/preflight.md` now say to run the command exactly as written, with nothing appended. When the denied command holds no merge, the wrapper denial now says it matched the letters g-h..p-r..$ instead of "mentions merge in text", with the same workaround. What the guard denies is unchanged (P9).
- test: make the time-budget and board-start --no-open cases hold on a busy machine (P7) (PR #8)
- feat: Pod Board desktop app for Pod Bay sessions (PR #24)
- feat(board): Pod Bay sessions view at /pods (PR #9)
- fix(merge-guard): say what matched when a denied command holds no merge; run preflight as written (P9) (PR #25)

## [0.2.0] — 2026-11-21

- `merge-guard.sh` now denies spellings that bash turns into `gh`, `merge` or a wrapper: brace expansion, a glob or brace in the gh words, octal, hex and unicode escapes (decoded before matching), `GH`/`Gh` and `Bash`/`SH`, a gh name built from variable slices, and any merge-like command with a backslash line break. Without jq it checks the command field only (P5).
- Added: the `session-role.sh` SessionStart hook tells the Pod Bay session of the main workspace (`src`) that it is the orchestrator: it checks, coordinates and delegates hands-on work. Sessions in `src-NN` and sessions outside Pod Bay get nothing. Switch it off with `{"orchestrator": "off"}` in `.claude/pd-bot.json` or `PD_BOT_ROLE=none`; `/pd-bot:pd-setup` asks about it.
- Changed: `pr-opened.sh` tells the orchestrator session to hand the `/code-review` to another session; every other session keeps the old reminder.
- Fixed: `pr-opened.sh` fired on any command that held the text `gh pr create` (a grep, a scan, a failed create). It now fires only when `gh pr create` is a command word (not text inside an argument) and the output holds a pull request URL. A script that runs `gh pr create` inside it gets no reminder.
- Changed: `docs/adr/` is tracked again and starts at ADR 0001; the rest of `docs/` stays local.
- `merge-guard.sh`: the wrapper denial names the workaround (put the command in a script file and run it by a path without the word). Prose that mentions merge next to `bash x.sh` stays denied: two security reviews broke both fixes tried; the bypasses they found are now regression tests (P1).
- A PR that changes the FE/UI (ticket kind FE or FE+BE, or a diff touching `.tsx .jsx .vue .svelte .css .scss .html`) needs a screenshot. Without one the PR still opens and the merge becomes `Merge: human (no UI screenshot attached)`: in `ship.js` (the final merge reason; a ticket is never blocked), `pd-finishing-a-development-branch` the fleet outcome contract and the `pd-work` toolbox (P2).
- Require a screenshot on every FE/UI PR, else Merge: human (P2) (PR #3)
- merge-guard: name the workaround in the wrapper denial; regression tests for 11 bypasses (P1) (PR #4)
- feat(hooks): give the Pod Bay main-workspace session the orchestrator role (PR #5)
- merge-guard: deny spellings that bash turns into gh, merge or a wrapper (P5) (PR #6)

## [0.1.0] — 2026-11-20

- Initial release of the `harness` plugin: skills, agents, hooks and the `autopilot-run` ship workflow.
