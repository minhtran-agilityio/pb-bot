import { forwardRef, type InputHTMLAttributes } from "react";
import { cn } from "../../lib/cn";

// DESIGN_SYSTEM.md §6.1 Input: pill, h-9.5 (38px). Focus = accent border + 4px soft halo. `invalid` = the §6 error state.
export interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  invalid?: boolean;
}

export const Input = forwardRef<HTMLInputElement, InputProps>(function Input(
  { invalid = false, className, ...rest },
  ref,
) {
  return (
    <input
      ref={ref}
      aria-invalid={invalid || undefined}
      className={cn(
        "h-9.5 w-full min-w-0 rounded-full border bg-surface-2 px-4 text-sm text-fg",
        "placeholder:text-fg-subtle disabled:pointer-events-none disabled:opacity-50",
        "focus-visible:outline-none",
        // An invalid field keeps its danger border on focus and gets no accent halo; only a valid one turns accent.
        invalid ? "border-danger" : "border-border focus-visible:border-accent focus-visible:shadow-focus",
        className,
      )}
      {...rest}
    />
  );
});
