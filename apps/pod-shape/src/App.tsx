import { useEffect, useState } from "react";
import { homeDir } from "@tauri-apps/api/path";
import { TitleBar } from "./components/app/TitleBar";
import { Sidebar, SidebarItem, SidebarLabel } from "./components/app/Sidebar";
import { AddProjectDialog } from "./components/app/AddProjectDialog";
import { Toaster, notify } from "./components/ui/toast";
import { TooltipProvider } from "./components/ui/tooltip";
import { ProjectsScreen } from "./screens/ProjectsScreen";
import { ProjectScreen } from "./screens/ProjectScreen";
import { GrillScreen } from "./screens/GrillScreen";
import { api, type Project } from "./lib/api";
import { useTheme } from "./lib/theme";

// DESIGN_SYSTEM.md §7: the one AppShell. TitleBar 48px on the bg "desk"; below it the Sidebar (256px) and the
// main panel float as rounded panels with a 12px gap and outer padding. Main scrolls on its own.
export default function App() {
  const { theme, cycle } = useTheme();
  const [projects, setProjects] = useState<Project[] | null>(null);
  const [selected, setSelected] = useState<number | null>(null);
  const [adding, setAdding] = useState(false);
  const [home, setHome] = useState<string | null>(null);
  // Build step 3: the Grill screen of one session, inside the selected project.
  const [grill, setGrill] = useState<{ sessionId: number; artifactId: number } | null>(null);

  useEffect(() => {
    homeDir().then((h) => setHome(h.replace(/\/$/, "")), () => setHome(null));
    api.listProjects().then(setProjects, (e) => {
      setProjects([]);
      notify.error("Could not read the project list", String(e));
    });
  }, []);

  const current = projects?.find((p) => p.id === selected) ?? null;
  function select(id: number | null) {
    setSelected(id);
    setGrill(null);
  }

  function added(p: Project) {
    setProjects((prev) => [...(prev ?? []), p].sort((a, b) => a.name.localeCompare(b.name)));
    select(p.id);
    setAdding(false);
    notify.success("Project added", p.repo);
  }

  return (
    <TooltipProvider>
      <div className="flex h-screen flex-col bg-bg">
        <TitleBar crumbs={current ? (grill ? ["Projects", current.name, "Grill"] : ["Projects", current.name]) : ["Shape Board", "Projects"]} />
        <div className="flex min-h-0 flex-1 gap-3 px-3 pb-3">
          <Sidebar onAddProject={() => setAdding(true)} theme={theme} onCycleTheme={cycle}>
            <SidebarItem icon="allProjects" label="All projects" selected={!current} onClick={() => select(null)} />
            <SidebarLabel>Projects</SidebarLabel>
            {projects?.map((p) => (
              <SidebarItem
                key={p.id}
                icon="project"
                label={p.name}
                count={0}
                selected={p.id === selected}
                onClick={() => select(p.id)}
              />
            ))}
          </Sidebar>
          {current && grill ? (
            // The Grill screen owns its main panel (chat column, sticky Composer) and its RightPanel.
            <GrillScreen
              key={grill.sessionId}
              project={current}
              home={home}
              sessionId={grill.sessionId}
              artifactId={grill.artifactId}
              onOpenSession={(sessionId) => setGrill({ sessionId, artifactId: grill.artifactId })}
            />
          ) : (
            <main className="min-w-0 flex-1 overflow-y-auto rounded-panel bg-surface-1">
              <div className="px-10 py-8">
                {projects === null ? null : current ? (
                  <ProjectScreen project={current} home={home} onOpenGrill={(sessionId, artifactId) => setGrill({ sessionId, artifactId })} />
                ) : (
                  <ProjectsScreen projects={projects} home={home} onOpen={(p) => select(p.id)} onAdd={() => setAdding(true)} />
                )}
              </div>
            </main>
          )}
        </div>
      </div>
      <AddProjectDialog open={adding} onOpenChange={setAdding} onAdded={added} projects={projects ?? []} />
      <Toaster />
    </TooltipProvider>
  );
}
