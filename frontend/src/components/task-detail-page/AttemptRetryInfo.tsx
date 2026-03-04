import { useEffect, useState } from 'react';
import type { RetryInfo } from '../../api/taskAttempts';
import { getRetryInfo, retryAttempt } from '../../api/taskAttempts';
import { logger } from '@/lib/logger';

interface AttemptRetryInfoProps {
  attemptId: string;
  status: string;
  onRetryTriggered?: () => void;
}

export function AttemptRetryInfo({ attemptId, status, onRetryTriggered }: AttemptRetryInfoProps) {
  const [retryInfo, setRetryInfo] = useState<RetryInfo | null>(null);
  const [isRetrying, setIsRetrying] = useState(false);
  const [countdown, setCountdown] = useState<number | null>(null);

  useEffect(() => {
    // Only fetch retry info for failed attempts
    if (status.toLowerCase() !== 'failed') return;

    getRetryInfo(attemptId)
      .then(setRetryInfo)
      .catch(() => setRetryInfo(null));
  }, [attemptId, status]);

  // Countdown timer for backoff
  useEffect(() => {
    if (!retryInfo?.next_backoff_seconds) {
      setCountdown(null);
      return;
    }

    setCountdown(retryInfo.next_backoff_seconds);

    const interval = setInterval(() => {
      setCountdown((prev) => {
        if (prev === null || prev <= 1) {
          clearInterval(interval);
          return null;
        }
        return prev - 1;
      });
    }, 1000);

    return () => clearInterval(interval);
  }, [retryInfo?.next_backoff_seconds]);

  const handleManualRetry = async () => {
    if (isRetrying || !retryInfo?.can_retry) return;

    setIsRetrying(true);
    try {
      await retryAttempt(attemptId);
      onRetryTriggered?.();
    } catch (err) {
      logger.error('Failed to trigger retry:', err);
    } finally {
      setIsRetrying(false);
    }
  };

  // Don't show anything for non-failed attempts or if no retry info
  if (status.toLowerCase() !== 'failed' || !retryInfo) return null;

  const { retry_count, max_retries, previous_error, can_retry, auto_retry_enabled, next_retry_attempt_id } = retryInfo;

  return (
    <div className="mt-2 pt-2 border-t border-slate-200 dark:border-slate-700">
      {/* Retry count indicator */}
      {retry_count > 0 && (
        <div className="flex items-center gap-2 text-xs text-amber-600 dark:text-amber-400 mb-1">
          <span className="material-symbols-outlined text-[14px]">replay</span>
          <span>Retry {retry_count}/{max_retries}</span>
          {previous_error && (
            <span className="text-slate-500 dark:text-slate-400 truncate max-w-[200px]" title={previous_error}>
              • Previous: {previous_error.slice(0, 50)}{previous_error.length > 50 ? '...' : ''}
            </span>
          )}
        </div>
      )}

      {/* Auto-retry scheduled indicator */}
      {next_retry_attempt_id && (
        <div className="flex items-center gap-2 text-xs text-blue-600 dark:text-blue-400">
          <span className="material-symbols-outlined text-[14px] animate-spin">sync</span>
          <span>Auto-retry scheduled</span>
          {countdown !== null && countdown > 0 && (
            <span className="text-slate-500">• in {countdown}s</span>
          )}
        </div>
      )}

      {/* Manual retry button */}
      {can_retry && !next_retry_attempt_id && (
        <button
          onClick={handleManualRetry}
          disabled={isRetrying}
          className="mt-1 inline-flex items-center gap-1 px-2 py-1 text-xs font-medium text-blue-600 dark:text-blue-400 hover:bg-blue-50 dark:hover:bg-blue-900/30 rounded transition-colors disabled:opacity-50"
        >
          <span className="material-symbols-outlined text-[14px]">
            {isRetrying ? 'hourglass_empty' : 'replay'}
          </span>
          {isRetrying ? 'Retrying...' : 'Retry'}
        </button>
      )}

      {/* Max retries exhausted */}
      {!can_retry && retry_count >= max_retries && (
        <div className="flex items-center gap-2 text-xs text-red-600 dark:text-red-400">
          <span className="material-symbols-outlined text-[14px]">error</span>
          <span>Max retries ({max_retries}) exhausted</span>
        </div>
      )}

      {/* Auto-retry status */}
      {!auto_retry_enabled && can_retry && (
        <div className="text-xs text-slate-500 dark:text-slate-400 mt-1">
          Auto-retry disabled • Manual retry available
        </div>
      )}
    </div>
  );
}
