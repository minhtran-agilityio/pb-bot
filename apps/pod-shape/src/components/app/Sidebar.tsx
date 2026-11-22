import type { ReactNode } from "react";
import { Icon, iconSize, type IconName } from "./icons";
import { Button } from "../ui/button";
import { cn } from "../../lib/cn";

// DESIGN_SYSTEM.md §6.2 Sidebar / SidebarItem: 240px project list; selected = §6 selected state.
export function Sidebar({ children, onAddProject }: { children: ReactNode; onAddProject: () => void }) {
  return (
    <aside aria-label="Projects" className="flex w-60 shrink-0 flex-col border-r border-border bg-surface-1">
      <div className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto p-2">{children}</div>
      <div className="p-2">
        <Button variant="ghost" icon="add" className="w-full" onClick={onAddProject}>
          Add project
        </Button>
      </div>
    </aside>
  );
}

export function SidebarLabel({ children }: { children: ReactNode }) {
  return <div className="px-2 pt-4 pb-1 text-xs font-medium tracking-wider text-fg-subtle uppercase">{children}</div>;
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
        "flex h-8 w-full items-center gap-2 rounded-md px-2 text-left text-base transition-colors duration-100 ease-out",
        selected ? "bg-surface-3 text-fg" : "text-fg-muted hover:bg-surface-3 hover:text-fg",
      )}
    >
      <I {...iconSize.default} aria-hidden className={cn("shrink-0", selected && "text-accent")} />
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {count !== undefined && <span className="text-sm text-fg-subtle tabular-nums">{count}</span>}
    </button>
  );
}
