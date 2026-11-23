import { PageHeader } from "../components/app/PageHeader";
import { EmptyState } from "../components/app/EmptyState";
import { StatusBar } from "../components/app/StatusBar";
import { Icon, iconSize } from "../components/app/icons";
import { Banner } from "../components/ui/banner";
import { Button } from "../components/ui/button";
import { clickableCard } from "../components/ui/card";
import { shortPath, type Project } from "../lib/api";
import { count, formatWhen } from "../lib/time";
import { hookMissing, useHookInbox } from "../lib/artifacts";

// DESIGN_SYSTEM.md §7.1 screen 1, Projects (home): PageHeader + grid of project Cards (grid-cards, gap-4):
// repo name, path (UI font, tabular), artifact count, StatusBar of all issues.
// Step 2 adds the artifact hook banners and the real artifact counts; issues arrive in step 4.
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
  const { status, counts } = useHookInbox();
  return (
    <div className="flex flex-col gap-6">
      {hookMissing(status) && (
        <Banner>Artifact hook not found. Add artifacts by hand with Add artifact on a project.</Banner>
      )}
      {status && status.badLines > 0 && (
        <Banner>
          {status.badLines === 1
            ? "1 line of the artifact log could not be read and was skipped."
            : `${status.badLines} lines of the artifact log could not be read and were skipped.`}
        </Banner>
      )}
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
      <div className="grid grid-cards gap-4">
        {projects.map((p) => (
          <button key={p.id} type="button" className={`${clickableCard} flex flex-col gap-2.5`} onClick={() => onOpen(p)}>
            <div className="flex flex-col gap-2.5">
              <h3 className="flex items-center gap-2 text-md leading-5 font-semibold text-fg">
                <Icon.project {...iconSize.default} aria-hidden className="shrink-0" />
                <span className="truncate">{p.name}</span>
              </h3>
              <span className="truncate text-sm leading-4 text-fg-subtle tabular-nums">{shortPath(p.path, home)}</span>
            </div>
            <StatusBar counts={NO_ISSUES} />
            <div className="mt-auto flex items-center justify-between gap-2 text-sm leading-4 text-fg-muted tabular-nums">
              <span>
                {count(counts[String(p.id)] ?? 0, "artifact")} · {count(0, "issue")}
              </span>
              <span>{formatWhen(p.createdAt)}</span>
            </div>
          </button>
        ))}
      </div>
      )}
    </div>
  );
}
