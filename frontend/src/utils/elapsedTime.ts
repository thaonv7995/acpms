/**
 * Elapsed time utilities for attempt runtime tracking.
 * Uses start timestamp and optional end (null = now for realtime).
 */

/**
 * Elapsed milliseconds from start to end (or now if end is null/undefined).
 */
export function getElapsedMs(
  startIso: string,
  endIso?: string | null
): number {
  const start = new Date(startIso).getTime();
  const end = endIso ? new Date(endIso).getTime() : Date.now();
  return Math.max(0, end - start);
}

/**
 * Elapsed whole minutes from start to end (or now).
 */
export function getElapsedMinutes(
  startIso: string,
  endIso?: string | null
): number {
  return Math.floor(getElapsedMs(startIso, endIso) / 60_000);
}

/**
 * Human-readable elapsed duration, e.g. "2m 30s" or "45s".
 * When end is null/undefined, uses current time (for live display).
 */
export function formatElapsed(
  startIso: string,
  endIso?: string | null
): string {
  const ms = getElapsedMs(startIso, endIso);
  const totalSeconds = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;

  if (minutes > 0) {
    return `${minutes}m ${seconds}s`;
  }
  return `${seconds}s`;
}
