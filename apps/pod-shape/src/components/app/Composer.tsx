import { forwardRef, type ReactNode } from "react";
import { cn } from "../../lib/cn";
import { Button } from "../ui/button";
import { Kbd } from "../ui/kbd";
import { Textarea } from "../ui/textarea";
import { RunningDot } from "./SessionList";

// DESIGN_SYSTEM.md §6.2 Composer: the message box at the bottom of the grill chat.
// Dumb: the screen owns the value and what a send does. Enter sends, Shift+Enter adds a line; Enter during IME composition does nothing.
export interface ComposerProps {
  value: string;
  onChange: (value: string) => void;
  onSend: () => void;
  placeholder?: string;
  disabled?: boolean;
  /** A turn is running: "Claude is thinking…" replaces the left side of the hint row. */
  thinking?: boolean;
  /** The right side of the hint row. */
  hint?: ReactNode;
  className?: string;
}

export const Composer = forwardRef<HTMLTextAreaElement, ComposerProps>(function Composer(
  { value, onChange, onSend, placeholder, disabled = false, thinking = false, hint, className },
  ref,
) {
  const blank = value.trim() === "";
  return (
    <div className={cn("flex flex-col gap-2 bg-transparent px-10 pt-3 pb-5", className)}>
      <div className="flex items-center gap-2 rounded-card border border-border bg-surface-2 py-2 pr-2 pl-4.5 shadow-card focus-within:shadow-focus">
        <Textarea
          ref={ref}
          variant="bare"
          aria-label="Message"
          value={value}
          placeholder={placeholder}
          disabled={disabled}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key !== "Enter" || e.shiftKey || e.nativeEvent.isComposing) return;
            e.preventDefault();
            if (!blank && !disabled) onSend();
          }}
        />
        <Button variant="primary" size="icon" icon="send" aria-label="Send" disabled={disabled || blank} onClick={onSend} />
      </div>
      <div className="flex items-center justify-between gap-4 text-xs text-fg-subtle">
        {thinking ? (
          <span role="status" className="inline-flex items-center gap-1.5">
            <RunningDot />
            Claude is thinking…
          </span>
        ) : (
          <span>
            <Kbd>Enter</Kbd> send · <Kbd>Shift</Kbd>+<Kbd>Enter</Kbd> new line
          </span>
        )}
        {hint != null && <span className="min-w-0 truncate">{hint}</span>}
      </div>
    </div>
  );
});
