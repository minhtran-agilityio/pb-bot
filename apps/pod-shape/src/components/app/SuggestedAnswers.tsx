// DESIGN_SYSTEM.md §6.2 SuggestedAnswers: one Claude question with its answer chips.
// Dumb: props in, events out. The screen owns the answers, says which card is `active` and
// fills the Composer. Keys 1–4 pick (or toggle) only while the card is active and not
// disabled, and never while the owner types in an input or a textarea.
import { useEffect, useId, useRef } from "react";
import { Kbd } from "../ui/kbd";
import { Icon, iconSize } from "./icons";
import { cn } from "../../lib/cn";

export interface SuggestedOption {
  label: string; // keeps "(Recommended)" when Claude put it there
  description?: string;
}

export interface SuggestedAnswersProps {
  index: number; // 1-based: "Question 1 of 3"
  total: number;
  header: string;
  question: string;
  options: SuggestedOption[]; // the screen sends at most 4; the part shows and numbers the first 4
  multiSelect?: boolean; // chips toggle; the screen joins the chosen labels with ", "
  selected?: number[]; // multiSelect only: chosen option indexes, controlled by the screen
  answered?: boolean; // the line and the question only, no chips
  disabled?: boolean; // Claude is running
  active?: boolean; // this is the card the keys 1–4 act on
  onPick: (index: number) => void;
  onToggle?: (index: number) => void;
  onWriteOwn: () => void;
  className?: string;
}

const MAX_OPTIONS = 4;

// Keys typed in a field, or on something inside a dialog or an open listbox, are not answers.
function ignoresKey(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  if (!el || !el.tagName) return false;
  if (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.tagName === "SELECT" || el.isContentEditable) return true;
  return el.closest('[role="dialog"], [role="alertdialog"], [role="listbox"]') !== null;
}

const chip = cn(
  "inline-flex min-h-8.5 max-w-full items-center gap-2 rounded-full border px-4 py-1 text-sm",
  "transition duration-160 ease-out active:scale-97 motion-safe:hover:-translate-y-px",
  "disabled:pointer-events-none disabled:opacity-50",
);

export function SuggestedAnswers({
  index,
  total,
  header,
  question,
  options,
  multiSelect = false,
  selected = [],
  answered = false,
  disabled = false,
  active = false,
  onPick,
  onToggle,
  onWriteOwn,
  className,
}: SuggestedAnswersProps) {
  const shown = options.slice(0, MAX_OPTIONS);
  const live = useRef({ count: shown.length, multiSelect, onPick, onToggle });
  live.current = { count: shown.length, multiSelect, onPick, onToggle };

  const listening = active && !disabled && !answered;
  useEffect(() => {
    if (!listening) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented || e.repeat || e.metaKey || e.ctrlKey || e.altKey || ignoresKey(e.target)) return;
      const n = Number(e.key);
      const cur = live.current;
      if (!Number.isInteger(n) || n < 1 || n > cur.count) return;
      e.preventDefault();
      if (cur.multiSelect) cur.onToggle?.(n - 1);
      else cur.onPick(n - 1);
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [listening]);

  const uid = useId();
  const descId = (i: number) => `${uid}-d${i}`;
  const described = shown.some((o) => o.description);

  return (
    <div className={cn("flex min-w-0 flex-col gap-2", className)}>
      <p className="text-xs font-medium text-fg-subtle">
        Question {index} of {total} · {header}
      </p>
      <p className="text-md text-fg">{question}</p>
      {!answered && (
        <>
          <div className="flex flex-wrap gap-2.5" role="group" aria-label="Suggested answers">
            {shown.map((o, i) => {
              const on = multiSelect && selected.includes(i);
              return (
                <button
                  key={i}
                  type="button"
                  disabled={disabled}
                  aria-pressed={multiSelect ? on : undefined}
                  aria-describedby={o.description ? descId(i) : undefined}
                  onClick={() => (multiSelect ? onToggle?.(i) : onPick(i))}
                  className={cn(
                    chip,
                    on
                      ? "border-accent bg-accent text-accent-fg"
                      : "border-transparent bg-accent-soft text-accent-text hover:border-accent",
                  )}
                >
                  {on ? (
                    <Icon.check {...iconSize.dense} aria-hidden className="flex-none" />
                  ) : (
                    <Kbd tone="accent" aria-hidden className="flex-none">
                      {i + 1}
                    </Kbd>
                  )}
                  <span className="min-w-0 text-left">{o.label}</span>
                </button>
              );
            })}
            <button
              type="button"
              disabled={disabled}
              onClick={onWriteOwn}
              className={cn(chip, "border-border bg-transparent text-fg-muted hover:border-border-strong hover:text-fg")}
            >
              <Icon.edit {...iconSize.dense} aria-hidden className="flex-none" />
              Write my own
            </button>
          </div>
          {described && (
            <ul className="flex flex-col gap-0.5 text-xs text-fg-subtle">
              {shown.map(
                (o, i) =>
                  o.description && (
                    <li key={i} id={descId(i)}>
                      {i + 1}. {o.description}
                    </li>
                  ),
              )}
            </ul>
          )}
        </>
      )}
    </div>
  );
}
