import { forwardRef, useCallback, useEffect, useLayoutEffect, useRef, type TextareaHTMLAttributes } from "react";
import { cn } from "../../lib/cn";

// DESIGN_SYSTEM.md §6.1 Textarea: auto-grows from 1 to 10 rows, then scrolls. `variant="code"` = font-mono text-xs.
// Focus and `invalid` follow Input exactly. The screen owns the value, the helper text and the error text.
const MAX_ROWS = 10;

export interface TextareaProps extends TextareaHTMLAttributes<HTMLTextAreaElement> {
  invalid?: boolean;
  variant?: "default" | "code";
}

export const Textarea = forwardRef<HTMLTextAreaElement, TextareaProps>(function Textarea(
  { invalid = false, variant = "default", className, rows = 1, onInput, ...rest },
  forwardedRef,
) {
  const inner = useRef<HTMLTextAreaElement | null>(null);

  const setRef = useCallback(
    (node: HTMLTextAreaElement | null) => {
      inner.current = node;
      if (typeof forwardedRef === "function") forwardedRef(node);
      else if (forwardedRef) forwardedRef.current = node;
    },
    [forwardedRef],
  );

  const resize = useCallback(() => {
    const el = inner.current;
    if (!el) return;
    const cs = getComputedStyle(el);
    const line = parseFloat(cs.lineHeight) || 22;
    const border = parseFloat(cs.borderTopWidth) + parseFloat(cs.borderBottomWidth);
    const pad = parseFloat(cs.paddingTop) + parseFloat(cs.paddingBottom);
    const max = line * MAX_ROWS + pad + border;
    el.style.height = "auto";
    const wanted = el.scrollHeight + border;
    el.style.height = `${Math.min(wanted, max)}px`;
    el.style.overflowY = wanted > max ? "auto" : "hidden";
  }, []);

  // Re-measure after every render: covers a controlled value and a reset.
  useLayoutEffect(resize);

  // A width change with no render (window resize, a panel opening) rewraps the text; a late font load changes line widths.
  useEffect(() => {
    const el = inner.current;
    if (!el) return;
    let alive = true;
    const ro = new ResizeObserver(() => resize());
    ro.observe(el);
    void document.fonts?.ready.then(() => {
      if (alive) resize();
    });
    return () => {
      alive = false;
      ro.disconnect();
    };
  }, [resize]);

  return (
    <textarea
      ref={setRef}
      rows={rows}
      aria-invalid={invalid || undefined}
      onInput={(e) => {
        resize();
        onInput?.(e);
      }}
      className={cn(
        "block w-full min-w-0 resize-none rounded-box border bg-surface-2 px-4 py-2.5 text-fg",
        variant === "code" ? "font-mono text-xs" : "text-base",
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
