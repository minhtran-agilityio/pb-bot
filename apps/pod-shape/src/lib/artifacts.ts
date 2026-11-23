import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  api,
  ARTIFACTS_CHANGED,
  MAX_HTML_BYTES,
  type ArtifactSummary,
  type FieldErrors,
  type HookStatus,
  type InboxItem,
} from "./api";

/** Runs `load` now and again each time the hook sync reports a change (Rust event `artifacts-changed`). */
export function useHookRefresh(load: () => void) {
  const ref = useRef(load);
  ref.current = load;
  useEffect(() => {
    ref.current();
    let stop: (() => void) | undefined;
    let gone = false;
    listen(ARTIFACTS_CHANGED, () => ref.current()).then((un) => (gone ? un() : (stop = un)), () => {});
    return () => {
      gone = true;
      stop?.();
    };
  }, []);
}

/** Hook health, the "New artifacts" inbox and the artifact count per project, for the Projects screen. */
export function useHookInbox() {
  const [status, setStatus] = useState<HookStatus | null>(null);
  const [inbox, setInbox] = useState<InboxItem[]>([]);
  const [counts, setCounts] = useState<Record<string, number>>({});
  const reload = useCallback(() => {
    api.hookStatus().then(setStatus, () => setStatus(null));
    api.listInbox().then(setInbox, () => setInbox([]));
    api.artifactCounts().then(setCounts, () => setCounts({}));
  }, []);
  useHookRefresh(reload);
  return { status, inbox, counts, reload };
}

/** The artifacts of one project, refreshed on every hook change. */
export function useProjectArtifacts(projectId: number) {
  const [artifacts, setArtifacts] = useState<ArtifactSummary[] | null>(null);
  const reload = useCallback(() => {
    api.listArtifacts(projectId).then(setArtifacts, () => setArtifacts([]));
  }, [projectId]);
  useHookRefresh(reload);
  useEffect(reload, [reload]);
  return { artifacts, reload };
}

/** True when the hook is missing: the banner "Artifact hook not found" shows (HANDOFF §11.4.7). */
export function hookMissing(s: HookStatus | null): boolean {
  return !!s && (!s.folderFound || !s.logFound);
}

/** "Last hook write: 3 days ago" (§11.4.7); null when nothing was ever written. */
export function lastHookWrite(s: HookStatus | null, now: Date = new Date()): string | null {
  if (!s || s.lastWrite === null) return null;
  const days = Math.max(0, Math.floor((now.getTime() / 1000 - s.lastWrite) / 86400));
  return `Last hook write: ${days === 0 ? "today" : days === 1 ? "1 day ago" : `${days} days ago`}`;
}

// ---------- Add artifact (paste or drop): behaviour only; the dialog's look is Storehouse part #78 ----------

export interface AddArtifactValues {
  link: string;
  name: string;
  html: string;
  /** Set when the HTML came from a dropped file. */
  file: { name: string; size: number } | null;
}

export const EMPTY_VALUES: AddArtifactValues = { link: "", name: "", html: "", file: null };

const LINK_RE = /^https:\/\/claude\.ai\/(code\/)?artifact\/[A-Za-z0-9_-]{1,64}$/;
const NAME_RE = /^[a-z0-9][a-z0-9-]{0,62}$/;

/** A slug from a file name or title: "20261122T111340Z_Hook Test.html" -> "hook-test". */
export function slugFrom(text: string): string {
  return text
    .replace(/\.html?$/i, "")
    .replace(/^\d{8}T\d{6}Z_/, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 63)
    .replace(/-+$/, "");
}

/** The same field checks Rust runs (store.rs `check_manual` and `import_manual`), so errors show before
 * the save. A paste that makes a new artifact needs the link; a drop or a new version does not. */
export function checkFields(v: AddArtifactValues, isNewVersion = false): FieldErrors {
  const e: FieldErrors = {};
  const link = v.link.trim();
  if (link ? !LINK_RE.test(link) : v.file === null && !isNewVersion) {
    e.link = "Paste a claude.ai artifact link, for example https://claude.ai/artifact/…";
  }
  if (!NAME_RE.test(v.name.trim())) e.name = "Use lower-case letters, digits and dashes, for example settings-page.";
  if (new TextEncoder().encode(v.html).length > MAX_HTML_BYTES) {
    e.html = "The HTML is larger than 5 MB. Shape Board stores files up to 5 MB.";
  } else if (!v.html.trim() || !v.html.includes("<")) {
    e.html = "Paste the HTML of the artifact. In Claude, open the artifact and copy its code.";
  }
  return e;
}

/** Checks a dropped file before it is read: `.html` only, at most 5 MB. Returns the error text or null. */
export function checkDroppedFile(file: File): string | null {
  if (!/\.html$/i.test(file.name)) return "Only .html files up to 5 MB.";
  if (file.size > MAX_HTML_BYTES) return "Only .html files up to 5 MB.";
  return null;
}

/** "48 KB", "1.2 MB" for the drop zone. */
export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${Math.round(n / 1024)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

/**
 * State and actions of the Add artifact dialog for one project. The dialog part (#78) is controlled:
 * it gets `values`, `errors`, `busy` and `primaryLabel`, and calls `change`, `drop` and `submit`.
 */
export function useAddArtifact(projectId: number, onSaved: (n: number, created: boolean, name: string) => void) {
  const [values, setValues] = useState<AddArtifactValues>(EMPTY_VALUES);
  const [errors, setErrors] = useState<FieldErrors>({});
  const [dropError, setDropError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [nextVersion, setNextVersion] = useState(1);

  // Ask Rust which artifact this would add to, so the button says "Save version N".
  useEffect(() => {
    const name = values.name.trim();
    if (!NAME_RE.test(name)) {
      setNextVersion(1);
      return;
    }
    let live = true;
    const link = values.link.trim() || null;
    api.findManualTarget(projectId, link && LINK_RE.test(link) ? link : null, name).then(
      (r) => live && setNextVersion("Ok" in r ? r.Ok.nextVersion : 1),
      () => live && setNextVersion(1),
    );
    return () => {
      live = false;
    };
  }, [projectId, values.link, values.name]);

  /** "Add version" from the preview: the link and name of that artifact are filled in. */
  const prefill = useCallback((link: string | null, name: string) => {
    setValues({ ...EMPTY_VALUES, link: link ?? "", name });
    setErrors({});
    setDropError(null);
  }, []);

  const reset = useCallback(() => {
    setValues(EMPTY_VALUES);
    setErrors({});
    setDropError(null);
    setBusy(false);
  }, []);

  function change(field: "link" | "name" | "html", value: string) {
    setValues((v) => ({ ...v, [field]: value, ...(field === "html" ? { file: null } : {}) }));
    setErrors((e) => ({ ...e, [field]: undefined }));
  }

  async function drop(file: File) {
    const bad = checkDroppedFile(file);
    setDropError(bad);
    if (bad) return;
    const html = await file.text();
    setValues((v) => ({ ...v, html, file: { name: file.name, size: file.size }, name: v.name || slugFrom(file.name) }));
    setErrors((e) => ({ ...e, html: undefined, link: undefined, name: undefined }));
  }

  async function submit() {
    const e = checkFields(values, nextVersion > 1);
    setErrors(e);
    if (Object.keys(e).length) return;
    setBusy(true);
    try {
      const link = values.link.trim() || null;
      const r = await api.importManual(projectId, link, values.name.trim(), values.html, values.file ? "drop" : "paste");
      if ("Err" in r) {
        setErrors(r.Err);
        return;
      }
      onSaved(r.Ok.n, r.Ok.created, values.name.trim());
      reset();
    } catch (err) {
      setErrors({ html: String(err) });
    } finally {
      setBusy(false);
    }
  }

  const primaryLabel = nextVersion > 1 ? `Save version ${nextVersion}` : "Add artifact";
  return { values, errors, dropError, busy, nextVersion, primaryLabel, change, drop, submit, reset, prefill };
}
