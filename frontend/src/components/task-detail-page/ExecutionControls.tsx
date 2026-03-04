import { useEffect, useState } from 'react';
import type { ProjectSettings } from '../../api/projectSettings';
import { DEFAULT_PROJECT_SETTINGS } from '../../api/projectSettings';
import { cancelAttempt } from '../../api/taskAttempts';
import { logger } from '@/lib/logger';

interface ExecutionControlsProps {
  attemptStatus?: string;
  attemptId?: string;
  startedAt?: string | null;
  settings?: Partial<ProjectSettings>;
  onCancelled?: () => void;
}

export function ExecutionControls({
  attemptStatus,
  attemptId,
  startedAt,
  settings = {},
  onCancelled,
}: ExecutionControlsProps) {
  const [elapsedSeconds, setElapsedSeconds] = useState(0);
  const [isCancelling, setIsCancelling] = useState(false);

  const mergedSettings = { ...DEFAULT_PROJECT_SETTINGS, ...settings };
  const isRunning = attemptStatus?.toLowerCase() === 'running';
  const timeoutSeconds = mergedSettings.timeout_mins * 60;

  // Elapsed time counter
  useEffect(() => {
    if (!isRunning || !startedAt) {
      setElapsedSeconds(0);
      return;
    }

    const startTime = new Date(startedAt).getTime();
    const updateElapsed = () => {
      const now = Date.now();
      setElapsedSeconds(Math.floor((now - startTime) / 1000));
    };

    updateElapsed();
    const interval = setInterval(updateElapsed, 1000);

    return () => clearInterval(interval);
  }, [isRunning, startedAt]);

  const handleCancel = async () => {
    if (!attemptId || isCancelling) return;

    setIsCancelling(true);
    try {
      await cancelAttempt(attemptId);
      onCancelled?.();
    } catch (err) {
      logger.error('Failed to cancel:', err);
    } finally {
      setIsCancelling(false);
    }
  };

  const formatTime = (seconds: number) => {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  };

  const progressPercent = Math.min((elapsedSeconds / timeoutSeconds) * 100, 100);
  const isNearTimeout = elapsedSeconds > timeoutSeconds * 0.8;

  // Only show when running or for queued tasks
  if (!isRunning && attemptStatus?.toLowerCase() !== 'queued') {
    return null;
  }

  return (
    <div className="bg-white dark:bg-surface-dark rounded-xl border border-slate-200 dark:border-slate-700 p-4">
      <div className="flex items-center justify-between mb-3">
        <h4 className="text-sm font-bold text-slate-900 dark:text-white uppercase flex items-center gap-2">
          <span className="material-symbols-outlined text-[18px] text-blue-500 animate-pulse">
            pending
          </span>
          Execution
        </h4>
        {isRunning && attemptId && (
          <button
            onClick={handleCancel}
            disabled={isCancelling}
            className="px-3 py-1 text-xs font-medium text-red-600 hover:bg-red-50 dark:hover:bg-red-900/30 rounded-lg transition-colors disabled:opacity-50 flex items-center gap-1"
          >
            <span className="material-symbols-outlined text-[14px]">
              {isCancelling ? 'hourglass_empty' : 'cancel'}
            </span>
            {isCancelling ? 'Cancelling...' : 'Cancel'}
          </button>
        )}
      </div>

      {/* Progress bar */}
      {isRunning && (
        <div className="mb-3">
          <div className="flex items-center justify-between text-xs mb-1">
            <span className="text-slate-600 dark:text-slate-400">
              Elapsed: {formatTime(elapsedSeconds)}
            </span>
            <span className={`${isNearTimeout ? 'text-amber-600' : 'text-slate-500'}`}>
              Timeout: {formatTime(timeoutSeconds)}
            </span>
          </div>
          <div className="h-1.5 bg-slate-200 dark:bg-slate-700 rounded-full overflow-hidden">
            <div
              className={`h-full transition-all duration-1000 ${
                isNearTimeout ? 'bg-amber-500' : 'bg-blue-500'
              }`}
              style={{ width: `${progressPercent}%` }}
            />
          </div>
          {isNearTimeout && (
            <p className="text-xs text-amber-600 mt-1 flex items-center gap-1">
              <span className="material-symbols-outlined text-[12px]">warning</span>
              Approaching timeout limit
            </p>
          )}
        </div>
      )}

      {/* Settings summary */}
      <div className="grid grid-cols-2 gap-2 text-xs">
        <div className="flex items-center gap-2 text-slate-600 dark:text-slate-400">
          <span className="material-symbols-outlined text-[14px]">timer</span>
          <span>Timeout: {mergedSettings.timeout_mins}m</span>
        </div>
        <div className="flex items-center gap-2 text-slate-600 dark:text-slate-400">
          <span className="material-symbols-outlined text-[14px]">replay</span>
          <span>
            {mergedSettings.auto_retry
              ? `Auto-retry (max ${mergedSettings.max_retries})`
              : 'Manual retry only'}
          </span>
        </div>
        <div className="flex items-center gap-2 text-slate-600 dark:text-slate-400">
          <span className="material-symbols-outlined text-[14px]">rate_review</span>
          <span>{mergedSettings.require_review ? 'Review required' : 'Auto-approve'}</span>
        </div>
        <div className="flex items-center gap-2 text-slate-600 dark:text-slate-400">
          <span className="material-symbols-outlined text-[14px]">schedule</span>
          <span>{mergedSettings.retry_backoff} backoff</span>
        </div>
      </div>
    </div>
  );
}
