import type { ReactNode } from "react";

// DESIGN_SYSTEM.md §6.2 PageHeader: text-xl title, text-sm fg-muted meta, actions right.
export function PageHeader({ title, meta, actions }: { title: string; meta?: ReactNode; actions?: ReactNode }) {
  return (
    <div className="flex items-start justify-between gap-4">
      <div className="flex min-w-0 flex-col gap-0.5">
        <h1 className="truncate text-xl font-semibold text-fg">{title}</h1>
        {meta && <div className="flex items-center gap-1.5 text-sm text-fg-muted">{meta}</div>}
      </div>
      {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
    </div>
  );
}
