const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/** DESIGN_SYSTEM.md §9: relative under 7 days ("2h ago"), else "Nov 22". */
export function formatWhen(iso: string, now: Date = new Date()): string {
  const then = new Date(iso);
  const s = Math.max(0, Math.round((now.getTime() - then.getTime()) / 1000));
  if (s < 60) return "just now";
  if (s < 3600) return `${Math.floor(s / 60)}m ago`;
  if (s < 86400) return `${Math.floor(s / 3600)}h ago`;
  if (s < 7 * 86400) return `${Math.floor(s / 86400)}d ago`;
  return `${MONTHS[then.getMonth()]} ${then.getDate()}`;
}

/** "1 issue", "14 issues". */
export function count(n: number, word: string): string {
  return `${n} ${word}${n === 1 ? "" : "s"}`;
}
