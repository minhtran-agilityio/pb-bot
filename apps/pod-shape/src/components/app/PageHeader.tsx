import type { ReactNode } from "react";

// DESIGN_SYSTEM.md §6.2 PageHeader: text-xl (26/34) title, text-sm fg-muted meta, optional steps line, actions right.
export function PageHeader({
  title,
  meta,
  steps,
  actions,
}: {
  title: string;
  meta?: ReactNode;
  steps?: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <div className="flex items-start justify-between gap-4">
      <div className="flex min-w-0 flex-col gap-0.5">
        <h1 className="truncate text-xl font-semibold tracking-title text-fg">{title}</h1>
        {meta && <div className="flex flex-wrap items-center gap-2 text-sm text-fg-muted">{meta}</div>}
        {steps && <div className="mt-1.5">{steps}</div>}
      </div>
      {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
    </div>
  );
}
