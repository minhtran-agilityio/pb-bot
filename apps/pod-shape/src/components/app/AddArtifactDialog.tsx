import { useId, type ReactNode } from "react";
import { Dialog } from "../ui/dialog";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import { Textarea } from "../ui/textarea";
import { DropZone } from "../ui/drop-zone";

// DESIGN_SYSTEM.md §6.2 AddArtifactDialog: the look of the "Add artifact" import dialog (prototype function ch).
// Controlled and dumb: values, errors, helpers, busy and the primary label come in as props; onChange, onFile,
// onSubmit and onOpenChange go out. The screen owns validation, saving and the version lookup.
export type ArtifactField = "link" | "name" | "html";

export interface AddArtifactDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  values: Record<ArtifactField, string>;
  onChange: (field: ArtifactField, value: string) => void;
  file: { name: string; size: number } | null;
  onFile: (file: File) => void;
  errors?: Partial<Record<"file" | ArtifactField, string>>;
  helpers?: Partial<Record<ArtifactField, string>>;
  /** "Add artifact" or "Save version 3". */
  primaryLabel: string;
  /** Fields disabled, primary button loading. */
  busy?: boolean;
  onSubmit: () => void;
}

function Field({
  id,
  label,
  error,
  helper,
  children,
}: {
  id: string;
  label: string;
  error?: string;
  helper?: string;
  children: (describedBy: string | undefined) => ReactNode;
}) {
  const noteId = `${id}-note`;
  const note = error ?? helper;
  return (
    <div className="flex flex-col gap-1.5">
      <label htmlFor={id} className="text-xs font-medium text-fg-muted">
        {label}
      </label>
      {children(note ? noteId : undefined)}
      {note ? (
        <p id={noteId} className={error ? "text-xs text-danger" : "text-xs text-fg-subtle"}>
          {note}
        </p>
      ) : null}
    </div>
  );
}

export function AddArtifactDialog({
  open,
  onOpenChange,
  values,
  onChange,
  file,
  onFile,
  errors = {},
  helpers = {},
  primaryLabel,
  busy = false,
  onSubmit,
}: AddArtifactDialogProps) {
  const uid = useId();
  const ids = { link: `${uid}-link`, name: `${uid}-name`, html: `${uid}-html` };

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      size="lg"
      title="Add artifact"
      footer={
        <>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button variant="primary" icon="add" loading={busy} onClick={onSubmit}>
            {primaryLabel}
          </Button>
        </>
      }
    >
      <form
        className="flex flex-col gap-4"
        onSubmit={(e) => {
          e.preventDefault();
          if (!busy) onSubmit();
        }}
      >
        <p className="text-sm text-fg-muted">
          Shape Board cannot read claude.ai. Drop an .html file, or paste the link and the HTML; the app keeps a copy. Pasting again with the same name adds a new version.
        </p>
        <DropZone label="Drop an .html file here, or paste the link and HTML below" file={file} error={errors.file} disabled={busy} onFile={onFile} />
        <Field id={ids.link} label="Artifact link" error={errors.link} helper={helpers.link}>
          {(d) => (
            <Input
              id={ids.link}
              autoFocus
              placeholder="https://claude.ai/artifact/…"
              value={values.link}
              invalid={!!errors.link}
              disabled={busy}
              aria-describedby={d}
              spellCheck={false}
              autoCapitalize="off"
              autoCorrect="off"
              onChange={(e) => onChange("link", e.target.value)}
            />
          )}
        </Field>
        <Field id={ids.name} label="Name" error={errors.name} helper={helpers.name}>
          {(d) => (
            <Input
              id={ids.name}
              placeholder="settings-page"
              value={values.name}
              invalid={!!errors.name}
              disabled={busy}
              aria-describedby={d}
              spellCheck={false}
              autoCapitalize="off"
              autoCorrect="off"
              onChange={(e) => onChange("name", e.target.value)}
            />
          )}
        </Field>
        <Field id={ids.html} label="HTML" error={errors.html} helper={helpers.html}>
          {(d) => (
            <Textarea
              id={ids.html}
              variant="code"
              rows={4}
              placeholder="<!doctype html>…"
              value={values.html}
              invalid={!!errors.html}
              disabled={busy}
              aria-describedby={d}
              spellCheck={false}
              onChange={(e) => onChange("html", e.target.value)}
            />
          )}
        </Field>
        <button type="submit" hidden />
      </form>
    </Dialog>
  );
}
