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

// DESIGN_SYSTEM.md §7: the one AppShell. TitleBar 40px, Sidebar 240px, main (bg) scrolls on its own.
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
      <div className="flex h-screen flex-col">
        <TitleBar
          crumbs={current ? ["Projects", current.name] : ["Shape Board", "Projects"]}
          theme={theme}
          onCycleTheme={cycle}
        />
        <div className="flex min-h-0 flex-1">
          <Sidebar onAddProject={() => setAdding(true)}>
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
          <main className="min-w-0 flex-1 overflow-y-auto bg-bg">
            <div className="px-8 py-6">
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
