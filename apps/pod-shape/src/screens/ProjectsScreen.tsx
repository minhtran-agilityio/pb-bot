import { useState } from "react";
import { NewArtifactsBox } from "../components/app/NewArtifactsBox";
import { PageHeader } from "../components/app/PageHeader";
import { EmptyState } from "../components/app/EmptyState";
import { StatusBar } from "../components/app/StatusBar";
import { Icon, iconSize } from "../components/app/icons";
import { Banner } from "../components/ui/banner";
import { Button } from "../components/ui/button";
import { notify } from "../components/ui/toast";
import { clickableCard } from "../components/ui/card";
import { api, shortPath, type Project } from "../lib/api";
import { count, formatWhen } from "../lib/time";
import { hookMissing, lastHookWrite, useHookInbox } from "../lib/artifacts";

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
  const { status, inbox, counts, reload } = useHookInbox();
  // The owner's project choice per inbox row; until they choose, the auto-selected project (HANDOFF §11.4.3).
  const [chosen, setChosen] = useState<Record<number, string>>({});
  const [busy, setBusy] = useState<Record<number, boolean>>({});

  async function run(id: number, work: () => Promise<void>, done: string, title: string) {
    setBusy((b) => ({ ...b, [id]: true }));
    try {
      await work();
      notify.success(done, title);
    } catch (e) {
      notify.error("Could not update the artifact", String(e));
    } finally {
      setBusy((b) => ({ ...b, [id]: false }));
      reload();
    }
  }

  const rows = inbox.map((a) => ({
    id: String(a.id),
    title: a.title,
    detail: [a.publishedAt ? formatWhen(a.publishedAt) : null, a.cwd ? shortPath(a.cwd, home) : null].filter(Boolean).join(" · "),
    projectId: chosen[a.id] ?? (a.suggestedProjectId !== null ? String(a.suggestedProjectId) : undefined),
    busy: busy[a.id],
  }));
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
        meta={
          // §11.4.7: "Last hook write" stays visible when the New artifacts box is hidden (review F5).
          [count(projects.length, "repo"), count(0, "issue"), rows.length === 0 ? lastHookWrite(status) : null]
            .filter(Boolean)
            .join(" · ")
        }
        actions={
          <Button variant="secondary" icon="add" onClick={onAdd}>
            Add project
          </Button>
        }
      />
      <NewArtifactsBox
        meta={lastHookWrite(status) ?? undefined}
        rows={rows}
        projects={projects.map((p) => ({ value: String(p.id), label: p.name }))}
        onProjectChange={(id, pid) => setChosen((c) => ({ ...c, [Number(id)]: pid }))}
        onAdd={(id) => {
          const row = rows.find((r) => r.id === id);
          const project = projects.find((p) => String(p.id) === row?.projectId);
          if (!row || !project) return;
          run(Number(id), () => api.addInboxArtifact(Number(id), project.id), `Added to ${project.name}`, row.title);
        }}
        onIgnore={(id) => {
          const row = rows.find((r) => r.id === id);
          run(Number(id), () => api.ignoreArtifact(Number(id)), "Artifact ignored", row?.title ?? "");
        }}
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
