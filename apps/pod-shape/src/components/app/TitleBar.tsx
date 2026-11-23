import { Icon, iconSize } from "./icons";
import logo from "../../assets/brand/shape-board-icon.svg";

// DESIGN_SYSTEM.md §6.2 TitleBar: 48px on the bg "desk", no border, logo 20px + breadcrumb.
// The theme control lives at the bottom of the Sidebar (v2.0).
// The macOS traffic lights sit over the left edge (titleBarStyle "Overlay"); pl-6 + the w-12 spacer keep clear of them.
export function TitleBar({ crumbs }: { crumbs: string[] }) {
  return (
    <header data-tauri-drag-region className="flex h-12 shrink-0 items-center gap-3 bg-bg pr-4.5 pl-6 select-none">
      <div data-tauri-drag-region className="w-12 shrink-0" />
      <img src={logo} alt="" width={20} height={20} className="pointer-events-none" />
      <nav aria-label="Breadcrumb" data-tauri-drag-region className="flex min-w-0 items-center gap-1.5 text-base">
        {crumbs.map((c, i) => (
          <span key={i} className="flex items-center gap-1.5" data-tauri-drag-region>
            {i > 0 && <Icon.expand {...iconSize.dense} aria-hidden className="text-fg-subtle" />}
            <span className={i === crumbs.length - 1 ? "truncate font-medium text-fg" : "truncate text-fg-muted"}>{c}</span>
          </span>
        ))}
      </nav>
      <div data-tauri-drag-region className="flex-1 self-stretch" />
    </header>
  );
}
