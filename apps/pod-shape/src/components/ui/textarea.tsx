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
  { invalid = false, variant = "default", className, rows = 1, onInput, value, ...rest },
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

  // One hidden mirror measures the content height, so the visible element is never collapsed and the scroll ancestor is never clamped.
  const mirror = useRef<HTMLTextAreaElement | null>(null);

  const resize = useCallback(() => {
    const el = inner.current;
    if (!el) return;
    let m = mirror.current;
    if (!m) {
      m = document.createElement("textarea");
      m.setAttribute("aria-hidden", "true");
      m.tabIndex = -1;
      m.rows = 1;
      m.readOnly = true;
      m.style.cssText = "position:absolute;top:0;left:-9999px;visibility:hidden;pointer-events:none;height:0;min-height:0;overflow:hidden;resize:none;";
      document.body.appendChild(m);
      mirror.current = m;
    }
    const cs = getComputedStyle(el);
    const line = parseFloat(cs.lineHeight) || 22;
    const border = parseFloat(cs.borderTopWidth) + parseFloat(cs.borderBottomWidth);
    const pad = parseFloat(cs.paddingTop) + parseFloat(cs.paddingBottom);
    const max = line * MAX_ROWS + pad + border;
    m.style.boxSizing = "border-box";
    m.style.width = `${el.clientWidth + parseFloat(cs.borderLeftWidth) + parseFloat(cs.borderRightWidth)}px`;
    m.style.font = cs.font;
    m.style.letterSpacing = cs.letterSpacing;
    m.style.lineHeight = cs.lineHeight;
    m.style.padding = cs.padding;
    m.style.borderWidth = cs.borderWidth;
    m.style.borderStyle = "solid";
    m.style.tabSize = cs.tabSize;
    m.style.whiteSpace = cs.whiteSpace;
    m.style.overflowWrap = cs.overflowWrap;
    m.value = el.value;
    const wanted = m.scrollHeight + border;
    const height = `${Math.min(wanted, max)}px`;
    const overflow = wanted > max ? "auto" : "hidden";
    if (el.style.height !== height) el.style.height = height;
    if (el.style.overflowY !== overflow) el.style.overflowY = overflow;
  }, []);

  useEffect(
    () => () => {
      mirror.current?.remove();
      mirror.current = null;
    },
    [],
  );

  // Re-measure when the value changes (controlled value, reset). Typing is handled by onInput.
  useLayoutEffect(resize, [value, resize]);

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
      value={value}
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
