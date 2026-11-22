import type { ReactNode } from "react";
import { Icon, iconSize, type IconName } from "./icons";
import pattern from "../../assets/brand/branch-pattern.svg?raw";

// DESIGN_SYSTEM.md §6.2 EmptyState + §6.4 BranchPattern: left-aligned block, pattern on the right
// in the `branch` color (the SVG carries its own 40% opacity; shown 120px wide as in the prototype), icon 24 fg-subtle, text-lg title, one line, one Button.
export function EmptyState({
  icon,
  title,
  text,
  action,
}: {
  icon: IconName;
  title: string;
  text: string;
  action: ReactNode;
}) {
  const I = Icon[icon];
  return (
    <div className="flex max-w-140 items-start justify-between gap-4 p-2">
      <div className="flex max-w-85 flex-col items-start gap-1.5">
        <I {...iconSize.empty} aria-hidden className="text-fg-subtle" />
        <h2 className="text-lg font-semibold text-fg">{title}</h2>
        <p className="text-base text-fg-muted">{text}</p>
        {action}
      </div>
      <span aria-hidden className="pointer-events-none w-30 shrink-0 text-branch *:h-auto *:w-full" dangerouslySetInnerHTML={{ __html: pattern }} />
    </div>
  );
}
