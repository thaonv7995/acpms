import { useCallback, useEffect, useState } from 'react';
import { sendAttemptInput } from '@/api/taskAttempts';
import { followUpExecutionProcess } from '@/api/executionProcesses';
import { useExecutionProcessesStream } from '@/hooks/useExecutionProcessesStream';
import { useDevServer } from '../../hooks/useDevServer';
import { PreviewPanel } from '../../components/preview/PreviewPanel';

/**
 * PreviewPanelWrapper - Connects PreviewPanel with dev server state
 */
interface PreviewPanelWrapperProps {
  taskId: string;
  attemptId: string;
  fallbackPreviewUrl?: string;
  autoStartOnMount?: boolean;
  attemptStatus?: string | null;
  taskStatus?: string | null;
  onFollowUpAttemptCreated?: (attemptId: string) => void;
}

const PREVIEW_RUNTIME_DISABLED_REASON =
  'preview unavailable: docker preview runtime is disabled';

const AGENT_PREVIEW_FOLLOW_UP_PROMPT = [
  'Deploy a preview for the latest code in this attempt.',
  'Because ACPMS managed Docker preview is disabled, run the preview yourself in this attempt environment.',
  'Verify the preview serves the latest code, then print `PREVIEW_TARGET: <url>` in the logs.',
  'Write `.acpms/preview-output.json` with `preview_url` or `preview_target`, and include `runtime_control` metadata if the runtime can be stopped by ACPMS.',
].join(' ');

export function PreviewPanelWrapper({
  taskId,
  attemptId,
  fallbackPreviewUrl,
  autoStartOnMount = false,
  attemptStatus = null,
  taskStatus = null,
  onFollowUpAttemptCreated,
}: PreviewPanelWrapperProps) {
  const {
    status,
    url,
    errorMessage,
    startServer,
    stopServer,
    dismissPreview,
    restartServer,
    startDisabled,
    startDisabledReason,
    externalPreview,
    previewRevision,
    canStopPreview,
    dismissOnly,
  } = useDevServer(taskId, attemptId, fallbackPreviewUrl, autoStartOnMount);
  const { processes } = useExecutionProcessesStream(attemptId);
  const [isRequestingAgentPreview, setIsRequestingAgentPreview] = useState(false);
  const [previewActionError, setPreviewActionError] = useState<string | undefined>();

  const normalizedAttemptStatus = attemptStatus?.toLowerCase() ?? null;
  const normalizedTaskStatus = taskStatus?.toLowerCase() ?? null;
  const isAttemptRunning =
    normalizedAttemptStatus === 'running' || normalizedAttemptStatus === 'queued';
  const isTaskDone = normalizedTaskStatus === 'done';
  const latestProcessId =
    processes.length > 0 ? processes[processes.length - 1].id : null;
  const canRequestAgentPreview =
    typeof startDisabledReason === 'string' &&
    startDisabledReason.toLowerCase().includes(PREVIEW_RUNTIME_DISABLED_REASON) &&
    !isTaskDone &&
    (isAttemptRunning || Boolean(latestProcessId));

  useEffect(() => {
    setIsRequestingAgentPreview(false);
    setPreviewActionError(undefined);
  }, [attemptId]);

  const handleStart = useCallback(async () => {
    if (!canRequestAgentPreview) {
      setPreviewActionError(undefined);
      await startServer();
      return;
    }

    setIsRequestingAgentPreview(true);
    setPreviewActionError(undefined);

    try {
      if (isAttemptRunning) {
        await sendAttemptInput(attemptId, AGENT_PREVIEW_FOLLOW_UP_PROMPT);
      } else {
        if (!latestProcessId) {
          throw new Error(
            'Execution process context is not ready yet. Please try again in a moment.'
          );
        }

        const followUpAttempt = await followUpExecutionProcess(
          latestProcessId,
          AGENT_PREVIEW_FOLLOW_UP_PROMPT
        );

        if (followUpAttempt.id && followUpAttempt.id !== attemptId) {
          onFollowUpAttemptCreated?.(followUpAttempt.id);
        }
      }
    } catch (error) {
      setPreviewActionError(
        error instanceof Error
          ? error.message
          : 'Failed to request preview deployment from the agent.'
      );
    } finally {
      setIsRequestingAgentPreview(false);
    }
  }, [
    canRequestAgentPreview,
    startServer,
    isAttemptRunning,
    attemptId,
    latestProcessId,
    onFollowUpAttemptCreated,
  ]);

  const effectiveStatus = previewActionError
    ? 'error'
    : isRequestingAgentPreview
      ? 'starting'
      : status;
  const effectiveErrorMessage = previewActionError || errorMessage;
  const previewStartWouldCreateAttemptReason =
    isTaskDone &&
    typeof startDisabledReason === 'string' &&
    startDisabledReason.toLowerCase().includes(PREVIEW_RUNTIME_DISABLED_REASON)
      ? 'Preview runtime is disabled here. Starting preview from a completed task would create a new follow-up attempt, so use the follow-up box if you want that explicitly.'
      : undefined;
  const effectiveStartDisabled = canRequestAgentPreview
    ? false
    : startDisabled || Boolean(previewStartWouldCreateAttemptReason);
  const effectiveStartDisabledReason =
    previewStartWouldCreateAttemptReason || startDisabledReason;
  const startActionTitle = canRequestAgentPreview
    ? 'Ask the agent to deploy a preview for this attempt'
    : undefined;
  const startActionLabel = canRequestAgentPreview
    ? 'Request agent preview'
    : undefined;

  return (
    <PreviewPanel
      devServerUrl={url}
      status={effectiveStatus}
      errorMessage={effectiveErrorMessage}
      externalPreview={externalPreview}
      previewRevision={previewRevision}
      onStart={handleStart}
      onStop={stopServer}
      onDismiss={dismissPreview}
      onRestart={restartServer}
      onRebuild={restartServer}
      startDisabled={effectiveStartDisabled}
      startDisabledReason={effectiveStartDisabledReason}
      startActionTitle={startActionTitle}
      startActionLabel={startActionLabel}
      canStopPreview={canStopPreview}
      dismissOnly={dismissOnly}
    />
  );
}
