import type { HTMLAttributes } from "react";
import { cn } from "../../lib/cn";

// DESIGN_SYSTEM.md §6.1 Card: one style. bg-surface-2, 1px border, rounded-card, p-5, shadow-card.
// Clickable card = a <button> that lifts 3px with shadow-card-hover.
export function Card({ className, ...rest }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("rounded-card border border-border bg-surface-2 p-5 shadow-card", className)} {...rest} />;
}

export const clickableCard =
  "rounded-card border border-border bg-surface-2 p-5 text-left shadow-card transition duration-200 ease-out hover:-translate-y-0.75 hover:shadow-card-hover motion-reduce:hover:translate-y-0";
