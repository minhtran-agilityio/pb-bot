import type { HTMLAttributes } from "react";
import { cn } from "../../lib/cn";

// DESIGN_SYSTEM.md §6.1 Card: one style. Clickable card = a <button> with hover border-strong + surface-3.
export function Card({ className, ...rest }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("rounded-lg border border-border bg-surface-2 p-4", className)} {...rest} />;
}

export const clickableCard =
  "rounded-lg border border-border bg-surface-2 p-4 text-left transition-colors duration-100 ease-out hover:border-border-strong hover:bg-surface-3";
