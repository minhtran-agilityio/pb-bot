// Pure helpers of the Grill screen, with no runtime imports, so `node --test grill.test.mjs` runs them
// without the Tauri packages. Re-exported by grill.ts.
import type { GrillState, OpenBlock, P2gQuestion } from "./grill";

/** The answer for a multiSelect question: the chosen labels joined with ", " (owner decision). */
export function joinChoices(labels: string[]): string {
  return labels.join(", ");
}

/** The question of the open block that still needs an answer, in order. */
export function nextQuestion(block: OpenBlock | null): { index: number; question: P2gQuestion } | null {
  if (!block) return null;
  const qs = block.questions.questions;
  const index = qs.findIndex((q) => !(q.question in block.drafts));
  return index < 0 ? null : { index, question: qs[index] };
}

/** An ADR question by its header (the app prompt asks Claude for exactly "ADR"). */
export function isAdrQuestion(q: P2gQuestion): boolean {
  return q.header.trim().toLowerCase() === "adr";
}

/** Rust's `is_adr_yes`: a label that starts with "Yes" or "Make" (fail closed). */
export function isAdrYes(answer: string): boolean {
  return /^\s*(yes|make)(?![a-z0-9])/i.test(answer);
}

export function isBriefQuestion(q: P2gQuestion): boolean {
  return q.header.trim().toLowerCase() === "brief" || q.question.toLowerCase().includes("brief");
}

/** The labels as written; "(Recommended)" stays in the label, as the protocol sends it. */
export function optionLabels(q: P2gQuestion): string[] {
  return q.options.map((o) => o.label);
}

/** The session status words of the prototype (`is`) for SessionStatus. */
export const STATE_LABEL: Record<GrillState, string> = {
  running: "Claude is working",
  waiting_owner: "Waiting for you",
  brief_ready: "Brief ready",
  confirmed: "Confirmed",
  stopped: "Stopped",
  failed: "Failed",
};

/** Generate issues stays disabled in step 3 (owner decision); this is its tooltip. */
export function generateTooltip(state: GrillState, briefConfirmed: boolean): string {
  return briefConfirmed || state === "confirmed" ? "Generate issues comes in build step 4" : "Confirm the brief first";
}

/** The Composer placeholder per state (prototype). */
export function composerPlaceholder(state: GrillState, open: P2gQuestion | null): string {
  if (state === "brief_ready") return "Ask for a change to the brief…";
  if (open) return `Answer, or pick a suggestion (1–${open.options.length})…`;
  if (state === "waiting_owner") return "Write your answer…";
  if (state === "confirmed") return "This session is done. Start a new grill for another feature.";
  if (state === "stopped" || state === "failed") return "Press Resume to continue this session.";
  return "Wait for Claude…";
}
