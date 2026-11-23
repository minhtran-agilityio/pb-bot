import * as T from "@radix-ui/react-tabs";
import type { ComponentPropsWithoutRef } from "react";

// DESIGN_SYSTEM.md §6.1 Tabs: pill segmented track. The screen owns the value (value + onValueChange).
export const Tabs = T.Root;

export function TabsList({ className = "", ...props }: ComponentPropsWithoutRef<typeof T.List>) {
  return (
    <T.List
      className={`inline-flex items-center gap-1 rounded-full bg-surface-3 p-1 ${className}`}
      {...props}
    />
  );
}

export function TabsTrigger({ className = "", ...props }: ComponentPropsWithoutRef<typeof T.Trigger>) {
  return (
    <T.Trigger
      className={`whitespace-nowrap rounded-full px-3.5 py-1.5 font-medium text-fg-muted transition-colors hover:text-fg disabled:pointer-events-none disabled:opacity-50 data-[state=active]:bg-surface-2 data-[state=active]:text-fg data-[state=active]:shadow-card ${className}`}
      {...props}
    />
  );
}

export function TabsContent({ className = "", ...props }: ComponentPropsWithoutRef<typeof T.Content>) {
  return <T.Content className={className} {...props} />;
}
