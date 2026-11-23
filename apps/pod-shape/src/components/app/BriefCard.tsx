import type { ReactNode } from "react";
import { cn } from "../../lib/cn";

// DESIGN_SYSTEM.md §6.2 BriefCard: the brief shown as SOURCE inside a claude ChatMessage (no card inside a card).
// A plain code block (font-mono text-xs, bg-surface-3, rounded-md, pre-wrap) that scrolls after about 16 lines,
// a 1px separator, then a footer row: meta (text-sm fg-muted) left, actions right.
// `replaced`: one muted line (the `meta`) instead of the block. Props in, events out: the screen owns the data.
export function BriefCard({
  source,
  meta,
  actions,
  replaced = false,
  className,
}: {
  source: string;
  meta: ReactNode;
  actions?: ReactNode;
  replaced?: boolean;
  className?: string;
}) {
  if (replaced) return <p className={cn("text-sm text-fg-muted", className)}>{meta}</p>;
  return (
    <div className={cn("flex flex-col gap-3", className)}>
      <pre
        tabIndex={0}
        aria-label="Brief source"
        className="max-h-70 overflow-auto whitespace-pre-wrap break-words rounded-md bg-surface-3 p-3 font-mono text-xs text-fg focus-visible:outline-none focus-visible:shadow-focus"
      >
        {source}
      </pre>
      <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-2 border-t border-border pt-3">
        <span className="min-w-0 text-sm text-fg-muted">{meta}</span>
        {actions && <div className="flex shrink-0 items-center gap-1">{actions}</div>}
      </div>
    </div>
  );
}
