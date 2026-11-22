import * as D from "@radix-ui/react-dialog";
import type { ReactNode } from "react";
import { Icon, iconSize } from "../app/icons";
import { Button } from "./button";
import { cn } from "../../lib/cn";

// DESIGN_SYSTEM.md §6.1 Dialog: sm 400px · md 560px · lg 800px; bg-surface-2, rounded-lg, shadow-lg, p-6.
const widths = { sm: "w-100", md: "w-140", lg: "w-200" } as const;

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
        <D.Content
          aria-describedby={undefined}
          className={cn(
            "fixed top-1/2 left-1/2 z-50 flex max-w-full -translate-x-1/2 -translate-y-1/2 flex-col gap-4",
            "rounded-lg border border-border bg-surface-2 p-6 shadow-lg focus-visible:outline-none",
            widths[size],
          )}
        >
          <div className="flex items-center justify-between gap-2">
            <D.Title className="text-lg font-semibold text-fg">{title}</D.Title>
            <D.Close asChild>
              <Button variant="ghost" size="icon" aria-label="Close">
                <Icon.close {...iconSize.default} aria-hidden />
              </Button>
            </D.Close>
          </div>
          {children}
          <div className="flex justify-end gap-2">{footer}</div>
        </D.Content>
      </D.Portal>
    </D.Root>
  );
}
