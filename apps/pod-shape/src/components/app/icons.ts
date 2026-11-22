// src/components/app/icons.ts
// Shape Board icon map (design system v2.0, DESIGN_SYSTEM.md §8).
// One meaning -> one lucide icon. Components import `Icon` from here, never from "lucide-react".
// Adding or changing a meaning is a design system change: ask the user first.
import {
  ArrowUp, Check, ChevronDown, ChevronRight, CircleCheck, CircleDot, CircleSlash,
  Copy, FileCode2, Flag, FolderGit2, GitBranchPlus, GitFork, GitMerge, GitPullRequest,
  Github, LayoutGrid, Loader, LoaderCircle, MessageSquare, Monitor, Moon, Pencil, Plus,
  Puzzle, RefreshCw, Search, Settings, Sun, Terminal, Trash2, TriangleAlert, X,
  type LucideIcon,
} from "lucide-react";

export const Icon = {
  // navigation
  allProjects: LayoutGrid,
  overview: GitFork,
  project: FolderGit2,
  artifact: FileCode2,
  feature: Puzzle,
  milestone: Flag,
  settings: Settings,
  // actions
  add: Plus,
  generate: GitBranchPlus, // "Generate issues" — never Sparkles
  grill: MessageSquare,
  send: ArrowUp,
  edit: Pencil,
  remove: Trash2,
  regenerate: RefreshCw,
  copy: Copy,
  search: Search,
  close: X,
  openGithub: Github, // lucide marks brand icons deprecated; swap if removed
  // run log
  runStep: Terminal,
  loading: LoaderCircle, // button loading spinner
  // issue status (DESIGN_SYSTEM.md §1.3)
  statusOpen: CircleDot,
  statusRunning: Loader, // spins
  statusPr: GitPullRequest,
  statusMerged: GitMerge,
  statusClosed: CircleSlash,
  // feedback
  success: CircleCheck,
  warning: TriangleAlert,
  check: Check,
  // structure
  expand: ChevronRight,
  collapse: ChevronDown,
  // theme
  themeSystem: Monitor,
  themeDark: Moon,
  themeLight: Sun,
} satisfies Record<string, LucideIcon>;

export type IconName = keyof typeof Icon;

/** Fixed size/stroke pairs (§8). Use these, not custom numbers. */
export const iconSize = {
  badge: { size: 12, strokeWidth: 1.5 },
  dense: { size: 14, strokeWidth: 1.5 },
  default: { size: 16, strokeWidth: 1.75 },
  empty: { size: 24, strokeWidth: 1.5 },
} as const;
