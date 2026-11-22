import type { ReactNode } from "react";
import { Icon, iconSize } from "./icons";

// DESIGN_SYSTEM.md §6.2 RunLog / RunLogItem: a rounded-card list on surface-2 with shadow-card.
// Row: status icon 14 + one-line action in font-mono text-xs (real code: the command that ran)
// + duration fg-subtle tabular-nums right. Running row = spinning loader in status-running;
// failed row = danger icon + one line error.
export type RunState = "running" | "done" | "failed";

export function RunLog({ children }: { children: ReactNode }) {
  return <ul className="flex flex-col divide-y divide-border overflow-hidden rounded-card border border-border bg-surface-2 shadow-card">{children}</ul>;
}

export function RunLogItem({
  state,
  action,
  ms,
  error,
}: {
  state: RunState;
  action: string;
  ms?: number;
  error?: string;
}) {
  return (
    <li className="flex min-h-8.5 flex-col justify-center gap-1 px-3.5 py-1">
      <div className="flex items-center gap-2">
        {state === "running" && (
          <Icon.statusRunning {...iconSize.dense} aria-label="Running" className="shrink-0 animate-spin text-status-running" />
        )}
        {state === "done" && <Icon.check {...iconSize.dense} aria-label="Passed" className="shrink-0 text-success" />}
        {state === "failed" && <Icon.warning {...iconSize.dense} aria-label="Failed" className="shrink-0 text-danger" />}
        <span className="min-w-0 flex-1 truncate font-mono text-xs text-fg">{action}</span>
        <span className="font-mono text-xs text-fg-subtle tabular-nums">
          {state === "running" ? "…" : ms !== undefined ? `${(ms / 1000).toFixed(1)}s` : ""}
        </span>
      </div>
      {state === "failed" && error && <p className="pl-5.5 text-sm text-danger">{error}</p>}
    </li>
  );
}
