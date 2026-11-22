import { PageHeader } from "../components/app/PageHeader";
import { EmptyState } from "../components/app/EmptyState";
import { StatusBar } from "../components/app/StatusBar";
import { Icon, iconSize } from "../components/app/icons";
import { Button } from "../components/ui/button";
import { clickableCard } from "../components/ui/card";
import { shortPath, type Project } from "../lib/api";
import { count, formatWhen } from "../lib/time";

// DESIGN_SYSTEM.md §7.1 screen 1, Projects (home): PageHeader + grid of project Cards (gap-3):
// repo name, path font-mono, artifact count, StatusBar of all issues.
// Artifacts and issues arrive in steps 2 and 4, so every count is 0 for now.
const NO_ISSUES = { merged: 0, pr: 0, open: 0, closed: 0 };

export function ProjectsScreen({
  projects,
  home,
  onOpen,
  onAdd,
}: {
  projects: Project[];
  home: string | null;
  onOpen: (p: Project) => void;
  onAdd: () => void;
}) {
  return (
    <div className={projects.length === 0 ? "flex flex-col gap-5" : "flex flex-col gap-6"}>
      <PageHeader
        title="Projects"
        meta={`${count(projects.length, "repo")} · ${count(0, "issue")}`}
        actions={
          <Button variant="secondary" icon="add" onClick={onAdd}>
            Add project
          </Button>
        }
      />
      {projects.length === 0 ? (
        <EmptyState
          icon="project"
          title="No projects yet"
          text="Add a GitHub repo and the folder where it is cloned on this Mac."
          action={
            <Button variant="secondary" icon="add" onClick={onAdd}>
              Add project
            </Button>
          }
        />
      ) : (
      <div className="grid grid-cols-2 gap-3 xl:grid-cols-3">
        {projects.map((p) => (
          <button key={p.id} type="button" className={`${clickableCard} flex flex-col gap-3`} onClick={() => onOpen(p)}>
            <div className="flex flex-col gap-1">
              <h3 className="flex items-center gap-2 text-md font-medium text-fg">
                <Icon.project {...iconSize.default} aria-hidden className="shrink-0" />
                <span className="truncate">{p.name}</span>
              </h3>
              <span className="truncate font-mono text-sm text-fg-muted">{shortPath(p.path, home)}</span>
            </div>
            <StatusBar counts={NO_ISSUES} />
            <div className="flex items-center justify-between gap-2 border-t border-border pt-3 text-sm text-fg-muted">
              <span>
                {count(0, "artifact")} · {count(0, "issue")}
              </span>
              <span className="text-fg-subtle tabular-nums">{formatWhen(p.createdAt)}</span>
            </div>
          </button>
        ))}
      </div>
      )}
    </div>
  );
}
