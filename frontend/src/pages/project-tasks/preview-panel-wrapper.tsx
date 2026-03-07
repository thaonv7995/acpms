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

function isLocalPreviewUrl(candidate?: string): boolean {
  if (!candidate) {
    return false;
  }

  const normalized = candidate.trim().toLowerCase();
  return (
    normalized.startsWith('http://localhost:') ||
    normalized.startsWith('https://localhost:') ||
    normalized.startsWith('http://127.0.0.1:') ||
    normalized.startsWith('https://127.0.0.1:') ||
    normalized.startsWith('http://0.0.0.0:') ||
    normalized.startsWith('https://0.0.0.0:') ||
    normalized.startsWith('http://[::1]:') ||
    normalized.startsWith('https://[::1]:')
  );
}

function formatMissingCloudflareField(field: string): string {
  switch (field) {
    case 'cloudflare_account_id':
      return 'Account ID';
    case 'cloudflare_api_token':
      return 'API Token';
    case 'cloudflare_zone_id':
      return 'Zone ID';
    case 'cloudflare_base_domain':
      return 'Base Domain';
    default:
      return field.replace(/_/g, ' ');
  }
}

const PREVIEW_RUNTIME_DISABLED_REASON =
  'preview unavailable: docker preview runtime is disabled';

const AGENT_PREVIEW_FOLLOW_UP_PROMPT = [
  'Deploy a preview for the latest code in this attempt.',
  'Use the `preview-docker-runtime` skill.',
  'If a public/tunneled preview is needed, also use `setup-cloudflare-tunnel` after the local Docker preview is reachable.',
  'Run the preview in Docker only, not as a host process.',
  'Keep `PREVIEW_TARGET` as the local Docker preview URL. If a public Cloudflare tunnel URL is available, write it to `PREVIEW_URL`; otherwise set `PREVIEW_URL` to the same local URL as `PREVIEW_TARGET`.',
  'Verify the preview serves the latest code, then print `PREVIEW_TARGET: <local-url>` in the logs and also print `PREVIEW_URL: <url>` where the URL is public when available, or the same local URL when no public URL exists.',
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
  'Stop the old Docker preview runtime if one is still running, start a fresh Docker preview, and keep `PREVIEW_TARGET` as the local Docker URL. If a public tunnel URL exists, write it to `PREVIEW_URL`; otherwise keep `PREVIEW_URL` equal to the same local URL.',
  'Verify the preview serves the latest code, then print `PREVIEW_TARGET: <local-url>` in the logs and also print `PREVIEW_URL: <url>` where the URL is public when available, or the same local URL when no public URL exists.',
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
    cloudflareReady,
    missingCloudflareFields,
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
    setPreviewActionError(undefined);
    const started = await startServer();
    if (!started && hasAgentActionContext) {
      await requestAgentPreviewAction(AGENT_PREVIEW_FOLLOW_UP_PROMPT);
    }
  }, [hasAgentActionContext, requestAgentPreviewAction, startServer]);

  const handleStop = useCallback(async () => {
    setPreviewActionError(undefined);
    const stopped = await stopServer();
    if (!stopped && hasAgentActionContext && hasPreviewUrl) {
      await requestAgentPreviewAction(AGENT_PREVIEW_STOP_PROMPT);
    }
  }, [
    hasAgentActionContext,
    hasPreviewUrl,
    requestAgentPreviewAction,
    stopServer,
  ]);

  const handleRestart = useCallback(async () => {
    setPreviewActionError(undefined);
    const restarted = await restartServer();
    if (!restarted && hasAgentActionContext) {
      await requestAgentPreviewAction(AGENT_PREVIEW_RESTART_PROMPT);
    }
  }, [
    hasAgentActionContext,
    requestAgentPreviewAction,
    restartServer,
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
      ? 'Preview runtime is disabled here. ACPMS will try the built-in preview flow first, then fall back to an agent follow-up if needed.'
      : undefined;
  const effectiveStartDisabled =
    !hasAgentActionContext &&
    (startDisabled || Boolean(previewStartWouldCreateAttemptReason));
  const effectiveStartDisabledReason =
    hasAgentActionContext
      ? startDisabledReason
      : previewStartWouldCreateAttemptReason || startDisabledReason;
  const startActionTitle = hasAgentActionContext
    ? 'Try ACPMS preview first, then fall back to an agent follow-up if needed'
    : undefined;
  const effectiveCanStopPreview =
    (hasAgentActionContext && hasPreviewUrl) || canStopPreview;
  const effectiveDismissOnly =
    hasPreviewUrl && !effectiveCanStopPreview ? true : dismissOnly;
  const showPublicUrlUnavailableBadge =
    Boolean(url) &&
    isLocalPreviewUrl(url) &&
    !cloudflareReady &&
    missingCloudflareFields.length > 0;
  const publicUrlUnavailableTitle = showPublicUrlUnavailableBadge
    ? `Missing Cloudflare settings: ${missingCloudflareFields
        .map(formatMissingCloudflareField)
        .join(', ')}.`
    : undefined;

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
      canStopPreview={effectiveCanStopPreview}
      dismissOnly={effectiveDismissOnly}
      statusBadgeLabel={
        showPublicUrlUnavailableBadge ? 'Public URL unavailable' : undefined
      }
      statusBadgeTitle={publicUrlUnavailableTitle}
    />
  );
}
