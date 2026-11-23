import type { HTMLAttributes } from "react";
import { cn } from "../../lib/cn";
import { Icon, iconSize, type IconName } from "../app/icons";

// DESIGN_SYSTEM.md §6.1 Badge: static pill label. neutral · accent · outline.
// Optional 12px icon from icons.ts (iconSize.badge). No interaction; the screen owns the text.
export type BadgeVariant = "neutral" | "accent" | "outline";

const variants: Record<BadgeVariant, string> = {
  neutral: "bg-surface-3 text-fg-muted",
  accent: "bg-accent-soft text-accent-text",
  outline: "border border-border text-fg-muted",
};

export interface BadgeProps extends HTMLAttributes<HTMLSpanElement> {
  variant?: BadgeVariant;
  icon?: IconName;
}

export function Badge({ variant = "neutral", icon, className, children, ...rest }: BadgeProps) {
  const I = icon ? Icon[icon] : null;
  return (
    <span
      className={cn(
        "inline-flex h-5.5 items-center gap-1 whitespace-nowrap rounded-full px-2.5 text-xs font-medium",
        variants[variant],
        className,
      )}
      {...rest}
    >
      {I ? <I {...iconSize.badge} aria-hidden /> : null}
      {children}
    </span>
  );
}
