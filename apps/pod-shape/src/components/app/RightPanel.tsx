import type { ReactNode } from "react";
import { Button } from "../ui/button";
import { Tooltip } from "../ui/tooltip";
import { Icon, iconSize } from "./icons";
import { cn } from "../../lib/cn";

// DESIGN_SYSTEM.md §6.2 RightPanel: 372px, bg-surface-2, rounded-panel, shadow-card. Header h-13 with the
// title and a close icon button, border-b; body p-5 gap-4.5 that scrolls (the header stays).
// Below 1180px (`max-panel:`, §7) it is an overlay sheet: absolute, top 0, right and bottom 12px, shadow-lg.
// The parent must be `relative` and sets the height; the screen or App decides when the panel shows.
export function RightPanel({ title, onClose, children }: { title: string; onClose: () => void; children: ReactNode }) {
  return (
    <aside
      aria-label={title}
      className={cn(
        "flex min-h-0 w-93 shrink-0 flex-col overflow-hidden rounded-panel bg-surface-2 shadow-card",
        "max-panel:absolute max-panel:bottom-3 max-panel:right-3 max-panel:top-0 max-panel:z-20 max-panel:shadow-lg",
      )}
    >
      <div className="flex h-13 shrink-0 items-center gap-2 border-b border-border pl-5 pr-2.5">
        <h2 className="min-w-0 flex-1 truncate text-base font-medium text-fg">{title}</h2>
        <Tooltip content="Close panel">
          <Button variant="ghost" size="icon-sm" aria-label="Close panel" onClick={onClose}>
            <Icon.close {...iconSize.default} aria-hidden />
          </Button>
        </Tooltip>
      </div>
      <div className="flex min-h-0 flex-1 flex-col gap-4.5 overflow-auto p-5 *:shrink-0">{children}</div>
    </aside>
  );
}

// DESIGN_SYSTEM.md §6.2 ReadyBox: the "Ready to generate" box inside a RightPanel body: bg-surface-2,
// border, rounded-box, p-4; a text-xs label and a bulleted list of lines (children are the <li> lines).
export function ReadyBox({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-2 rounded-box border border-border bg-surface-2 p-4">
      <span className="text-xs font-medium text-fg-subtle">{label}</span>
      <ul className="flex list-disc flex-col gap-0.5 pl-4.5 text-base text-fg-muted">{children}</ul>
    </div>
  );
}
