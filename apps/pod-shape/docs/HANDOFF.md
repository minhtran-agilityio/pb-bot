# Shape Board — Hand-off for the build session

Version 2, 2026-11-22. Read all of this before you do anything. Do not write code until step 0 (the spike) is done and the owner has seen the result.

## 0. What is in this bundle

You received `shape-board-handoff.zip`. Unzip it **outside** the repo first (for example `~/Downloads/shape-board-handoff/`). Do not commit anything until the owner confirms the target folder (section 2).

| Path in the zip | What it is | Where it goes later (inside the app folder) |
| --- | --- | --- |
| `HANDOFF.md` | This file | `docs/HANDOFF.md` |
| `prototype/shape-board-prototype.html` | Clickable UI prototype, one HTML file. Open it in a browser | `docs/prototype/` (reference only, not app code) |
| `design-system/DESIGN_SYSTEM.md` | Design system v1.1, the UI contract | `docs/DESIGN_SYSTEM.md` |
| `design-system/tokens.css` | Tailwind v4 tokens | `src/styles/tokens.css` |
| `design-system/icons.ts` | The only allowed icon map | `src/components/app/icons.ts` |
| `design-system/CLAUDE.md.snippet.md` | Rules for coding agents | Paste into the app's `CLAUDE.md` |
| `design-system/preview.html` | Visual reference of every token and component | `docs/` (reference only) |
| `design-system/anti-slop-report.html` | Why v1.1 bans some patterns | `docs/` (reference only) |
| `design-system/assets/` | Logo SVGs + `branch-pattern.svg` | `src/assets/brand/` |
| `logos/final/` | Final logo: icon SVG, 1024 PNG (for `tauri icon`), lockup SVG/PNG | `src/assets/brand/` (+ run `tauri icon` on the PNG) |

**Fonts are NOT in the bundle.** See section 7.

## 1. What to build

**Shape Board** is a personal desktop app that runs only on the owner's Mac. It takes a prototype (a Claude artifact) through a "grill" step and then creates GitHub milestones and issues from it.

The flow: add a repo, paste an artifact, grill it, generate issues, then see everything in one tree.

- The tree: Project (= 1 GitHub repo) → Artifact → Feature (= 1 brief) → Milestone → Issue.
- Issue status is read live from GitHub: open, running, has PR, merged, closed.
- The owner works on many projects. The tree lets them check status fast without opening the Claude desktop app.

Sources of truth:

- **Design doc (architecture, protocol, data model, risks):** https://claude.ai/artifact/K47EPMgWrZgCv37Z7zSzyH (Vietnamese).
- **Design system v1.1:** https://claude.ai/artifact/WRkcjobJX9VCEi74o3mVjN (same as `design-system/` in the zip).
- **Clickable prototype:** https://claude.ai/artifact/FVACosM4PvP3syMpk9ngc1 (same as `prototype/shape-board-prototype.html`). It shows every screen and flow with fake data. Copy its screens and behavior. Do not copy its code: it is a single esbuild bundle, and its source files were not kept.

If the design doc and the design system disagree, follow the design doc for behavior and the design system for looks. One known conflict is listed in section 5.

## 2. Where the code goes, and access

- Repo: https://github.com/minhtran-agilityio/pb-bot (public). The owner's local checkout is at `~/Documents/agilityio/pb-bot` on their Mac.
- **Folder, still to confirm:** the owner said `app/pod-shape`. pb-bot already has a Tauri app at `apps/pod-board` (Pod Board), so the owner may mean `apps/pod-shape`. **Ask before you create the folder.** You can copy Pod Board's Tauri build setup.
- **Push access:** the cloud GitHub account (`minhtran3124`) cannot push to pb-bot. Pushes and PRs go from the owner's Mac (directly, or through Remote Control).
- The spike (step 0) must run on the owner's Mac. It needs `claude` logged in with the owner's subscription, `gh` logged in, and the pd-bot plugin with `pd-shape` installed.

## 3. Agreed decisions (do not reopen)

| Topic | Decision |
| --- | --- |
| Platform | Tauri 2: Rust backend + web UI |
| UI stack | React + TypeScript + Tailwind CSS v4 + shadcn/ui (Radix) + lucide-react through `icons.ts` |
| Storage | SQLite on the local machine. Save the full grill history |
| Claude access | Use the owner's Claude subscription. The app runs the Claude Code CLI in the background: `claude -p` with `--session-id` on the first turn and `--resume <id>` after that, plus `--output-format stream-json --verbose`. No API key |
| GitHub access | `gh` CLI, already logged in. The app stores no tokens |
| Getting artifacts | No official API exists for claude.ai Projects or artifacts. The terms do not allow an app to read claude.ai cookies or sessions. The owner pastes the artifact link and its HTML, and the app stores a copy. Pasting again creates a new version |
| Grill logic | Reuse the `pd-shape` skill from pd-bot. Do not rewrite its rules in Rust. The app disables `AskUserQuestion` and appends a system prompt. Claude then prints questions as a fenced `p2g-questions` JSON block (same shape as the AskUserQuestion input). The app shows them as cards and sends the answers back with `--resume` |
| Grill output | 1 grill session = 1 feature = 1 brief (`specs/<slug>/brief.md`, local scratch, not committed). The owner confirms the brief in the app |
| Issue creation | Run `/pd-bot:pd-tickets specs/<slug>/brief.md`. Its single owner approval becomes the review screen. After creation, the app finds the issues by their `source:` line and stores the links |
| Ticket rules | A new feature with 2 or more issues gets a milestone. A small update is 1 issue. Brief content goes into each issue, and the brief is not committed |
| ADR rules | ADRs are repo-wide and rare. A decision needs an ADR only if it is hard to reverse, affects architecture/data/security/users beyond the feature, and has 2 or more real options. The owner decides. ADRs go to main via a PR |
| Not in V1 | No "run podbay" button. No auto mode. No sync across machines. No fetching from claude.ai |
| Prototype preview | Use an iframe with `sandbox="allow-scripts"` and without `allow-same-origin`, so the pasted prototype cannot reach Tauri commands |

## 4. Open questions: status at hand-off

**The owner has not confirmed any of these.** The clickable prototype picked the recommended option for each, so you can build on that default. Ask the owner before you build the part that depends on it.

| # | Question | Default the prototype uses | Status |
| --- | --- | --- | --- |
| A | Target folder: `app/pod-shape` or `apps/pod-shape`? | — | **Ask first. Blocks step 1** |
| 1 | macOS only for V1? | macOS look (traffic-light title bar) | Not confirmed |
| 3 | When `pd-shape` asks "Make this an ADR?" and the owner says yes, may the app let Claude commit, push and open a PR? | ADR step not shown | Not confirmed. Recommendation: yes, with git limited to `docs/adr/` + `gh pr create` |
| 4 | Commit the prototype HTML to `prototypes/<slug>.html` on branch `prototype/<slug>` when generating issues, so issues can link to it? | A checkbox in the Create dialog, on by default | Not confirmed. Recommendation: yes, and show the command before pushing |
| 5 | How does pd-bot mark an issue as "running"? | "Running" status kept | Not checked. Read the pb-bot repo. If there is no clear signal, drop "running" from V1 and tell the owner |
| 2 | React or Svelte? | React | Settled by the design system |
| 6 | Separate repo? | — | Settled: code lives in pb-bot |

## 5. Known conflict

When is "Generate issues" enabled? Design system §7.1 says "after at least one Q&A". The design doc says "after the owner confirms the brief". The prototype follows the design doc. **Follow the design doc**, and propose a one-line fix to `DESIGN_SYSTEM.md` §7.1 in your first UI PR. The owner must approve design system changes.

Other prototype choices: the artifact preview is a Dialog, not its own screen. App state in the prototype uses browser localStorage, but the real app uses SQLite.

## 6. Build plan: spike first

Each step is 1 small PR with a clear done point. Do not start step 1 until step 0 passes and the owner has seen the result.

0. **Spike (no UI).** Write a small script that runs `pd-shape` through `claude -p` and `--resume` on one real prototype, using the `p2g-questions` protocol.
   - Done when 3 runs in a row each produce valid `p2g-questions` JSON and end with a `brief.md`.
   - Also check that `/pd-bot:pd-shape` and `/pd-bot:pd-tickets` work as slash commands in `-p` mode.
   - If the spike fails, report to the owner before going on. The fallback is a small `Mode: app` section in `pd-shape`.
   - Report the result as an artifact (owner's standing rule).
1. **App skeleton.** Tauri 2 + React, the design system files in place, fonts, SQLite + migrations, the Projects screen, adding a project (check that `gh`, `claude` and the repo folder are present). Done when a repo is added and still shows after the app restarts.
2. **Artifact.** Paste link + HTML, store versions, preview in the sandboxed iframe. Done when a real prototype runs inside the app.
3. **Grill.** `claude_runner`, the Grill screen, question cards, Stop/Resume, saved history, brief confirmation. Done when a real prototype is grilled end to end, and an interrupted session can be resumed after closing the app.
4. **Generate issues.** Run `pd-tickets`, show the review screen, create the issues, store the links. Done when real issues appear on GitHub and in the tree.
5. **Tree + status.** The full tree, status from one GraphQL call per project, progress bars. Done when 2 projects with issues in different states show correctly.

## 7. Design system v1.1, fonts and logo (must follow)

- Read `DESIGN_SYSTEM.md` fully before any UI work. Paste `CLAUDE.md.snippet.md` into the app's `CLAUDE.md`.
- Use only its tokens, components (§6), layouts (§7) and icons (§8). **Do not invent** a color, size, spacing, radius, shadow, component or variant. No hex or arbitrary Tailwind values. If something is missing, stop and ask the owner first.
- v1.1 rules: warm-tinted neutrals (not zinc); orange accent near 10% of a screen, one primary button per view; the branch line is the only signature detail (TreeView, PipelineSteps, EmptyState); banned patterns in §0.1 (no gradients, blur, glow, sparkles/wand icons, emoji, blobs, card inside card, centered hero). Icons only through `icons.ts`, never directly from `lucide-react`.
- Status = color + icon + word. Dark and light themes. Run the §10 checklist before you call a UI change done.
- **Fonts (not in the bundle).** The cloud session could not reach Fontshare, so no font files were made. Do not create or fake font files.
  - **Switzer** (UI font, weights 400/500/600): download from Fontshare, https://www.fontshare.com/fonts/switzer , as WOFF2. Put the files in `src/assets/fonts/` (the app must work offline). **Check the Fontshare / ITF Free Font licence** before release, and tell the owner what it says.
  - **JetBrains Mono** (code font): `npm i @fontsource/jetbrains-mono` (OFL licence).
  - Fallback order: Switzer, then Inter, then system UI. The prototype shows Inter only because artifacts can load only Google Fonts.
- Logo: option 1 (branch tree). Never redraw or recolor it.

## 8. Dependencies and risks

- **`pd-shape` is in PR #59:** https://github.com/minhtran-agilityio/pb-bot/pull/59 (branch `feat/pd-shape`). Check whether it is merged before the spike. If not, install that branch's plugin version on the Mac. Keep the app's protocol in the app's own system prompt and do not depend on `pd-shape` internals.
- `pd-tickets` and `pd-own` have `disable-model-invocation: true`. Only a typed slash command can start them, which is what the app does.
- Main technical risk: Claude may not follow the `p2g-questions` protocol reliably. This is why the spike comes first.
- **Terms risk:** Anthropic allows subscription login for "ordinary, individual usage" of Claude Code. A personal app calling `claude -p` probably fits, but the docs do not say so clearly: https://code.claude.com/docs/en/legal-and-compliance . Keep it personal. Do not distribute the app, and do not run bulk automation.
- Subscription usage limits can stop a grill mid-way. Show the real error, set the session to `failed`, and allow resume.
- Never use `--dangerously-skip-permissions`. Allow only the tools each step needs, with `--allowedTools`.

## 9. Working with the owner (KhoiTran)

- Software QA/engineer in Da Nang. Writes Vietnamese in chat, so reply in Vietnamese with short, plain sentences.
- **Ask before you build.** They dislike agents that rush ahead. If something important is unclear, ask a few short questions with your recommendation. Do not guess.
- Be direct. Point out weak assumptions and risks.
- **Every report is an artifact** (standing rule), for example spike results, reviews and status reports. Chat gets a short line and the link.
- Do not state today's date unless they confirm it. Cite sources for facts.

## 10. First actions for the new session

1. Read this file, then `DESIGN_SYSTEM.md`, then open the prototype HTML in a browser and click through it.
2. Ask the owner, in one short message: question A (folder), then 1, 3 and 4.
3. Check PR #59's state and how pd-bot marks a running issue (question 5) by reading the pb-bot repo.
4. Write the spike plan (step 0) and wait for the owner's "go".
