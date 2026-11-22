import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";
import { Icon, iconSize, type IconName } from "../app/icons";
import { cn } from "../../lib/cn";

// DESIGN_SYSTEM.md §6.1 Button: primary · secondary · ghost · danger; sm · md; icon.
type Variant = "primary" | "secondary" | "ghost" | "danger";
type Size = "sm" | "md" | "icon";

const variants: Record<Variant, string> = {
  primary: "bg-accent text-accent-fg hover:bg-accent-hover",
  secondary: "bg-surface-2 text-fg border border-border-strong hover:bg-surface-3",
  ghost: "bg-transparent text-fg hover:bg-surface-3",
  danger: "bg-danger-soft text-danger hover:bg-surface-3",
};
const sizes: Record<Size, string> = {
  sm: "h-7 px-2 text-sm",
  md: "h-8 px-3 text-base",
  icon: "h-8 w-8 justify-center",
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
        "inline-flex shrink-0 items-center gap-2 rounded-md font-medium whitespace-nowrap",
        "transition-colors duration-100 ease-out active:translate-y-px",
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
