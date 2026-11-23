// DESIGN_SYSTEM.md §6.2 ArtifactPreviewFrame: shows one stored artifact version from a URL (the preview://
// scheme). SECURITY: the sandbox value is a fixed literal. Never make it a prop, never spread props onto
// the iframe, never add another token (see ArtifactPreviewFrame.test.mjs).
export function ArtifactPreviewFrame({ src, title }: { src: string; title: string }) {
  return (
    <div className="flex flex-col gap-2">
      <div className="h-105 w-full overflow-hidden rounded-box border border-border bg-surface-1">
        <iframe
          className="size-full border-0"
          sandbox="allow-scripts"
          referrerPolicy="no-referrer"
          src={src || undefined}
          title={title}
        />
      </div>
      <p className="text-xs text-fg-subtle">
        Sandboxed: scripts run, but the page cannot reach the app, files or Tauri commands.
      </p>
    </div>
  );
}
