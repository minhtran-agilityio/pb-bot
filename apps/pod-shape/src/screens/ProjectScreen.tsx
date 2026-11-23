import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { PageHeader } from "../components/app/PageHeader";
import { EmptyState } from "../components/app/EmptyState";
import { Button } from "../components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "../components/ui/tabs";
import { notify } from "../components/ui/toast";
import { shortPath, type Project } from "../lib/api";
import { useProjectArtifacts } from "../lib/artifacts";
import { count } from "../lib/time";

type Tab = "artifacts" | "overview" | "history";

// DESIGN_SYSTEM.md §7.1 screen 2, Project -> Artifacts. Tabs: Artifacts · Overview · Grill history.
// Step 2 makes Artifacts work; Overview (step 5) and Grill history (step 3) are EmptyStates.
export function ProjectScreen({ project, home }: { project: Project; home: string | null }) {
  const [tab, setTab] = useState<Tab>("artifacts");
  const { artifacts } = useProjectArtifacts(project.id);
  const url = `https://github.com/${project.repo}`;
  async function openGithub() {
    try {
      await openUrl(url);
    } catch (e) {
      notify.error("Could not open GitHub", String(e));
    }
  }
  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title={project.name}
        meta={
          <>
            <span className="tabular-nums">{shortPath(project.path, home)}</span>
            <span aria-hidden>·</span>
            <span>github.com/{project.repo}</span>
            <span aria-hidden>·</span>
            <span>{count(0, "issue")}</span>
          </>
        }
        actions={
          <>
            <Button variant="secondary" icon="openGithub" onClick={openGithub}>
              Open in GitHub
            </Button>
            {/* Enabled when the Add artifact dialog (Storehouse #78) is merged. */}
            <Button variant="secondary" icon="add" disabled>
              Add artifact
            </Button>
          </>
        }
      />
      <Tabs value={tab} onValueChange={(v) => setTab(v as Tab)} className="flex flex-col gap-6">
        <TabsList className="self-start">
          <TabsTrigger value="artifacts">Artifacts</TabsTrigger>
          <TabsTrigger value="overview">Overview</TabsTrigger>
          <TabsTrigger value="history">Grill history</TabsTrigger>
        </TabsList>
        <TabsContent value="artifacts">
          {artifacts !== null && artifacts.length === 0 && (
            <EmptyState
              icon="artifact"
              title="No artifacts yet"
              text="Publish an artifact from Claude Code in this repo, or add its HTML by hand."
              action={
                <Button variant="secondary" icon="add" disabled>
                  Add artifact
                </Button>
              }
            />
          )}
        </TabsContent>
        <TabsContent value="overview">
          <EmptyState
            icon="overview"
            title="No overview yet"
            text="The tree of features, milestones and issues appears after issues are created."
            action={
              <Button variant="secondary" icon="artifact" onClick={() => setTab("artifacts")}>
                Show artifacts
              </Button>
            }
          />
        </TabsContent>
        <TabsContent value="history">
          <EmptyState
            icon="grill"
            title="No grill sessions yet"
            text="Grill an artifact to shape it into a feature brief."
            action={
              <Button variant="secondary" icon="artifact" onClick={() => setTab("artifacts")}>
                Show artifacts
              </Button>
            }
          />
        </TabsContent>
      </Tabs>
    </div>
  );
}
