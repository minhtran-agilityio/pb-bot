import { useEffect, useState } from "react";
import { ArtifactPreviewFrame } from "../components/app/ArtifactPreviewFrame";
import { Button } from "../components/ui/button";
import { Dialog } from "../components/ui/dialog";
import { Select } from "../components/ui/select";
import { listen } from "@tauri-apps/api/event";
import { api, ARTIFACTS_CHANGED, type ArtifactSummary, type VersionInfo } from "../lib/api";
import { formatWhen } from "../lib/time";

// The artifact preview (prototype: "Preview" on an ArtifactCard; HANDOFF §5: a Dialog, not a screen).
// Dialog lg: Version Select + link, the sandboxed frame (Storehouse part, sandbox fixed), footer
// "Paste new version" + "Close". The frame loads preview://localhost/v/<id>, which Rust serves with its own CSP.
// Opening it clears the "Updated" mark (owner decision 2026-11-23).
export function ArtifactPreviewDialog({
  artifact,
  onClose,
  onAddVersion,
  onSeen,
}: {
  artifact: ArtifactSummary | null;
  onClose: () => void;
  /** Opens the Add artifact dialog for a new version; undefined disables the button. */
  onAddVersion?: (a: ArtifactSummary) => void;
  onSeen?: () => void;
}) {
  const [versions, setVersions] = useState<VersionInfo[] | null>(null);
  const [selected, setSelected] = useState<string | undefined>(undefined);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setVersions(null);
    setSelected(undefined);
    setError(null);
    if (!artifact) return;
    let live = true;
    api.listVersions(artifact.id).then(
      (v) => {
        if (!live) return;
        setVersions(v);
        setSelected(v[0] ? String(v[0].id) : undefined);
      },
      (e) => live && setError(String(e)),
    );
    if (artifact.updated) api.markArtifactSeen(artifact.id).then(() => onSeen?.(), () => {});
    return () => {
      live = false;
    };
    // Reload only when another artifact opens.
  }, [artifact?.id]);

  // A version that arrives while the dialog is open joins the list; the shown version stays.
  useEffect(() => {
    if (!artifact) return;
    let stop: (() => void) | undefined;
    let gone = false;
    listen(ARTIFACTS_CHANGED, () => {
      api.listVersions(artifact.id).then((v) => !gone && setVersions(v), () => {});
    }).then((un) => (gone ? un() : (stop = un)), () => {});
    return () => {
      gone = true;
      stop?.();
    };
  }, [artifact?.id]);

  const current = versions?.find((v) => String(v.id) === selected) ?? null;

  // Each load of the frame gets a fresh single-use URL from Rust (review F6). A page that reloads
  // itself keeps showing the old page on WKWebView, one more request gets a 404 (known limit); "Reload" mints a new URL (owner decision 2026-11-23, round 2 N2).
  const [src, setSrc] = useState<{ id: number; url: string } | null>(null);
  const [loads, setLoads] = useState(0);
  const [minting, setMinting] = useState(false);
  useEffect(() => {
    setSrc(null);
    setError(null);
    if (!current) return;
    let live = true;
    setMinting(true);
    api.previewUrl(current.id).then(
      (url) => live && setSrc({ id: current.id, url }),
      (e) => live && setError(String(e)),
    ).finally(() => live && setMinting(false));
    return () => {
      live = false;
    };
  }, [current?.id, loads]);
  return (
    <Dialog
      open={artifact !== null}
      onOpenChange={(o) => !o && onClose()}
      size="lg"
      title={artifact ? `Preview: ${artifact.name}` : "Preview"}
      footer={
        <>
          <Button variant="ghost" icon="add" disabled={!onAddVersion || !artifact} onClick={() => artifact && onAddVersion?.(artifact)}>
            Paste new version
          </Button>
          <Button variant="secondary" onClick={onClose}>
            Close
          </Button>
        </>
      }
    >
      <div className="flex min-w-0 items-center gap-3">
        <div className="w-50 shrink-0">
          <Select
            aria-label="Version"
            value={selected}
            onValueChange={setSelected}
            disabled={!versions || versions.length < 2}
            options={(versions ?? []).map((v) => ({ value: String(v.id), label: `Version ${v.n} · ${formatWhen(v.createdAt)}` }))}
            placeholder="Version"
          />
        </div>
        {artifact?.link && <span className="min-w-0 flex-1 truncate text-sm text-fg-subtle tabular-nums">{artifact.link}</span>}
        <Button variant="ghost" size="sm" icon="regenerate" loading={minting} disabled={!current} onClick={() => setLoads((n) => n + 1)}>
          Reload
        </Button>
      </div>
      {error ? (
        <p className="text-sm text-danger">{error}</p>
      ) : (
        <ArtifactPreviewFrame
          key={src?.url ?? "none"}
          src={src?.url ?? ""}
          title={artifact && current ? `Preview of ${artifact.name}, version ${current.n}` : "Preview"}
        />
      )}
    </Dialog>
  );
}
