import { forwardRef, type InputHTMLAttributes } from "react";
import { cn } from "../../lib/cn";

// DESIGN_SYSTEM.md §6.1 Input (md, h-8). `invalid` = the §6 error state.
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
        "h-8 w-full min-w-0 rounded-md border bg-surface-2 px-3 text-base text-fg",
        "placeholder:text-fg-subtle disabled:pointer-events-none disabled:opacity-50",
        invalid ? "border-danger" : "border-border-strong",
        className,
      )}
      {...rest}
    />
  );
});
