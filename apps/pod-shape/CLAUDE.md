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

- Switzer WOFF2 files in `src/assets/fonts/` (Fontshare, ITF Free Font licence: check it before any release).
  `src/styles/fonts.css` wires 400 `Switzer-Regular`, 500 `Switzer-Medium`, 600 `Switzer-Semibold`.
- JetBrains Mono comes from `@fontsource/jetbrains-mono` (OFL).
- `lucide-react` is pinned to 0.577.x: 1.x removed the `Github` icon that `icons.ts` maps to `openGithub`.


## UI design system (must follow)

The UI follows `docs/DESIGN_SYSTEM.md` and `src/styles/tokens.css`. Read both before any UI change.

Rules:
- Use only the tokens, components (§6), layouts (§7) and lucide icons (§8) listed there.
- Do NOT create a new color, font size, spacing, radius, shadow, animation, component, or component variant. Do NOT add another UI or icon library.
- No hex/rgb values, no arbitrary Tailwind values (`bg-[#..]`, `p-[13px]`), no default Tailwind colors (`bg-blue-500`, `text-white`) in components.
- If the task needs something that is not in the design system: STOP and ask me first. Say what is missing, which existing piece is closest, and the smallest addition you suggest. After I say yes, add it to `tokens.css` or `src/components/ui|app/` AND update `docs/DESIGN_SYSTEM.md` in the same change.
- Every UI change must work in dark and light theme, show issue status with icon + label (not color only), and pass the checklist in DESIGN_SYSTEM.md §10.
- Never use the banned patterns in DESIGN_SYSTEM.md §0.1: no gradients, no blur/glass/glow, no sparkles or wand icons for "AI", no emoji, no card inside a card, no colored left border on cards, no blobs/waves/decorative backgrounds, no centered hero layouts, no marketing filler words, no lorem ipsum.
- Icons only through `src/components/app/icons.ts` (one meaning = one icon, fixed size/stroke pairs). Never import from `lucide-react` directly.
- Keep accent near 10% of a screen: one primary button per view; the orange node means only "you are here" (current step or selected item).
- The branch line (DESIGN_SYSTEM.md §6.4) is the only signature detail. Use it only in TreeView, PipelineSteps and EmptyState's BranchPattern.
- Do not redraw or recolor the logo. Use the files in `src/assets/brand/`.

Where things live:
- Tokens: `src/styles/tokens.css` (imported after `@import "tailwindcss";` in `src/styles/globals.css`)
- Primitives (shadcn/ui): `src/components/ui/`
- App parts: `src/components/app/`
- Icon map: `src/components/app/icons.ts`
- Fonts (Switzer WOFF2, JetBrains Mono): `src/assets/fonts/`
- Logo: `src/assets/brand/shape-board-icon.svg`, `shape-board-lockup.svg`, `branch-pattern.svg`
