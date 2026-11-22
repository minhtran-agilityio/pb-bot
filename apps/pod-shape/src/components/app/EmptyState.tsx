import type { ReactNode } from "react";
import { Icon, iconSize, type IconName } from "./icons";
import pattern from "../../assets/brand/branch-pattern.svg?raw";

// DESIGN_SYSTEM.md §6.2 EmptyState + §6.4 BranchPattern: left-aligned block, pattern on the right
// in the `branch` color (the SVG carries its own 40% opacity and 160px size via currentColor), icon 24 fg-subtle, text-lg title, one line, one Button.
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
    <div className="flex items-start gap-12 py-2">
      <div className="flex flex-col items-start gap-2">
        <I {...iconSize.empty} aria-hidden className="text-fg-subtle" />
        <h2 className="text-lg font-semibold text-fg">{title}</h2>
        <p className="text-base text-fg-muted">{text}</p>
        <div className="pt-1">{action}</div>
      </div>
      <span aria-hidden className="pointer-events-none text-branch" dangerouslySetInnerHTML={{ __html: pattern }} />
    </div>
  );
}
