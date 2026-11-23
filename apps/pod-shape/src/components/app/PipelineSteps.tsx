// DESIGN_SYSTEM.md §6.2 PipelineSteps and §6.4 (branch line): where an artifact is in the
// flow Prototype → Grill → Generate → Issues. Display only; the current step is announced
// with aria-current="step" and a visually hidden state word, not by colour alone.
import { cn } from "../../lib/cn";

export type PipelineStep = "prototype" | "grill" | "generate" | "issues";

const STEPS: { key: PipelineStep; label: string }[] = [
  { key: "prototype", label: "Prototype" },
  { key: "grill", label: "Grill" },
  { key: "generate", label: "Generate" },
  { key: "issues", label: "Issues" },
];

export function PipelineSteps({ current, className = "" }: { current: PipelineStep; className?: string }) {
  const at = STEPS.findIndex((s) => s.key === current);
  return (
    <ol aria-label="Pipeline" className={cn("flex items-center text-xs", className)}>
      {STEPS.map((step, i) => {
        const state = i < at ? "done" : i === at ? "current" : "later";
        const node =
          state === "current"
            ? "size-2.5 bg-accent ring-4 ring-accent-soft"
            : state === "done"
              ? "size-2 bg-fg-muted"
              : "size-2 border-2 border-border-strong";
        return (
          <li key={step.key} className="flex items-center">
            {i > 0 && <span aria-hidden className="mx-2 h-0 w-6 flex-none border-t-2 border-branch" />}
            <span
              aria-current={state === "current" ? "step" : undefined}
              className={cn(
                "flex items-center gap-1.5 whitespace-nowrap",
                state === "later" ? "text-fg-subtle" : state === "current" ? "font-medium text-fg" : "text-fg",
              )}
            >
              <span aria-hidden className={cn("flex-none rounded-full", node)} />
              {step.label}
              <span className="sr-only">
                {state === "done" ? " (done)" : state === "later" ? " (later)" : " (current)"}
              </span>
            </span>
          </li>
        );
      })}
    </ol>
  );
}
