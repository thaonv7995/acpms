import { useCallback, useState } from 'react';
import { resetExecutionProcess, type ResetExecutionProcessResponse } from '@/api/executionProcesses';

const FORCE_RESET_HINT = 'force_when_dirty=true';

export function isForceResetRequiredError(message: string): boolean {
  return message.includes(FORCE_RESET_HINT);
}

export function buildResetSuccessMessage(response: ResetExecutionProcessResponse): string {
  if (!response.git_reset_applied) {
    return 'Execution process reset acknowledged.';
  }

  if (response.worktree_was_dirty) {
    return 'Execution process reset completed. Uncommitted changes were discarded.';
  }

  return 'Execution process reset completed.';
}

export interface UseExecutionProcessResetResult {
  isResetting: boolean;
  resetError: string | null;
  resetInfo: string | null;
  requiresForceReset: boolean;
  resetProcess: (processId: string | null | undefined) => Promise<void>;
  clearResetState: () => void;
}

export function useExecutionProcessReset(): UseExecutionProcessResetResult {
  const [isResetting, setIsResetting] = useState(false);
  const [resetError, setResetError] = useState<string | null>(null);
  const [resetInfo, setResetInfo] = useState<string | null>(null);
  const [requiresForceReset, setRequiresForceReset] = useState(false);

  const clearResetState = useCallback(() => {
    setResetError(null);
    setResetInfo(null);
    setRequiresForceReset(false);
  }, []);

  const resetProcess = useCallback(
    async (processId: string | null | undefined) => {
      if (!processId || isResetting) {
        return;
      }

      setIsResetting(true);
      setResetError(null);
      setResetInfo(null);

      try {
        const response = await resetExecutionProcess(processId, {
          perform_git_reset: true,
          force_when_dirty: requiresForceReset,
        });
        setResetInfo(buildResetSuccessMessage(response));
        setRequiresForceReset(false);
      } catch (error) {
        const message = error instanceof Error ? error.message : 'Failed to reset execution process';
        if (isForceResetRequiredError(message)) {
          setRequiresForceReset(true);
          setResetError(
            'Worktree has uncommitted changes. Click reset again to force a hard reset.'
          );
        } else {
          setResetError(message);
        }
      } finally {
        setIsResetting(false);
      }
    },
    [isResetting, requiresForceReset]
  );

  return {
    isResetting,
    resetError,
    resetInfo,
    requiresForceReset,
    resetProcess,
    clearResetState,
  };
}
