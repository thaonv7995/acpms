import { useState, useEffect } from 'react';
import { formatElapsed } from '@/utils/elapsedTime';

const TICK_MS = 10_000; // 10s for "elapsed minutes" granularity

/**
 * Returns a human-readable elapsed duration that updates every TICK_MS when
 * isRunning is true. Use for running attempt status display.
 */
export function useElapsedRealtime(
  startedAt: string | undefined | null,
  isRunning: boolean
): string {
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (!startedAt || !isRunning) return;
    const id = setInterval(() => setNow(Date.now()), TICK_MS);
    return () => clearInterval(id);
  }, [startedAt, isRunning]);

  if (!startedAt || !isRunning) return '';
  // Re-render on interval so formatElapsed(startedAt, undefined) uses current time
  void now;
  return formatElapsed(startedAt, undefined);
}
