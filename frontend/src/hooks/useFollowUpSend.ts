import { useState, useCallback } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { sendAttemptInput } from '@/api/taskAttempts';
import { followUpExecutionProcess } from '@/api/executionProcesses';

export interface UseFollowUpSendArgs {
  sessionId: string;
  isRunning?: boolean;
  message: string;
  retryProcessId?: string | null;
  onAfterSendCleanup: () => void;
  onFollowUpAttemptCreated?: (attemptId: string) => void;
}

export interface UseFollowUpSendReturn {
  isSendingFollowUp: boolean;
  followUpError: string | null;
  setFollowUpError: (error: string | null) => void;
  onSendFollowUp: () => Promise<void>;
}

export function useFollowUpSend({
  sessionId,
  isRunning = false,
  message,
  retryProcessId,
  onAfterSendCleanup,
  onFollowUpAttemptCreated,
}: UseFollowUpSendArgs): UseFollowUpSendReturn {
  const queryClient = useQueryClient();
  const [isSendingFollowUp, setIsSendingFollowUp] = useState(false);
  const [followUpError, setFollowUpError] = useState<string | null>(null);

  const onSendFollowUp = useCallback(async () => {
    if (!message.trim() || isSendingFollowUp) {
      return;
    }

    setIsSendingFollowUp(true);
    setFollowUpError(null);

    try {
      if (isRunning) {
        // Agent is running — send input to the current attempt
        await sendAttemptInput(sessionId, message.trim());
      } else {
        // Agent is not running — require process-scoped follow-up flow.
        if (!retryProcessId) {
          throw new Error('Execution process context is not ready yet. Please try again in a moment.');
        }
        const followUpAttempt = await followUpExecutionProcess(retryProcessId, message.trim());
        if (followUpAttempt.id && followUpAttempt.id !== sessionId) {
          onFollowUpAttemptCreated?.(followUpAttempt.id);
          void queryClient.invalidateQueries({ queryKey: ['execution-processes', followUpAttempt.id] });
        }
      }

      // Refetch attempt logs so new user message appears seamlessly (no full reload)
      void queryClient.invalidateQueries({ queryKey: ['attempt-logs-full', sessionId] });
      void queryClient.invalidateQueries({ queryKey: ['task-attempts'] });

      // Clean up on success
      onAfterSendCleanup();
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : 'Failed to send follow-up message';
      setFollowUpError(errorMessage);
    } finally {
      setIsSendingFollowUp(false);
    }
  }, [
    sessionId,
    isRunning,
    message,
    retryProcessId,
    isSendingFollowUp,
    onAfterSendCleanup,
    onFollowUpAttemptCreated,
    queryClient,
  ]);

  return {
    isSendingFollowUp,
    followUpError,
    setFollowUpError,
    onSendFollowUp,
  };
}
