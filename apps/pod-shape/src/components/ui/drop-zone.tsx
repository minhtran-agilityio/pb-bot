import { useId, useRef, useState, type DragEvent, type KeyboardEvent } from "react";
import { cn } from "../../lib/cn";
import { Icon, iconSize } from "../app/icons";

// DESIGN_SYSTEM.md §6.1 DropZone: a drop target for one file. It reports the File and never validates it;
// the screen checks the file and passes back `error`.
export interface DropZoneProps {
  /** Prompt shown when no file is chosen. */
  label: string;
  /** Passed to the hidden file input. */
  accept?: string;
  /** The chosen file; shows its name and size instead of the label. */
  file?: { name: string; size: number } | null;
  /** Error text under the zone (the §6 error state). */
  error?: string;
  disabled?: boolean;
  onFile: (file: File) => void;
  className?: string;
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function DropZone({ label, accept = ".html", file, error, disabled = false, onFile, className }: DropZoneProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const errorId = useId();
  const [dragOver, setDragOver] = useState(false);

  const open = () => {
    if (!disabled) inputRef.current?.click();
  };
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.target !== e.currentTarget) return;
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      open();
    }
  };
  const onDragOver = (e: DragEvent<HTMLDivElement>) => {
    if (disabled) return;
    e.preventDefault();
    setDragOver(true);
  };
  const onDragLeave = (e: DragEvent<HTMLDivElement>) => {
    if (e.currentTarget.contains(e.relatedTarget as Node | null)) return;
    setDragOver(false);
  };
  const onDrop = (e: DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    setDragOver(false);
    const dropped = e.dataTransfer.files?.[0];
    if (!disabled && dropped) onFile(dropped);
  };

  return (
    <div className={cn("flex min-w-0 flex-col gap-1.5", className)}>
      <div
        role="button"
        tabIndex={disabled ? -1 : 0}
        aria-disabled={disabled || undefined}
        aria-invalid={error ? true : undefined}
        aria-describedby={error ? errorId : undefined}
        onClick={open}
        onKeyDown={onKeyDown}
        onDragEnter={onDragOver}
        onDragOver={onDragOver}
        onDragLeave={onDragLeave}
        onDrop={onDrop}
        className={cn(
          "flex min-h-16 min-w-0 cursor-pointer items-center justify-center gap-2 rounded-box border border-dashed px-4 py-3 text-sm",
          "transition-colors duration-160 ease-out",
          disabled && "pointer-events-none opacity-50",
          dragOver
            ? "border-accent bg-accent-soft text-fg"
            : cn(
                "bg-surface-2 text-fg-muted hover:bg-surface-3",
                error ? "border-danger" : "border-border-strong",
              ),
        )}
      >
        <Icon.artifact {...iconSize.default} aria-hidden className="shrink-0" />
        {file ? (
          <span className="flex min-w-0 items-baseline gap-1.5">
            <span className="min-w-0 truncate text-fg" title={file.name}>
              {file.name}
            </span>
            <span className="shrink-0">· {formatSize(file.size)}</span>
          </span>
        ) : (
          <span className="min-w-0 text-center">{label}</span>
        )}
        <input
          ref={inputRef}
          type="file"
          accept={accept}
          disabled={disabled}
          tabIndex={-1}
          hidden
          onClick={(e) => e.stopPropagation()}
          onChange={(e) => {
            const picked = e.target.files?.[0];
            if (picked) onFile(picked);
            e.target.value = "";
          }}
        />
      </div>
      {error ? (
        <p id={errorId} className="text-xs text-danger">
          {error}
        </p>
      ) : null}
    </div>
  );
}
