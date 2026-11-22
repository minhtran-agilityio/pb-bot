import { Icon, iconSize } from "./icons";
import { Button } from "../ui/button";
import { Tooltip } from "../ui/tooltip";
import logo from "../../assets/brand/shape-board-icon.svg";
import type { Theme } from "../../lib/theme";

// DESIGN_SYSTEM.md §6.2 TitleBar: 40px, bg-surface-1, border-b, logo 16px + breadcrumb, theme toggle right.
// The macOS traffic lights sit over the left edge (titleBarStyle "Overlay"); pl-6 + the w-12 spacer keep clear of them.
const themeIcon = { system: Icon.themeSystem, dark: Icon.themeDark, light: Icon.themeLight } as const;

export function TitleBar({
  crumbs,
  theme,
  onCycleTheme,
}: {
  crumbs: string[];
  theme: Theme;
  onCycleTheme: () => void;
}) {
  const ThemeIcon = themeIcon[theme];
  return (
    <header
      data-tauri-drag-region
      className="flex h-10 shrink-0 items-center gap-2 border-b border-border bg-surface-1 pr-3 pl-6 select-none"
    >
      <div data-tauri-drag-region className="w-12 shrink-0" />
      <img src={logo} alt="" width={16} height={16} className="pointer-events-none" />
      <nav aria-label="Breadcrumb" data-tauri-drag-region className="flex min-w-0 items-center gap-1.5 text-base">
        {crumbs.map((c, i) => (
          <span key={i} className="flex items-center gap-1.5" data-tauri-drag-region>
            {i > 0 && <Icon.expand {...iconSize.dense} aria-hidden className="text-fg-subtle" />}
            <span className={i === crumbs.length - 1 ? "truncate text-fg" : "truncate text-fg-muted"}>{c}</span>
          </span>
        ))}
      </nav>
      <div data-tauri-drag-region className="flex-1 self-stretch" />
      <Tooltip content={`Theme: ${theme}`}>
        <Button variant="ghost" size="icon" aria-label={`Theme: ${theme}`} onClick={onCycleTheme}>
          <ThemeIcon {...iconSize.default} aria-hidden />
        </Button>
      </Tooltip>
    </header>
  );
}
