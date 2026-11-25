# Shape Board (`apps/pod-shape`)

Personal macOS desktop app (Tauri 2 + React): grill a prototype, then turn it into GitHub milestones and issues.
Read `docs/HANDOFF.md` first. The build plan is §6 there; one step per PR.

## Run and build

- `npm install`, then `npm run tauri dev` for the dev app (Vite on port 1430).
- `npm run tauri build` makes `src-tauri/target/release/bundle/macos/Shape Board.app`.
- `cargo test --manifest-path src-tauri/Cargo.toml` runs the Rust tests (SQLite migrations, project checks).
- `npm run typecheck` runs TypeScript.

## Data

- SQLite at `~/Library/Application Support/io.pdbot.shapeboard/shape-board.db`.
- Migrations are `src-tauri/migrations/NNNN_*.sql`, listed in `src-tauri/src/db.rs` (`MIGRATIONS`);
  `PRAGMA user_version` counts how many ran. Add a new file for a change; never edit one that shipped.

## Fonts

- Outfit (UI, 400/500/600) and JetBrains Mono (real code only, 400/500) come from `@fontsource/outfit`
  and `@fontsource/jetbrains-mono` (both OFL), imported in `src/styles/globals.css`. Bundled, no CDN.
- `lucide-react` is pinned to 0.577.x: 1.x removed the `Github` icon that `icons.ts` maps to `openGithub`.


## UI design system (must follow)

The UI follows `docs/DESIGN_SYSTEM.md` and `src/styles/tokens.css`. Read both before any UI change.

**Before UI work, ask the keeper**, the session whose name starts with `Pb Bot Storehouse` (for example `Pb Bot Storehouse #UA-01`): `ListAgents`, copy its name exactly as the row prints it, then `SendMessage` a `UI-ASK` (forms in `docs/storehouse-keeper.md`, flow in DESIGN_SYSTEM.md §11). The keeper runs on demand: when no row starts with `Pb Bot Storehouse`, send the `UI-ASK` to the orchestrator instead (§11 says what to do with no orchestrator). Design pull requests are reviewed by the `Pb Bot UI Reviewer` session (`docs/ui-reviewer.md`), also started on demand. Do not build or style a missing part in a screen; continue that part only after `UI-READY`.

Rules:
- Use only the tokens, components (§6), layouts (§7) and lucide icons (§8) listed there.
- Do NOT create a new color, font size, spacing, radius, shadow, animation, component, or component variant. Do NOT add another UI or icon library.
- No hex/rgb values, no arbitrary Tailwind values (`bg-[#..]`, `p-[13px]`), no default Tailwind colors (`bg-blue-500`, `text-white`) in components.
- If the task needs something that is not in the design system: STOP and ask me first. Say what is missing, which existing piece is closest, and the smallest addition you suggest. After I say yes, add it to `tokens.css` or `src/components/ui|app/` AND update `docs/DESIGN_SYSTEM.md` in the same change.
- Every UI change must work in dark and light theme, show issue status with icon + label (not color only), and pass the checklist in DESIGN_SYSTEM.md §10.
- Never use the banned patterns in DESIGN_SYSTEM.md §0.1: no gradients, no blur/glass/glow, no shadow that is not a token, no sparkles or wand icons for "AI", no emoji, no card inside a card, no colored left border on cards, no blobs/waves/decorative backgrounds, no centered hero layouts, no uppercase labels, no marketing filler words, no lorem ipsum. Pills, rounded cards and the shadow tokens of §5 are the v2.0 "Cozy" look and are allowed.
- Icons only through `src/components/app/icons.ts` (one meaning = one icon, fixed size/stroke pairs). Never import from `lucide-react` directly.
- Keep accent near 10% of a screen: one primary button per view; the orange node means only "you are here" (current step or selected item).
- The branch line (DESIGN_SYSTEM.md §6.4) is the only signature detail. Use it only in TreeView, PipelineSteps, LevelsLegend and EmptyState's BranchPattern.
- Mono (JetBrains Mono) only for real code: RunLog rows, the brief source, inline code. Paths, repos and `#numbers` use the UI font with `tabular-nums`.
- Do not redraw or recolor the logo. Use the files in `src/assets/brand/`.

Where things live:
- Tokens: `src/styles/tokens.css` (imported after `@import "tailwindcss";` in `src/styles/globals.css`)
- Primitives (shadcn/ui): `src/components/ui/`
- App parts: `src/components/app/`
- Icon map: `src/components/app/icons.ts`
- Fonts: Outfit + JetBrains Mono, bundled via `@fontsource/outfit` and `@fontsource/jetbrains-mono` (no CDN; the app works offline)
- Logo: `src/assets/brand/shape-board-icon.svg`, `shape-board-lockup.svg`, `branch-pattern.svg`
