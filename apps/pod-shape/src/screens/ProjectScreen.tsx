import { openUrl } from "@tauri-apps/plugin-opener";
import { PageHeader } from "../components/app/PageHeader";
import { EmptyState } from "../components/app/EmptyState";
import { Button } from "../components/ui/button";
import { notify } from "../components/ui/toast";
import { shortPath, type Project } from "../lib/api";
import { count } from "../lib/time";

// DESIGN_SYSTEM.md §7.1 screen 2, Project -> Artifacts. Step 1 shows the header and the empty state;
// adding an artifact is step 2, so its button is disabled until then.
export function ProjectScreen({ project, home }: { project: Project; home: string | null }) {
  const url = `https://github.com/${project.repo}`;
  async function openGithub() {
    try {
      await openUrl(url);
    } catch (e) {
      notify.error("Could not open GitHub", String(e));
    }
  }
  return (
    <div className="flex flex-col gap-8">
      <PageHeader
        title={project.name}
        meta={
          <>
            <span className="font-mono">{shortPath(project.path, home)}</span>
            <span aria-hidden>·</span>
            <span>github.com/{project.repo}</span>
            <span aria-hidden>·</span>
            <span>{count(0, "issue")}</span>
          </>
        }
        actions={
          <Button variant="secondary" icon="openGithub" onClick={openGithub}>
            Open in GitHub
          </Button>
        }
      />
      <EmptyState
        icon="artifact"
        title="No artifacts yet"
        text="Paste a Claude artifact link and its HTML to start a grill."
        action={
          <Button variant="secondary" icon="add" disabled>
            Add artifact
          </Button>
        }
      />
    </div>
  );
}
