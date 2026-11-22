import * as T from "@radix-ui/react-tooltip";
import type { ReactNode } from "react";

// DESIGN_SYSTEM.md §6.1 Tooltip: bg-surface-3, text-sm, shadow-sm, rounded-md, 400ms delay.
export const TooltipProvider = ({ children }: { children: ReactNode }) => (
  <T.Provider delayDuration={400}>{children}</T.Provider>
);

export function Tooltip({ content, children }: { content: ReactNode; children: ReactNode }) {
  return (
    <T.Root>
      <T.Trigger asChild>{children}</T.Trigger>
      <T.Portal>
        <T.Content
          sideOffset={6}
          className="z-50 rounded-md bg-surface-3 px-2 py-1 text-sm text-fg shadow-sm"
        >
          {content}
        </T.Content>
      </T.Portal>
    </T.Root>
  );
}
