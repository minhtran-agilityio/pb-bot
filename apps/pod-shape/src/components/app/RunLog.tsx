import type { ReactNode } from "react";
import { Icon, iconSize } from "./icons";

// DESIGN_SYSTEM.md §6.2 RunLog / RunLogItem: status icon 14 + one-line action in font-mono text-sm
// + duration fg-subtle tabular-nums right. Running row = spinning loader in status-running;
// failed row = danger icon + one line error.
export type RunState = "running" | "done" | "failed";

export function RunLog({ children }: { children: ReactNode }) {
  return <ul className="flex flex-col divide-y divide-border rounded-lg border border-border">{children}</ul>;
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
    <li className="flex flex-col gap-1 px-3 py-2">
      <div className="flex items-center gap-2">
        {state === "running" && (
          <Icon.statusRunning {...iconSize.dense} aria-label="Running" className="shrink-0 animate-spin text-status-running" />
        )}
        {state === "done" && <Icon.check {...iconSize.dense} aria-label="Passed" className="shrink-0 text-success" />}
        {state === "failed" && <Icon.warning {...iconSize.dense} aria-label="Failed" className="shrink-0 text-danger" />}
        <span className="min-w-0 flex-1 truncate font-mono text-sm text-fg">{action}</span>
        <span className="text-sm text-fg-subtle tabular-nums">
          {state === "running" ? "…" : ms !== undefined ? `${(ms / 1000).toFixed(1)}s` : ""}
        </span>
      </div>
      {state === "failed" && error && <p className="pl-6 text-sm text-danger">{error}</p>}
    </li>
  );
}
