import { cn } from "../../lib/cn";
import { Badge } from "../ui/badge";
import { Icon, iconSize } from "./icons";

// DESIGN_SYSTEM.md §6.2 SessionList / SessionStatus: history of grill sessions (right panel).
// Dumb: the screen owns the sessions and the selection. Status = icon or dot + word, never colour alone.
export type SessionState = "failed" | "running" | "waiting_owner" | "brief_ready" | "stopped" | "confirmed";

// The pulsing "running" dot alone (SessionStatus and Composer share it).
export function RunningDot() {
  return <span aria-hidden="true" className="size-2 shrink-0 animate-pulse rounded-full bg-status-running motion-reduce:animate-none" />;
}

export function SessionStatus({ status }: { status: SessionState }) {
  switch (status) {
    case "running":
      return (
        <span className="inline-flex h-5.5 shrink-0 items-center gap-1.5 whitespace-nowrap text-xs font-medium text-fg-muted">
          <RunningDot />
          Claude is working
        </span>
      );
    case "failed":
      return (
        <Badge variant="outline">
          <Icon.warning {...iconSize.badge} aria-hidden="true" className="text-danger" />
          Failed
        </Badge>
      );
    case "waiting_owner":
      return <Badge variant="accent">Waiting for you</Badge>;
    case "brief_ready":
      return <Badge variant="accent">Brief ready</Badge>;
    case "stopped":
      return <Badge variant="outline">Stopped</Badge>;
    case "confirmed":
      return <Badge variant="neutral">Confirmed</Badge>;
    default:
      return <Badge variant="outline">{String(status)}</Badge>;
  }
}

export interface SessionItem {
  id: string;
  title: string;
  meta: string;
  status: SessionState;
}

export function SessionList({
  sessions,
  selectedId,
  onSelect,
}: {
  sessions: SessionItem[];
  selectedId?: string;
  onSelect: (id: string) => void;
}) {
  return (
    <ul className="flex flex-col gap-0.5">
      {sessions.map((s) => {
        const selected = s.id === selectedId;
        return (
          <li key={s.id}>
            <button
              type="button"
              aria-current={selected ? "true" : undefined}
              onClick={() => onSelect(s.id)}
              className={cn(
                "flex min-h-12 w-full flex-col justify-center rounded-box px-3 py-2 text-left transition duration-160 ease-out",
                selected ? "bg-surface-2 shadow-card" : "hover:bg-surface-3",
              )}
            >
              <span className="flex items-center gap-2">
                <span className="min-w-0 flex-1 truncate text-base text-fg">{s.title}</span>
                <SessionStatus status={s.status} />
              </span>
              <span className="truncate text-xs text-fg-muted">{s.meta}</span>
            </button>
          </li>
        );
      })}
    </ul>
  );
}
