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

/** Hook health for the banner and "Last hook write" (Rust `HookStatus`). */
export interface HookStatus {
  folderFound: boolean;
  logFound: boolean;
  /** Unix seconds. */
  lastWrite: number | null;
  badLines: number;
}

export interface SyncReport {
  newVersions: number;
  badLines: number;
  skippedFiles: number;
}

/** A hook artifact in no project yet (the "New artifacts" box). */
export interface InboxItem {
  id: number;
  title: string;
  link: string | null;
  publishedAt: string | null;
  cwd: string | null;
  versions: number;
  /** The project whose folder holds `cwd` (longest match), else null. */
  suggestedProjectId: number | null;
}

export interface ArtifactSummary {
  id: number;
  name: string;
  title: string;
  link: string | null;
  cwd: string | null;
  source: "hook" | "manual";
  /** A new version arrived since the owner last opened the preview. */
  updated: boolean;
  versions: number;
  latestAt: string;
}

export interface VersionInfo {
  id: number;
  n: number;
  source: "hook" | "drop" | "paste";
  createdAt: string;
  bytes: number;
}

export interface FieldErrors {
  link?: string;
  name?: string;
  html?: string;
}

export interface ManualTarget {
  artifactId: number | null;
  nextVersion: number;
}

export interface ManualResult {
  artifactId: number;
  versionId: number;
  n: number;
  /** False when the HTML equals the latest version (nothing new was saved). */
  created: boolean;
}

/** Rust returns `Result<T, FieldErrors>` as `{Ok: T}` or `{Err: FieldErrors}`. */
export type Checked<T> = { Ok: T } | { Err: FieldErrors };

/** The largest HTML the app stores (HANDOFF §11.4.7); Rust checks it again. */
export const MAX_HTML_BYTES = 5 * 1024 * 1024;

/** The event Rust emits after each hook sync. */
export const ARTIFACTS_CHANGED = "artifacts-changed";

/** URL of one stored version for the sandboxed preview frame (served by Rust with its own CSP).
 * macOS only (owner decision); Windows would need http://preview.localhost, which Rust allows only there. */
export function previewUrl(versionId: number): string {
  return `preview://localhost/v/${versionId}`;
}

export const api = {
  listProjects: () => invoke<Project[]>("list_projects"),
  runCheck: (kind: CheckKind, repo: string, path: string) => invoke<CheckResult>("run_check", { kind, repo, path }),
  /** Turns `github.com/o/n`, a full URL or `o/n` into `owner/name`; rejects with the form hint. */
  normalizeRepo: (input: string) => invoke<string>("normalize_repo", { input }),
  /** Kills every running check; in-flight run_check calls resolve with "Cancelled.". */
  cancelChecks: () => invoke<void>("cancel_checks"),
  addProject: (repo: string, path: string) => invoke<Project>("add_project", { repo, path }),

  // Step 2: artifacts
  hookStatus: () => invoke<HookStatus>("hook_status"),
  rescanHook: () => invoke<SyncReport>("rescan_hook"),
  listInbox: () => invoke<InboxItem[]>("list_inbox"),
  addInboxArtifact: (artifactId: number, projectId: number) => invoke<void>("add_inbox_artifact", { artifactId, projectId }),
  ignoreArtifact: (artifactId: number) => invoke<void>("ignore_artifact", { artifactId }),
  listArtifacts: (projectId: number) => invoke<ArtifactSummary[]>("list_artifacts", { projectId }),
  /** Artifact count per project id; projects with none are missing. */
  artifactCounts: () => invoke<Record<string, number>>("artifact_counts"),
  listVersions: (artifactId: number) => invoke<VersionInfo[]>("list_versions", { artifactId }),
  markArtifactSeen: (artifactId: number) => invoke<void>("mark_artifact_seen", { artifactId }),
  findManualTarget: (projectId: number, link: string | null, name: string) =>
    invoke<Checked<ManualTarget>>("find_manual_target", { projectId, link, name }),
  importManual: (projectId: number, link: string | null, name: string, html: string, source: "paste" | "drop") =>
    invoke<Checked<ManualResult>>("import_manual", { projectId, link, name, html, source }),
};

/** Shows `/Users/me/code/x` as `~/code/x`. */
export function shortPath(path: string, home: string | null): string {
  return home && path.startsWith(home + "/") ? "~" + path.slice(home.length) : path;
}
