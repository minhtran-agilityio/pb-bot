import type { ReactNode } from "react";
import { cn } from "../../lib/cn";
import { Icon, iconSize } from "./icons";

// DESIGN_SYSTEM.md §6.2 ChatMessage + ChatNote (grill chat). Dumb parts: the screen owns the thread.
// Long words wrap (wrap-anywhere); code spans are font-mono on surface-3 (real code only).
const code = "[&_code]:rounded-xs [&_code]:bg-surface-3 [&_code]:px-1 [&_code]:font-mono [&_code]:text-sm";

export interface ChatMessageProps {
  role: "claude" | "user";
  /** claude: text, and later a question card or the brief card. user: text. */
  children: ReactNode;
}

export function ChatMessage({ role, children }: ChatMessageProps) {
  if (role === "user") {
    return (
      <div
        data-role="user"
        className={cn(
          "max-w-4/5 self-end whitespace-pre-wrap rounded-card rounded-br-xs bg-surface-2 px-4 py-2.5 text-md wrap-anywhere shadow-card",
          code,
        )}
      >
        {children}
      </div>
    );
  }
  return (
    <div data-role="claude" className="flex gap-3">
      <span aria-hidden className="grid size-7.5 shrink-0 place-items-center rounded-full bg-accent-soft">
        <span className="size-2 rounded-full bg-accent" />
      </span>
      <div className={cn("flex min-w-0 flex-1 flex-col gap-2 text-md wrap-anywhere", code)}>{children}</div>
    </div>
  );
}

export interface ChatNoteProps {
  tone: "error" | "info" | "success";
  children: ReactNode;
}

/** error = role="alert", danger text; info and success = plain text with an icon (status = icon + words). */
export function ChatNote({ tone, children }: ChatNoteProps) {
  const I = tone === "success" ? Icon.success : Icon.warning;
  return (
    <div
      role={tone === "error" ? "alert" : undefined}
      data-tone={tone}
      className={cn(
        "flex items-start gap-2 wrap-anywhere",
        tone === "error" ? "text-sm text-danger" : "text-xs text-fg-muted",
        code,
      )}
    >
      <I
        {...(tone === "error" ? iconSize.default : iconSize.dense)}
        aria-hidden
        className={cn("shrink-0", tone === "success" && "text-success")}
      />
      <span className="min-w-0 flex-1">{children}</span>
    </div>
  );
}
