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

export function getProjectProgressTextColor(progress: number): string {
  const normalized = normalizeProjectProgress(progress);

  if (normalized === 0) {
    return 'text-slate-500 dark:text-slate-400';
  }

  if (normalized === 100) {
    return 'text-emerald-600 dark:text-emerald-400';
  }

  return 'text-sky-600 dark:text-sky-400';
}

export function getProjectProgressAccentColor(progress: number): string {
  const normalized = normalizeProjectProgress(progress);

  if (normalized === 0) {
    return 'bg-slate-500/10 dark:bg-slate-500/20 text-slate-500 dark:text-slate-300';
  }

  if (normalized === 100) {
    return 'bg-emerald-500/10 dark:bg-emerald-500/20 text-emerald-500 dark:text-emerald-400';
  }

  return 'bg-sky-500/10 dark:bg-sky-500/20 text-sky-500 dark:text-sky-400';
}
