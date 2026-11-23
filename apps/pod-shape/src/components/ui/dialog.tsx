import * as D from "@radix-ui/react-dialog";
import type { ReactNode } from "react";
import { Icon, iconSize } from "../app/icons";
import { Button } from "./button";
import { cn } from "../../lib/cn";

// DESIGN_SYSTEM.md §6.1 Dialog: sm 420px · md 560px · lg 800px; bg-surface-2, rounded-dialog, shadow-lg, p-7.
const widths = { sm: "w-105", md: "w-140", lg: "w-200" } as const;

export function Dialog({
  open,
  onOpenChange,
  title,
  size = "md",
  children,
  footer,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  size?: keyof typeof widths;
  children: ReactNode;
  footer: ReactNode;
}) {
  return (
    <D.Root open={open} onOpenChange={onOpenChange}>
      <D.Portal>
        <D.Overlay className="fixed inset-0 z-40 bg-overlay" />
        {/* Inset wrapper: the content never exceeds the viewport. It lets clicks through to the overlay. */}
        <div className="pointer-events-none fixed inset-6 z-50 flex items-center justify-center">
          <D.Content
            aria-describedby={undefined}
            className={cn(
              "pointer-events-auto relative flex max-h-full max-w-full flex-col gap-4.5",
              "rounded-dialog bg-surface-2 p-7 shadow-lg focus-visible:outline-none",
              widths[size],
            )}
          >
            <div className="flex min-h-7 flex-none items-center justify-between gap-2 pr-10">
              <D.Title className="text-lg font-semibold text-fg">{title}</D.Title>
            </div>
            {/* Only the body scrolls; the padding keeps focus rings at its edges from being clipped. */}
            <div className="-m-1.5 min-h-0 flex-1 overflow-y-auto p-1.5">{children}</div>
            <div className="flex flex-none justify-end gap-2">{footer}</div>
            {/* Last in the DOM so it is the last Tab stop; positioned where the header would hold it. */}
            <D.Close asChild>
              <Button variant="ghost" size="icon-sm" aria-label="Close" className="absolute top-6.5 right-6.5">
                <Icon.close {...iconSize.default} aria-hidden />
              </Button>
            </D.Close>
          </D.Content>
        </div>
      </D.Portal>
    </D.Root>
  );
}
