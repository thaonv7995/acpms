import { useEffect, useState } from 'react';
import { getAttemptDiffSummary } from '@/api/taskAttempts';
import type { FileDiffSummary } from '@/types/timeline-log';
import { logger } from '@/lib/logger';

/**
 * Hook to fetch file diff metadata for an attempt.
 *
 * Uses the lightweight diff-summary endpoint (no log processing).
 * Use for timeline enrichment instead of structured-logs.
 */
export function useAttemptDiffs(attemptId: string | undefined) {
  const [diffs, setDiffs] = useState<FileDiffSummary[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!attemptId) {
      setDiffs([]);
      return;
    }

    let mounted = true;
    setIsLoading(true);
    setError(null);

    getAttemptDiffSummary(attemptId)
      .then(({ files }) => {
        if (!mounted) return;
        setDiffs(files);
        setIsLoading(false);
      })
      .catch(err => {
        if (!mounted) return;
        logger.error('Failed to fetch file diffs:', err);
        setError(err instanceof Error ? err.message : 'Failed to fetch file diffs');
        setIsLoading(false);
      });

    return () => {
      mounted = false;
    };
  }, [attemptId]);

  return { diffs, isLoading, error };
}
