import { useEffect, useRef, useState } from "react";
import { open as pickFolder } from "@tauri-apps/plugin-dialog";
import { Dialog } from "../ui/dialog";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import { RunLog, RunLogItem, type RunState } from "./RunLog";
import { api, type CheckKind, type CheckResult, type Project } from "../../lib/api";

// Add project (prototype screen): repo + local folder, then three checks before the project is saved:
// gh is signed in, claude is installed, and the folder's origin is that GitHub repo.
const REPO_HINT = "Use the form owner/name, for example acme/shop-web.";
const KINDS: CheckKind[] = ["gh", "claude", "folder"];

type Row = { state: RunState; result?: CheckResult };

function label(kind: CheckKind, path: string) {
  if (kind === "gh") return "gh auth status";
  if (kind === "claude") return "claude --version";
  return `git -C ${path} remote get-url origin`;
}

export function AddProjectDialog({
  open,
  onOpenChange,
  onAdded,
  projects,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onAdded: (p: Project) => void;
  projects: Project[];
}) {
  const [repo, setRepo] = useState("");
  const [path, setPath] = useState("");
  const [rows, setRows] = useState<Partial<Record<CheckKind, Row>> | null>(null);
  const [busy, setBusy] = useState(false);
  const [repoError, setRepoError] = useState<string | null>(null);
  const [pathError, setPathError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const inFlight = useRef(false);
  const gen = useRef(0);

  // Check rows from an earlier run name the old folder, so clear them on open and on any input change.
  useEffect(() => {
    if (open) {
      gen.current += 1;
      setRows(null);
    }
  }, [open]);

  function reset() {
    setRepo("");
    setPath("");
    setRows(null);
    setBusy(false);
    setRepoError(null);
    setPathError(null);
    setSaveError(null);
  }

  // The one close path: Cancel, X, Esc and the overlay all end here. A running check or save is stopped,
  // its late result is ignored (generation bump), and nothing is saved.
  function change(next: boolean) {
    if (next) {
      gen.current += 1;
      onOpenChange(true);
      return;
    }
    gen.current += 1;
    if (inFlight.current) void api.cancelChecks().catch(() => {});
    inFlight.current = false;
    reset();
    onOpenChange(false);
  }

  async function choose() {
    const picked = await pickFolder({ directory: true, multiple: false, title: "Choose the repo folder" });
    if (typeof picked === "string") {
      setPath(picked);
      setRows(null);
      setPathError(null);
    }
  }

  async function submit() {
    if (inFlight.current) return;
    inFlight.current = true;
    const g = ++gen.current;
    setBusy(true);
    setRows(null);
    try {
      await run(g);
    } finally {
      if (g === gen.current) {
        inFlight.current = false;
        setBusy(false);
      }
    }
  }

  async function run(g: number) {
    const stale = () => g !== gen.current;
    const input = repo.trim();
    const p = path.trim();
    let r = "";
    let rErr: string | null = null;
    if (!input) {
      rErr = REPO_HINT;
    } else {
      try {
        r = await api.normalizeRepo(input);
        if (projects.some((x) => x.repo.toLowerCase() === r.toLowerCase())) rErr = "This repo is already added.";
      } catch (e) {
        rErr = typeof e === "string" && e ? e : REPO_HINT;
      }
    }
    if (stale()) return;
    let pErr: string | null = null;
    if (!p) pErr = "Pick the folder where this repo is cloned.";
    else if (!(p === "~" || p.startsWith("/") || p.startsWith("~/"))) pErr = "Use a full path, for example ~/code/shop-web.";
    setRepoError(rErr);
    setPathError(pErr);
    setSaveError(null);
    if (rErr || pErr) return;

    setRows(Object.fromEntries(KINDS.map((k) => [k, { state: "running" }])));
    const results = await Promise.all(
      KINDS.map(async (k) => {
        let result: CheckResult;
        try {
          result = await api.runCheck(k, r, p);
        } catch (e) {
          result = { ok: false, detail: String(e), ms: 0 };
        }
        if (!stale()) setRows((prev) => ({ ...prev, [k]: { state: result.ok ? "done" : "failed", result } }));
        return result;
      }),
    );
    if (stale()) return;
    if (results.every((x) => x.ok)) {
      try {
        const project = await api.addProject(r, p);
        if (stale()) return;
        inFlight.current = false;
        reset();
        onAdded(project);
        return;
      } catch (e) {
        if (stale()) return;
        setSaveError(String(e));
      }
    }
  }

  return (
    <Dialog
      open={open}
      onOpenChange={change}
      title="Add project"
      footer={
        <>
          <Button variant="ghost" onClick={() => change(false)}>
            Cancel
          </Button>
          <Button variant="primary" icon="add" loading={busy} onClick={submit}>
            Check and add
          </Button>
        </>
      }
    >
      <form
        className="flex flex-col gap-4"
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        <label className="flex flex-col gap-1.5">
          <span className="text-xs font-medium text-fg-muted">GitHub repo</span>
          <Input
            autoFocus
            placeholder="owner/name"
            value={repo}
            invalid={!!repoError}
            disabled={busy}
            spellCheck={false}
            autoCapitalize="off"
            autoCorrect="off"
            onChange={(e) => {
              setRepo(e.target.value);
              setRows(null);
              setRepoError(null);
            }}
          />
          <span className={repoError ? "text-xs text-danger" : "text-xs text-fg-subtle"}>
            {repoError ?? "1 project = 1 GitHub repo."}
          </span>
        </label>
        <div className="flex flex-col gap-1.5">
          <label htmlFor="project-folder" className="text-xs font-medium text-fg-muted">
            Local folder
          </label>
          <div className="flex gap-2">
            <Input
              id="project-folder"
              placeholder="~/code/shop-web"
              value={path}
              invalid={!!pathError}
              disabled={busy}
              spellCheck={false}
              autoCapitalize="off"
              autoCorrect="off"
              onChange={(e) => {
                setPath(e.target.value);
                setRows(null);
                setPathError(null);
              }}
            />
            <Button variant="secondary" onClick={choose} disabled={busy}>
              Choose…
            </Button>
          </div>
          <span className={pathError ? "text-xs text-danger" : "text-xs text-fg-subtle"}>
            {pathError ?? "Claude Code runs in this folder, so it can read the code."}
          </span>
        </div>
        {rows && (
          <RunLog>
            {KINDS.map((k) => {
              const row = rows[k];
              if (!row) return null;
              return (
                <RunLogItem
                  key={k}
                  state={row.state}
                  action={label(k, path.trim())}
                  ms={row.result?.ms}
                  error={row.result?.ok ? undefined : row.result?.detail}
                />
              );
            })}
          </RunLog>
        )}
        {saveError && <p className="text-sm text-danger">{saveError}</p>}
        <button type="submit" hidden />
      </form>
    </Dialog>
  );
}
