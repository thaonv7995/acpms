export function normalizeProjectProgress(progress: number | null | undefined): number {
  if (typeof progress !== 'number' || !Number.isFinite(progress)) {
    return 0;
  }

  return Math.max(0, Math.min(100, Math.round(progress)));
}

export function getProjectProgressColor(progress: number): string {
  const normalized = normalizeProjectProgress(progress);

  if (normalized === 0) {
    return 'bg-slate-400';
  }

  if (normalized === 100) {
    return 'bg-emerald-500';
  }

  return 'bg-sky-500';
}
