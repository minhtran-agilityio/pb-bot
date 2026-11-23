import type { HTMLAttributes } from "react";
import { cn } from "../../lib/cn";

// DESIGN_SYSTEM.md §6.1 Kbd: a key name the owner can press ("1", "Enter", "Shift").
// `accent` is for a key inside an accent chip (accent-text on an accent-soft border).
export type KbdTone = "default" | "accent";

const tones: Record<KbdTone, string> = {
  default: "border-border text-fg-muted",
  accent: "border-accent-soft text-accent-text",
};

export interface KbdProps extends HTMLAttributes<HTMLElement> {
  tone?: KbdTone;
}

export function Kbd({ tone = "default", className, children, ...rest }: KbdProps) {
  return (
    <kbd
      className={cn(
        "whitespace-nowrap rounded-xs border px-1 font-sans text-xs tabular-nums",
        tones[tone],
        className,
      )}
      {...rest}
    >
      {children}
    </kbd>
  );
}
