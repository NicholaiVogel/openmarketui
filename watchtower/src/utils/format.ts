export function formatDuration(secs: number): string {
  const mins = Math.floor(secs / 60);
  const remainingSecs = secs % 60;
  if (mins > 0) {
    return `${mins}m ${remainingSecs}s`;
  }
  return `${remainingSecs}s`;
}

export function renderProgressBar(pct: number, width: number): string {
  const safeWidth = Math.max(0, Math.floor(Number.isFinite(width) ? width : 0));
  const safePct = Math.max(0, Math.min(100, Number.isFinite(pct) ? pct : 0));
  const filled = Math.min(safeWidth, Math.round((safePct / 100) * safeWidth));
  const empty = safeWidth - filled;
  return "[" + "\u2588".repeat(filled) + "\u2591".repeat(empty) + "]";
}
