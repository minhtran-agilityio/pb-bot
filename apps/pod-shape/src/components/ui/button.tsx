import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";
import { Icon, iconSize, type IconName } from "../app/icons";
import { cn } from "../../lib/cn";

// DESIGN_SYSTEM.md §6.1 Button: pills. primary · secondary · ghost · danger; sm 30 · md 36; icon (square).
type Variant = "primary" | "secondary" | "ghost" | "danger";
type Size = "sm" | "md" | "icon";

const variants: Record<Variant, string> = {
  primary: "bg-accent text-accent-fg shadow-btn hover:bg-accent-hover hover:shadow-btn-hover",
  secondary: "border border-border bg-surface-2 text-fg shadow-card hover:bg-surface-3",
  ghost: "bg-transparent text-fg-muted hover:bg-surface-3 hover:text-fg",
  danger: "bg-danger-soft text-danger hover:bg-surface-3",
};
const sizes: Record<Size, string> = {
  sm: "h-7.5 px-3 text-sm",
  md: "h-9 px-4 text-base",
  icon: "h-9 w-9 justify-center",
};

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: Size;
  icon?: IconName;
  loading?: boolean;
  children?: ReactNode;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = "secondary", size = "md", icon, loading = false, disabled, className, children, type = "button", ...rest },
  ref,
) {
  const LeftIcon = loading ? Icon.loading : icon ? Icon[icon] : null;
  return (
    <button
      ref={ref}
      type={type}
      disabled={disabled || loading}
      className={cn(
        "inline-flex shrink-0 items-center gap-1.5 rounded-full font-medium whitespace-nowrap",
        "transition duration-160 ease-out active:scale-97",
        "disabled:pointer-events-none disabled:opacity-50",
        variants[variant],
        sizes[size],
        className,
      )}
      {...rest}
    >
      {LeftIcon && (
        <LeftIcon {...iconSize.default} aria-hidden className={cn(loading && "animate-spin")} />
      )}
      {children}
    </button>
  );
});
