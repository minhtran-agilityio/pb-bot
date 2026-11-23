import { forwardRef, useRef, type ReactNode } from "react";
import { cn } from "../../lib/cn";
import { Button } from "../ui/button";
import { Kbd } from "../ui/kbd";
import { Textarea } from "../ui/textarea";
import { RunningDot } from "./SessionList";

// DESIGN_SYSTEM.md §6.2 Composer: the message box at the bottom of the grill chat.
// Dumb: the screen owns the value and what a send does. Enter sends, Shift+Enter adds a line.
// Enter that commits an IME word never sends: isComposing, keyCode 229 (WebKit) and a composition ref are all checked.
export interface ComposerProps {
  value: string;
  onChange: (value: string) => void;
  onSend: () => void;
  placeholder?: string;
  disabled?: boolean;
  /** A turn is running: sending is blocked and "Claude is thinking…" replaces the left side of the hint row. */
  thinking?: boolean;
  /** Danger border on the box, no halo. */
  invalid?: boolean;
  /** The right side of the hint row. */
  hint?: ReactNode;
  /** Accessible name of the textarea (default "Message"). */
  ariaLabel?: string;
  className?: string;
}

export const Composer = forwardRef<HTMLTextAreaElement, ComposerProps>(function Composer(
  { value, onChange, onSend, placeholder, disabled = false, thinking = false, invalid = false, hint, ariaLabel = "Message", className },
  ref,
) {
  const composing = useRef(false);
  const canSend = value.trim() !== "" && !disabled && !thinking;
  return (
    <div className={cn("flex flex-col gap-2 bg-transparent px-10 pt-3 pb-5", className)}>
      <div
        className={cn(
          "flex items-end gap-2 rounded-card border bg-surface-2 py-2 pr-2 pl-4.5 shadow-card",
          invalid ? "border-danger" : "border-border focus-within:shadow-focus-card",
        )}
      >
        <Textarea
          ref={ref}
          variant="bare"
          aria-label={ariaLabel}
          value={value}
          placeholder={placeholder}
          disabled={disabled}
          onChange={(e) => onChange(e.target.value)}
          onCompositionStart={() => {
            composing.current = true;
          }}
          onCompositionEnd={() => {
            // WebKit fires compositionend before the keydown of the committing Enter; clear after that keydown.
            setTimeout(() => {
              composing.current = false;
            }, 0);
          }}
          onKeyDown={(e) => {
            if (e.key !== "Enter" || e.shiftKey) return;
            if (e.nativeEvent.isComposing || e.nativeEvent.keyCode === 229 || composing.current) return;
            e.preventDefault();
            if (canSend) onSend();
          }}
        />
        <Button variant="primary" size="icon" icon="send" aria-label="Send" disabled={!canSend} onClick={onSend} />
      </div>
      <div className="flex items-center justify-between gap-4 text-xs text-fg-subtle">
        <span>
          {!thinking && (
            <span>
              <Kbd>Enter</Kbd> send · <Kbd>Shift</Kbd>+<Kbd>Enter</Kbd> new line
            </span>
          )}
          <span role="status" className="inline-flex items-center gap-2 text-fg-muted">
            {thinking && (
              <>
                <RunningDot />
                Claude is thinking…
              </>
            )}
          </span>
        </span>
        {hint ? <span className="min-w-0 truncate">{hint}</span> : null}
      </div>
    </div>
  );
});
