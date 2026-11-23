import { useId } from "react";
import { Badge } from "../ui/badge";
import { Button } from "../ui/button";
import { Select, type SelectOption } from "../ui/select";
import { Icon, iconSize } from "./icons";

// DESIGN_SYSTEM.md §6.2 NewArtifactsBox: artifacts the hook saved that belong to no project yet.
// Card list: header (title + count Badge + meta) then rows divided by 1px border. Dumb: the screen owns
// the rows, the chosen project and the async work. Renders nothing when `rows` is empty.
export interface NewArtifactRow {
  id: string;
  title: string;
  detail: string;
  projectId?: string;
  busy?: boolean;
}

export interface NewArtifactsBoxProps {
  meta?: string;
  rows: NewArtifactRow[];
  projects: SelectOption[];
  onProjectChange: (id: string, projectId: string) => void;
  onAdd: (id: string) => void;
  onIgnore: (id: string) => void;
}

export function NewArtifactsBox({ meta, rows, projects, onProjectChange, onAdd, onIgnore }: NewArtifactsBoxProps) {
  const headingId = useId();
  if (rows.length === 0) return null;
  return (
    <section aria-labelledby={headingId} className="overflow-hidden rounded-card border border-border bg-surface-2 shadow-card">
      <header className="flex items-center gap-2.5 px-5 py-3.5">
        <h2 id={headingId} className="text-md font-semibold text-fg">
          New artifacts
        </h2>
        <Badge aria-hidden>{rows.length}</Badge>
        <span className="sr-only">{rows.length === 1 ? "1 artifact" : `${rows.length} artifacts`}</span>
        {meta && <span className="ml-auto text-sm text-fg-muted">{meta}</span>}
      </header>
      <ul className="divide-y divide-border border-t border-border">
        {rows.map((r) => (
          <li key={r.id} className="flex items-center gap-3 px-5 py-3 transition-colors duration-160 ease-out hover:bg-surface-3">
            <Icon.artifact {...iconSize.default} aria-hidden className="shrink-0 text-fg-muted" />
            <div className="min-w-0 flex-1">
              <p className="truncate text-base font-medium text-fg">{r.title}</p>
              <p className="truncate text-sm text-fg-muted">{r.detail}</p>
            </div>
            <div className="w-48 shrink-0">
              <Select
                options={projects}
                value={r.projectId}
                placeholder="Choose a project"
                disabled={r.busy}
                aria-label={`Project for ${r.title}`}
                onValueChange={(v) => onProjectChange(r.id, v)}
              />
            </div>
            <Button
              size="sm"
              variant="secondary"
              icon="add"
              aria-label={`Add to project: ${r.title}`}
              disabled={!r.projectId || !projects.some((p) => p.value === r.projectId)}
              loading={r.busy}
              onClick={() => onAdd(r.id)}
            >
              Add to project
            </Button>
            <Button size="sm" variant="ghost" aria-label={`Ignore: ${r.title}`} disabled={r.busy} onClick={() => onIgnore(r.id)}>
              Ignore
            </Button>
          </li>
        ))}
      </ul>
    </section>
  );
}
