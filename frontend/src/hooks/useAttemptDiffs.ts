import { useEffect, useState } from 'react';
import { getAttemptDiff, getAttemptDiffSummary } from '@/api/taskAttempts';
import type { FileDiffSummary } from '@/types/timeline-log';
import { logger } from '@/lib/logger';

/**
 * Hook to fetch file diff metadata for an attempt.
 *
 * Uses the lightweight diff-summary endpoint by default.
 * While an attempt is still active, it falls back to the live diff endpoint
 * so file-edit rows can show +/− metadata before the attempt finishes.
 */
const LIVE_DIFF_POLL_INTERVAL_MS = 4000;

interface UseAttemptDiffsOptions {
  enablePolling?: boolean;
}

function mapLiveDiffsToSummary(
  files: Awaited<ReturnType<typeof getAttemptDiff>>['files']
): FileDiffSummary[] {
  return files
    .map((file, index) => {
      const filePath = file.new_path || file.old_path || '';
      if (!filePath) return null;
      return {
        id: `live:${filePath}:${index}`,
        file_path: filePath,
        additions: Math.max(0, file.additions ?? 0),
        deletions: Math.max(0, file.deletions ?? 0),
        change_type:
          file.change === 'added'
            ? 'created'
            : file.change === 'deleted'
              ? 'deleted'
              : 'modified',
      } satisfies FileDiffSummary;
    })
    .filter((file): file is FileDiffSummary => file !== null);
}

export function useAttemptDiffs(
  attemptId: string | undefined,
  options: UseAttemptDiffsOptions = {}
) {
  const [diffs, setDiffs] = useState<FileDiffSummary[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const enablePolling = options.enablePolling ?? false;

  useEffect(() => {
    if (!attemptId) {
      setDiffs([]);
      setError(null);
      setIsLoading(false);
      return;
    }

    let mounted = true;
    setIsLoading(true);
    setError(null);

    const refresh = async (showLoading: boolean) => {
      if (showLoading && mounted) {
        setIsLoading(true);
      }

      try {
        let nextDiffs: FileDiffSummary[];

        if (enablePolling) {
          const liveDiff = await getAttemptDiff(attemptId);
          nextDiffs = mapLiveDiffsToSummary(liveDiff.files);
        } else {
          const { files } = await getAttemptDiffSummary(attemptId);
          nextDiffs = files;
        }

        if (!mounted) return;
        setDiffs(nextDiffs);
        setError(null);
      } catch (err) {
        if (!mounted) return;
        logger.error('Failed to fetch file diffs:', err);
        setError(err instanceof Error ? err.message : 'Failed to fetch file diffs');
      } finally {
        if (mounted && showLoading) {
          setIsLoading(false);
        }
      }
    };

    void refresh(true);

    const pollId = enablePolling
      ? window.setInterval(() => {
          if (typeof document !== 'undefined' && document.visibilityState === 'hidden') {
            return;
          }
          void refresh(false);
        }, LIVE_DIFF_POLL_INTERVAL_MS)
      : null;

    return () => {
      mounted = false;
      if (pollId != null) {
        window.clearInterval(pollId);
      }
    };
  }, [attemptId, enablePolling]);

  return { diffs, isLoading, error };
}
