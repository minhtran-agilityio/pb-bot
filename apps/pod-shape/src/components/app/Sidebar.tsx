import type { ReactNode } from "react";
import { Icon, iconSize, type IconName } from "./icons";
import { Button } from "../ui/button";
import { Tooltip } from "../ui/tooltip";
import { cn } from "../../lib/cn";
import type { Theme } from "../../lib/theme";

// DESIGN_SYSTEM.md §6.2 Sidebar / SidebarItem: a 256px rounded olive panel (bg-side, rounded-panel).
// Items are pills; selected = surface-2 + shadow-card + accent icon. Bottom: "Add project", then a row
// with the theme button (Settings joins this row when its screen exists).
const themeIcon = { system: Icon.themeSystem, dark: Icon.themeDark, light: Icon.themeLight } as const;

export function Sidebar({
  children,
  onAddProject,
  theme,
  onCycleTheme,
}: {
  children: ReactNode;
  onAddProject: () => void;
  theme: Theme;
  onCycleTheme: () => void;
}) {
  const ThemeIcon = themeIcon[theme];
  return (
    <aside aria-label="Projects" className="flex w-64 shrink-0 flex-col rounded-panel bg-side pb-4">
      {/* The list scrolls with its own padding, so focus rings are not clipped (the selected item's soft shadow is cut at the edge, as in the prototype). */}
      <div className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto px-2.5 pt-4 pb-2">{children}</div>
      <div className="px-2.5">
        <Button variant="ghost" icon="add" className="w-full justify-start" onClick={onAddProject}>
          Add project
        </Button>
      </div>
      <div className="mx-2.5 mt-1.5 flex items-center justify-end border-t border-border pt-2">
        <Tooltip content={`Theme: ${theme}`}>
          <Button variant="side" size="icon-lg" aria-label={`Theme: ${theme}`} onClick={onCycleTheme}>
            <ThemeIcon {...iconSize.default} aria-hidden />
          </Button>
        </Tooltip>
      </div>
    </aside>
  );
}

export function SidebarLabel({ children }: { children: ReactNode }) {
  return <div className="px-3 pt-3.5 pb-1.5 text-xs font-semibold text-fg-muted">{children}</div>;
}

export function SidebarItem({
  icon,
  label,
  count,
  selected,
  onClick,
}: {
  icon: IconName;
  label: string;
  count?: number;
  selected: boolean;
  onClick: () => void;
}) {
  const I = Icon[icon];
  return (
    <button
      type="button"
      onClick={onClick}
      aria-current={selected ? "page" : undefined}
      className={cn(
        "flex h-9.5 w-full shrink-0 items-center gap-2.5 rounded-full px-3 text-left text-base text-fg transition duration-160 ease-out",
        selected ? "bg-surface-2 font-semibold shadow-card" : "font-medium hover:bg-side-hover",
      )}
    >
      <I {...iconSize.default} aria-hidden className={cn("shrink-0", selected && "text-accent")} />
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {count !== undefined && <span className="text-sm font-medium text-fg-muted tabular-nums">{count}</span>}
    </button>
  );
}
