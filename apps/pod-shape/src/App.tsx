import { useEffect, useState } from "react";
import { homeDir } from "@tauri-apps/api/path";
import { TitleBar } from "./components/app/TitleBar";
import { Sidebar, SidebarItem, SidebarLabel } from "./components/app/Sidebar";
import { AddProjectDialog } from "./components/app/AddProjectDialog";
import { Toaster, notify } from "./components/ui/toast";
import { TooltipProvider } from "./components/ui/tooltip";
import { ProjectsScreen } from "./screens/ProjectsScreen";
import { ProjectScreen } from "./screens/ProjectScreen";
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

  useEffect(() => {
    homeDir().then((h) => setHome(h.replace(/\/$/, "")), () => setHome(null));
    api.listProjects().then(setProjects, (e) => {
      setProjects([]);
      notify.error("Could not read the project list", String(e));
    });
  }, []);

  const current = projects?.find((p) => p.id === selected) ?? null;

  function added(p: Project) {
    setProjects((prev) => [...(prev ?? []), p].sort((a, b) => a.name.localeCompare(b.name)));
    setSelected(p.id);
    setAdding(false);
    notify.success("Project added", p.repo);
  }

  return (
    <TooltipProvider>
      <div className="flex h-screen flex-col bg-bg">
        <TitleBar crumbs={current ? ["Projects", current.name] : ["Shape Board", "Projects"]} />
        <div className="flex min-h-0 flex-1 gap-3 px-3 pb-3">
          <Sidebar onAddProject={() => setAdding(true)} theme={theme} onCycleTheme={cycle}>
            <SidebarItem icon="allProjects" label="All projects" selected={!current} onClick={() => setSelected(null)} />
            <SidebarLabel>Projects</SidebarLabel>
            {projects?.map((p) => (
              <SidebarItem
                key={p.id}
                icon="project"
                label={p.name}
                count={0}
                selected={p.id === selected}
                onClick={() => setSelected(p.id)}
              />
            ))}
          </Sidebar>
          <main className="min-w-0 flex-1 overflow-y-auto rounded-panel bg-surface-1">
            <div className="px-10 py-8">
              {projects === null ? null : current ? (
                <ProjectScreen project={current} home={home} />
              ) : (
                <ProjectsScreen projects={projects} home={home} onOpen={(p) => setSelected(p.id)} onAdd={() => setAdding(true)} />
              )}
            </div>
          </main>
        </div>
      </div>
      <AddProjectDialog open={adding} onOpenChange={setAdding} onAdded={added} projects={projects ?? []} />
      <Toaster />
    </TooltipProvider>
  );
}

export const throwaway = "bg-red-500";
