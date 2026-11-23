import * as S from "@radix-ui/react-select";
import { forwardRef, type ReactNode } from "react";
import { cn } from "../../lib/cn";
import { Icon, iconSize } from "../app/icons";

// DESIGN_SYSTEM.md §6.1 Select: Radix Select, pill like Input (h-9.5, pr-9, chevron 16 at the right).
// The menu is the §6.1 DropdownMenu style. Dumb: the screen owns `value`, `onValueChange` and the options.
// Option values must not be "" (Radix reserves it for "no value"); with no value the placeholder shows.
export interface SelectOption {
  value: string;
  label: ReactNode;
  disabled?: boolean;
}

export interface SelectProps {
  value?: string;
  onValueChange?: (value: string) => void;
  options: SelectOption[];
  placeholder?: string;
  disabled?: boolean;
  invalid?: boolean;
  id?: string;
  name?: string;
  className?: string;
  "aria-label"?: string;
  "aria-labelledby"?: string;
}

export const Select = forwardRef<HTMLButtonElement, SelectProps>(function Select(
  { value, onValueChange, options, placeholder, disabled, invalid = false, name, className, ...rest },
  ref,
) {
  return (
    <S.Root value={value} onValueChange={onValueChange} disabled={disabled} name={name}>
      <S.Trigger
        ref={ref}
        aria-invalid={invalid || undefined}
        className={cn(
          "relative flex h-9.5 w-full min-w-0 items-center rounded-full border bg-surface-2 pl-4 pr-9 text-left text-base text-fg",
          "hover:bg-surface-3 data-[placeholder]:text-fg-subtle",
          "disabled:pointer-events-none disabled:opacity-50 focus-visible:outline-none",
          // Same focus rule as Input: only a valid select turns accent; an invalid one keeps border-danger.
          invalid ? "border-danger" : "border-border focus-visible:border-accent focus-visible:shadow-focus",
          className,
        )}
        {...rest}
      >
        <span className="min-w-0 flex-1 truncate">
          <S.Value placeholder={placeholder} />
        </span>
        <S.Icon className="pointer-events-none absolute right-3 text-fg-muted">
          <Icon.collapse {...iconSize.default} aria-hidden />
        </S.Icon>
      </S.Trigger>
      <S.Portal>
        <S.Content
          position="popper"
          sideOffset={6}
          style={{ minWidth: "var(--radix-select-trigger-width)", maxHeight: "var(--radix-select-content-available-height)" }}
          className="z-50 max-w-96 overflow-hidden rounded-box border border-border bg-surface-2 p-1.5 text-fg shadow-sm"
        >
          <S.Viewport>
            {options.map((o) => (
              <S.Item
                key={o.value}
                value={o.value}
                disabled={o.disabled}
                className={cn(
                  "relative flex h-9 cursor-default select-none items-center rounded-sm pl-3 pr-9 text-base outline-none",
                  "data-[highlighted]:bg-surface-3 data-[disabled]:pointer-events-none data-[disabled]:opacity-50",
                )}
              >
                <S.ItemText asChild>
                  <span className="min-w-0 flex-1 truncate">{o.label}</span>
                </S.ItemText>
                <S.ItemIndicator className="absolute right-3 text-accent">
                  <Icon.check {...iconSize.default} aria-hidden />
                </S.ItemIndicator>
              </S.Item>
            ))}
          </S.Viewport>
        </S.Content>
      </S.Portal>
    </S.Root>
  );
});
