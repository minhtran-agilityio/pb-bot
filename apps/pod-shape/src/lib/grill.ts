import { invoke } from "@tauri-apps/api/core";

// Build step 3: the grill. Types mirror Rust `grill::store` (camelCase). The UI never parses
// stream-json: Rust stores everything and emits GRILL_CHANGED; the screen reloads the session.

export type GrillState = "running" | "waiting_owner" | "brief_ready" | "confirmed" | "stopped" | "failed";

export interface GrillSessionSummary {
  id: number;
  artifactId: number;
  artifactVersionId: number;
  versionN: number;
  /** "Session 2" per artifact. */
  n: number;
  state: GrillState;
  /** The confirmed feature's name, else the latest brief's title. */
  featureName: string | null;
  messages: number;
  answers: number;
  createdAt: string;
  updatedAt: string;
}

export type MessageRole = "owner" | "claude" | "tool" | "error" | "note";

export interface P2gOption {
  label: string;
  description: string;
  /** Not shown in step 3 (owner decision). */
  preview?: string;
}

export interface P2gQuestion {
  question: string;
  header: string;
  multiSelect: boolean;
  options: P2gOption[];
}

export interface GrillMessage {
  id: number;
  turn: number;
  role: MessageRole;
  text: string;
  questions: { questions: P2gQuestion[] } | null;
  answered: boolean;
  interrupted: boolean;
  createdAt: string;
}

export interface GrillRun {
  id: number;
  step: "grill" | "adr" | "tickets" | "create";
  command: string;
  exitCode: number | null;
  error: string | null;
  ms: number | null;
  running: boolean;
}

export interface GrillBrief {
  id: number;
  n: number;
  /** specs/<slug>/brief.md, relative to the project folder. */
  path: string;
  content: string;
  confirmed: boolean;
}

export interface GrillFeature {
  id: number;
  name: string;
  slug: string;
}

export interface OpenBlock {
  messageId: number;
  questions: { questions: P2gQuestion[] };
  /** Answers given so far, by question text. */
  drafts: Record<string, string>;
}

export interface GrillDetail {
  session: GrillSessionSummary;
  error: string | null;
  hasPending: boolean;
  adrPending: boolean;
  projectPath: string;
  messages: GrillMessage[];
  runs: GrillRun[];
  brief: GrillBrief | null;
  feature: GrillFeature | null;
  openBlock: OpenBlock | null;
}

export const GRILL_CHANGED = "grill-changed";

export const grillApi = {
  start: (versionId: number) => invoke<number>("grill_start", { versionId }),
  sessions: (artifactId: number) => invoke<GrillSessionSummary[]>("grill_sessions", { artifactId }),
  latest: (projectId: number) => invoke<GrillSessionSummary[]>("grill_latest", { projectId }),
  detail: (sessionId: number) => invoke<GrillDetail>("grill_detail", { sessionId }),
  /** Resolves true when the block was complete and the turn was sent. */
  answer: (sessionId: number, question: string, answer: string, adr = false) =>
    invoke<boolean>("grill_answer", { sessionId, question, answer, adr }),
  send: (sessionId: number, text: string) => invoke<void>("grill_send", { sessionId, text }),
  confirmBrief: (sessionId: number) => invoke<void>("grill_confirm_brief", { sessionId }),
  stop: (sessionId: number) => invoke<void>("grill_stop", { sessionId }),
  resume: (sessionId: number) => invoke<void>("grill_resume", { sessionId }),
};

export {
  joinChoices,
  nextQuestion,
  isAdrQuestion,
  isAdrYes,
  isBriefQuestion,
  optionLabels,
  STATE_LABEL,
  generateTooltip,
  composerPlaceholder,
} from "./grillHelpers";
