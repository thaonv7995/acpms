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
  'Use the `preview-docker-runtime` skill.',
  'If a public/tunneled preview is needed, also use `setup-cloudflare-tunnel` after the local Docker preview is reachable.',
  'Run the preview in Docker only, not as a host process.',
  'Keep `PREVIEW_TARGET` as the local Docker preview URL and write the public Cloudflare tunnel URL to `PREVIEW_URL`.',
  'Verify the preview serves the latest code, then print `PREVIEW_TARGET: <local-url>` in the logs and `PREVIEW_URL: <public-url>` when a tunnel is available.',
  'Write `.acpms/preview-output.json` with `preview_target`, `preview_url`, and `runtime_control` metadata if the runtime can be stopped by ACPMS.',
].join(' ');

const AGENT_PREVIEW_STOP_PROMPT = [
  'Stop the preview that is currently associated with this attempt.',
  'Use the `preview-docker-runtime` skill.',
  'Use `deploy-cancel-stop-cleanup` if extra Docker cleanup is needed.',
  'Read `.acpms/preview-output.json` if present, stop the container or process it describes, and make sure the preview URL is no longer serving.',
  'Then update `.acpms/preview-output.json` to indicate the preview is stopped and clear or refresh any runtime_control metadata.',
].join(' ');

const AGENT_PREVIEW_RESTART_PROMPT = [
  'Restart the preview for the latest code in this attempt.',
  'Use the `preview-docker-runtime` skill.',
  'If a public/tunneled preview is needed, also use `setup-cloudflare-tunnel` after the local Docker preview is reachable.',
  'Stop the old Docker preview runtime if one is still running, start a fresh Docker preview, and keep `PREVIEW_TARGET` as the local Docker URL while `PREVIEW_URL` stores any public tunnel URL.',
  'Verify the preview serves the latest code, then print `PREVIEW_TARGET: <local-url>` in the logs and `PREVIEW_URL: <public-url>` when a tunnel is available.',
  'Write `.acpms/preview-output.json` with the new `preview_target`, `preview_url`, and `runtime_control` metadata if ACPMS can stop it.',
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
  const hasAgentActionContext = isAttemptRunning || Boolean(latestProcessId);
  const useAgentManagedPreviewActions = hasAgentActionContext;
  const hasPreviewUrl = Boolean(url);

  useEffect(() => {
    setIsRequestingAgentPreview(false);
    setPreviewActionError(undefined);
  }, [attemptId]);

  const requestAgentPreviewAction = useCallback(async (prompt: string) => {
    setIsRequestingAgentPreview(true);
    setPreviewActionError(undefined);

    try {
      if (isAttemptRunning) {
        await sendAttemptInput(attemptId, prompt);
      } else {
        if (!latestProcessId) {
          throw new Error(
            'Execution process context is not ready yet. Please try again in a moment.'
          );
        }

        const followUpAttempt = await followUpExecutionProcess(
          latestProcessId,
          prompt
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
    isAttemptRunning,
    attemptId,
    latestProcessId,
    onFollowUpAttemptCreated,
  ]);

  const handleStart = useCallback(async () => {
    if (useAgentManagedPreviewActions) {
      await requestAgentPreviewAction(AGENT_PREVIEW_FOLLOW_UP_PROMPT);
      return;
    }

    setPreviewActionError(undefined);
    await startServer();
  }, [requestAgentPreviewAction, startServer, useAgentManagedPreviewActions]);

  const handleStop = useCallback(async () => {
    if (useAgentManagedPreviewActions && hasPreviewUrl) {
      await requestAgentPreviewAction(AGENT_PREVIEW_STOP_PROMPT);
      return;
    }

    setPreviewActionError(undefined);
    await stopServer();
  }, [
    hasPreviewUrl,
    requestAgentPreviewAction,
    stopServer,
    useAgentManagedPreviewActions,
  ]);

  const handleRestart = useCallback(async () => {
    if (useAgentManagedPreviewActions) {
      await requestAgentPreviewAction(AGENT_PREVIEW_RESTART_PROMPT);
      return;
    }

    setPreviewActionError(undefined);
    await restartServer();
  }, [
    requestAgentPreviewAction,
    restartServer,
    useAgentManagedPreviewActions,
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
  const effectiveStartDisabled = useAgentManagedPreviewActions
    ? false
    : startDisabled || Boolean(previewStartWouldCreateAttemptReason);
  const effectiveStartDisabledReason =
    previewStartWouldCreateAttemptReason || startDisabledReason;
  const startActionTitle = useAgentManagedPreviewActions
    ? 'Ask the agent to deploy a preview for this attempt'
    : undefined;
  const startActionLabel = useAgentManagedPreviewActions
    ? 'Request agent preview'
    : undefined;
  const effectiveCanStopPreview =
    (useAgentManagedPreviewActions && hasPreviewUrl) || canStopPreview;
  const effectiveDismissOnly =
    hasPreviewUrl && !effectiveCanStopPreview ? true : dismissOnly;

  return (
    <PreviewPanel
      devServerUrl={url}
      status={effectiveStatus}
      errorMessage={effectiveErrorMessage}
      externalPreview={externalPreview}
      previewRevision={previewRevision}
      onStart={handleStart}
      onStop={handleStop}
      onDismiss={dismissPreview}
      onRestart={handleRestart}
      onRebuild={handleRestart}
      startDisabled={effectiveStartDisabled}
      startDisabledReason={effectiveStartDisabledReason}
      startActionTitle={startActionTitle}
      startActionLabel={startActionLabel}
      canStopPreview={effectiveCanStopPreview}
      dismissOnly={effectiveDismissOnly}
    />
  );
}
