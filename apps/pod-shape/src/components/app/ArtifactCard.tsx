// DESIGN_SYSTEM.md §6.2 ArtifactCard: one artifact of a project. A plain Card (not clickableCard):
// it holds its own buttons, so it does not lift and nests no interactive element in a button.
// Dumb part: props in, events out. The screen formats the time and fills the chips and grill slots.
import type { ReactNode } from "react";
import { cn } from "../../lib/cn";
import { Button } from "../ui/button";
import { Card } from "../ui/card";
import { Icon, iconSize } from "./icons";
import { PipelineSteps, type PipelineStep } from "./PipelineSteps";
import { StatusBar, type StatusCounts } from "./StatusBar";

export interface ArtifactCardProps {
  name: string;
  /** Preformatted by the screen, for example "2h ago". */
  updatedAt: string;
  summary?: string;
  step: PipelineStep;
  /** Badges, passed by the screen. */
  chips?: ReactNode;
  /** Mini StatusBar, shown only when given. */
  counts?: StatusCounts;
  onPreview: () => void;
  /** The screen's own Start grill button. */
  grillAction?: ReactNode;
  className?: string;
}

export function ArtifactCard({
  name,
  updatedAt,
  summary,
  step,
  chips,
  counts,
  onPreview,
  grillAction,
  className,
}: ArtifactCardProps) {
  return (
    <Card className={cn("flex flex-col gap-2.5", className)}>
      <div className="flex items-center gap-2">
        <Icon.artifact {...iconSize.default} aria-hidden className="flex-none text-fg-muted" />
        <h3 title={name} className="min-w-0 flex-1 truncate text-md leading-5 font-semibold text-fg">
          {name}
        </h3>
        <span className="flex-none whitespace-nowrap text-xs text-fg-subtle">{updatedAt}</span>
      </div>
      {summary ? <p className="line-clamp-2 text-base text-fg-muted">{summary}</p> : null}
      <PipelineSteps current={step} />
      <div className="flex flex-wrap items-center gap-2">
        {chips}
        {counts ? (
          <div className="w-24">
            <StatusBar counts={counts} />
          </div>
        ) : null}
        <div className="ml-auto flex items-center gap-2">
          <Button variant="ghost" size="sm" icon="artifact" onClick={onPreview} aria-label={`Preview ${name}`}>
            Preview
          </Button>
          {grillAction}
        </div>
      </div>
    </Card>
  );
}
