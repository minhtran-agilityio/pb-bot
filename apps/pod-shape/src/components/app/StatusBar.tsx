import { Tooltip } from "../ui/tooltip";

// DESIGN_SYSTEM.md §6.2 StatusBar: h-1.5 rounded-full, segments in status colors,
// order merged · pr · open · closed; tooltip shows counts.
// "running" is not tracked in V1 (owner decision 5), so it has no segment.
export interface StatusCounts {
  merged: number;
  pr: number;
  open: number;
  closed: number;
}

const SEGMENTS = [
  { key: "merged", label: "merged", className: "bg-status-merged" },
  { key: "pr", label: "has PR", className: "bg-status-pr" },
  { key: "open", label: "open", className: "bg-status-open" },
  { key: "closed", label: "closed", className: "bg-status-closed" },
] as const;

export function StatusBar({ counts }: { counts: StatusCounts }) {
  const total = SEGMENTS.reduce((n, s) => n + counts[s.key], 0);
  const summary =
    total === 0
      ? "No issues"
      : SEGMENTS.filter((s) => counts[s.key] > 0)
          .map((s) => `${counts[s.key]} ${s.label}`)
          .join(" · ");
  return (
    <Tooltip content={summary}>
      <div role="img" aria-label={summary} className="flex h-1.5 w-full gap-px overflow-hidden rounded-full bg-surface-3">
        {SEGMENTS.map((s) =>
          counts[s.key] > 0 ? (
            <div key={s.key} className={s.className} style={{ flexGrow: counts[s.key] }} />
          ) : null,
        )}
      </div>
    </Tooltip>
  );
}
