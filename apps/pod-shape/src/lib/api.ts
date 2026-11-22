import { invoke } from "@tauri-apps/api/core";

export interface Project {
  id: number;
  repo: string;
  name: string;
  path: string;
  createdAt: string;
}

export type CheckKind = "gh" | "claude" | "folder";

export interface CheckResult {
  ok: boolean;
  detail: string;
  ms: number;
}

export const api = {
  listProjects: () => invoke<Project[]>("list_projects"),
  runCheck: (kind: CheckKind, repo: string, path: string) => invoke<CheckResult>("run_check", { kind, repo, path }),
  /** Turns `github.com/o/n`, a full URL or `o/n` into `owner/name`; rejects with the form hint. */
  normalizeRepo: (input: string) => invoke<string>("normalize_repo", { input }),
  addProject: (repo: string, path: string) => invoke<Project>("add_project", { repo, path }),
};

/** Shows `/Users/me/code/x` as `~/code/x`. */
export function shortPath(path: string, home: string | null): string {
  return home && path.startsWith(home + "/") ? "~" + path.slice(home.length) : path;
}
