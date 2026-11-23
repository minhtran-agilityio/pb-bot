import type { ReactNode } from "react";
import { Icon, iconSize } from "../app/icons";
import { Button } from "./button";
import { Tooltip } from "./tooltip";

// DESIGN_SYSTEM.md §6.1 Banner: a one-line notice above the page content. `warning` only.
// Status = icon + words. role="status" (polite), not an alert. The screen owns the data and the dismissed state.
export interface BannerProps {
  variant?: "warning";
  children: ReactNode;
  /** Optional ghost sm Button, for example <Button variant="ghost" size="sm">Add artifact</Button>. */
  action?: ReactNode;
  /** Shows the dismiss button when set. */
  onDismiss?: () => void;
}

export function Banner({ variant = "warning", children, action, onDismiss }: BannerProps) {
  return (
    <div
      role="status"
      data-variant={variant}
      className="flex items-center gap-2 rounded-box bg-warning-soft px-4 py-2.5 text-sm text-fg"
    >
      <Icon.warning {...iconSize.default} aria-hidden className="shrink-0 text-warning" />
      <p className="min-w-0 flex-1">{children}</p>
      {action}
      {onDismiss && (
        <Tooltip content="Dismiss">
          <Button variant="ghost" size="icon-sm" aria-label="Dismiss" onClick={onDismiss}>
            <Icon.close {...iconSize.default} aria-hidden />
          </Button>
        </Tooltip>
      )}
    </div>
  );
}
