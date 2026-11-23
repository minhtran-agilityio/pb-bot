import { Toaster as Sonner, toast } from "sonner";
import { Icon, iconSize } from "../app/icons";

// DESIGN_SYSTEM.md §6.1 Toast: sonner, bottom-right, 4s; error stays until closed.
export function Toaster() {
  return (
    <Sonner
      position="bottom-right"
      duration={4000}
      closeButton
      icons={{
        success: <Icon.success {...iconSize.default} aria-hidden className="text-success" />,
        error: <Icon.warning {...iconSize.default} aria-hidden className="text-danger" />,
      }}
      toastOptions={{
        unstyled: true,
        classNames: {
          toast:
            "flex w-full items-start gap-2 rounded-card border border-border bg-surface-2 px-4 py-3.5 text-base text-fg shadow-sm",
          title: "font-medium text-fg",
          description: "text-sm text-fg-muted",
          closeButton: "order-last ml-auto rounded-full p-1 text-fg-muted hover:bg-surface-3",
        },
      }}
    />
  );
}

export const notify = {
  success: (title: string, description?: string) => toast.success(title, { description }),
  error: (title: string, description?: string) => toast.error(title, { description, duration: Infinity }),
};
