import { useState, useCallback, useEffect } from 'react';
import { DevServerStatus } from '@/components/preview/DevServerControls';
import {
  createPreview,
  deletePreview,
  getPreview,
  getPreviewReadiness,
  getPreviewRuntimeStatus,
} from '@/api/previews';
import { getAttemptLogs, type AgentLog } from '@/api/taskAttempts';
import { ApiError } from '@/api/client';

interface DevServerState {
  status: DevServerStatus;
  url?: string;
  errorMessage?: string;
  previewId?: string;
  startDisabled: boolean;
  startDisabledReason?: string;
  externalPreview: boolean;
}

interface UseDevServerReturn {
  status: DevServerStatus;
  url?: string;
  errorMessage?: string;
  startServer: () => Promise<void>;
  stopServer: () => Promise<void>;
  restartServer: () => Promise<void>;
  isLoading: boolean;
  startDisabled: boolean;
  startDisabledReason?: string;
  externalPreview: boolean;
}

const PREVIEW_READINESS_BLOCKERS = [
  'preview unavailable: missing cloudflare config',
  'preview unavailable: docker preview runtime is disabled',
  'preview is disabled in project settings',
  'preview not supported for project type',
];

const PREVIEW_TARGET_REGEX = /\bpreview_target\b\s*[:=]\s*(https?:\/\/\S+)/i;
const PREVIEW_URL_REGEX = /\bpreview_url\b\s*[:=]\s*(https?:\/\/\S+)/i;
const ANSI_ESCAPE_REGEX = /\u001b\[[0-9;?]*[ -/]*[@-~]/g;

export function isPreviewReadinessBlockingMessage(message: string): boolean {
  const normalized = message.toLowerCase();
  return PREVIEW_READINESS_BLOCKERS.some((keyword) => normalized.includes(keyword));
}

export function isPreviewAlreadyStoppedMessage(message: string): boolean {
  const normalized = message.toLowerCase();
  return (
    normalized.includes('preview is already stopped') ||
    normalized.includes('preview not found')
  );
}

export function mapPreviewErrorMessage(message: string): string {
  const normalized = message.toLowerCase();

  if (isPreviewReadinessBlockingMessage(message)) {
    return message;
  }

  if (normalized.includes('attempt not found')) {
    return 'Attempt not found. Refresh the task detail page and try again.';
  }

  if (normalized.includes('permission') || normalized.includes('forbidden')) {
    return 'You do not have permission to manage preview for this project.';
  }

  if (isPreviewAlreadyStoppedMessage(message)) {
    return 'Preview is already stopped for this attempt.';
  }

  if (
    normalized.includes('tried scripts') &&
    normalized.includes('available scripts')
  ) {
    return 'No compatible start script found for preview. Add a dev/start script or set PREVIEW_DEV_COMMAND in server env.';
  }

  if (normalized.includes('no supported non-node entrypoint')) {
    return 'Cannot detect Python/Go/Rust preview entrypoint. Add manage.py/main.py/go.mod/Cargo.toml or set PREVIEW_DEV_COMMAND.';
  }

  if (normalized.includes('unable to resolve preview command')) {
    return 'Cannot detect a valid dev-server command. Add scripts like dev/start in package.json or set PREVIEW_DEV_COMMAND.';
  }

  if (
    normalized.includes('failed to read package.json') ||
    normalized.includes('failed to parse package.json')
  ) {
    return 'Cannot read package.json for preview command detection. Fix package.json syntax or set PREVIEW_DEV_COMMAND explicitly.';
  }

  if (
    normalized.includes('no module named uvicorn') ||
    normalized.includes('modulenotfounderror') ||
    normalized.includes('command not found') && normalized.includes('python')
  ) {
    return 'Python preview dependencies are missing in runtime image. Install required packages or override PREVIEW_DEV_IMAGE/PREVIEW_DEV_COMMAND.';
  }

  if (
    normalized.includes('command not found') &&
    (normalized.includes('go') || normalized.includes('cargo'))
  ) {
    return 'Runtime image does not include required toolchain (Go/Rust). Set PREVIEW_DEV_IMAGE to a compatible image and retry.';
  }

  if (
    normalized.includes('address already in use') ||
    normalized.includes('port is already allocated') ||
    normalized.includes('eaddrinuse')
  ) {
    return 'Preview port is already in use. Stop the conflicting process/container or change PREVIEW_DEV_PORT.';
  }

  if (
    normalized.includes('failed to execute docker compose up') ||
    normalized.includes('failed to execute docker compose down') ||
    normalized.includes('failed to execute docker compose ps')
  ) {
    return 'Docker compose command failed to execute. Verify Docker is installed/running and PREVIEW_DOCKER_COMMAND is valid.';
  }

  if (
    normalized.includes('cannot connect to the docker daemon') ||
    normalized.includes('is the docker daemon running') ||
    normalized.includes('docker daemon')
  ) {
    return 'Cannot connect to Docker daemon. Start Docker service and retry preview.';
  }

  if (
    normalized.includes('permission denied') &&
    (normalized.includes('docker') || normalized.includes('sock'))
  ) {
    return 'Docker permission denied. Ensure server user can access Docker socket or run with proper group permissions.';
  }

  if (normalized.includes('docker compose up failed')) {
    return 'Failed to start preview runtime containers. Check Docker daemon health and container logs.';
  }

  if (
    normalized.includes('timed out') &&
    normalized.includes('waiting for dev-server and cloudflared')
  ) {
    return 'Preview runtime startup timed out. Check app boot logs and ensure dev server binds to 0.0.0.0.';
  }

  if (
    normalized.includes('cloudflare') &&
    (normalized.includes('token') ||
      normalized.includes('dns') ||
      normalized.includes('tunnel') ||
      normalized.includes('client'))
  ) {
    return 'Cloudflare request failed. Verify Cloudflare account/zone/base-domain/token configuration and token permissions.';
  }

  if (normalized.includes('worktree path')) {
    return 'Worktree path for this attempt is unavailable. Re-run the attempt before starting preview.';
  }

  return message;
}

function parseApiError(error: unknown, fallback: string): string {
  if (error instanceof ApiError) {
    if (error.status === 401) {
      return 'Session expired. Please sign in again.';
    }
    if (error.status === 403) {
      return 'You do not have permission to manage preview for this project.';
    }
    return mapPreviewErrorMessage(error.message || fallback);
  }
  if (error instanceof Error) {
    return mapPreviewErrorMessage(error.message || fallback);
  }
  return mapPreviewErrorMessage(fallback);
}

function stripAnsiSequences(text: string): string {
  return text.replace(ANSI_ESCAPE_REGEX, '');
}

function normalizePreviewUrlCandidate(candidate: string): string | undefined {
  const normalized = candidate
    .trim()
    .replace(/^[<({\["'`]+/g, '')
    .replace(/[),.;\]}>\"'`]+$/g, '');
  if (!normalized) {
    return undefined;
  }
  if (/<[^>]*port\b|{[^}]*port\b/i.test(normalized)) {
    return undefined;
  }
  return normalized;
}

export function extractPreviewUrlFromText(text: string): string | undefined {
  const sanitized = stripAnsiSequences(text);

  for (const regex of [PREVIEW_TARGET_REGEX, PREVIEW_URL_REGEX]) {
    const match = sanitized.match(regex);
    const candidate = match?.[1];
    if (!candidate) {
      continue;
    }

    const normalized = normalizePreviewUrlCandidate(candidate);
    if (normalized) {
      return normalized;
    }
  }

  return undefined;
}

function extractPreviewUrlFromStructuredLog(value: unknown): string | undefined {
  if (typeof value === 'string') {
    return extractPreviewUrlFromText(value);
  }
  if (!value || typeof value !== 'object') {
    return undefined;
  }

  const record = value as Record<string, unknown>;
  for (const key of ['content', 'message', 'text']) {
    const field = record[key];
    if (typeof field !== 'string') {
      continue;
    }
    const extracted = extractPreviewUrlFromText(field);
    if (extracted) {
      return extracted;
    }
  }

  return undefined;
}

export function extractPreviewUrlFromAttemptLogs(logs: AgentLog[]): string | undefined {
  for (let index = logs.length - 1; index >= 0; index -= 1) {
    const content = logs[index]?.content;
    if (typeof content !== 'string' || content.length === 0) {
      continue;
    }

    const direct = extractPreviewUrlFromText(content);
    if (direct) {
      return direct;
    }

    try {
      const parsed = JSON.parse(content) as unknown;
      const extracted = extractPreviewUrlFromStructuredLog(parsed);
      if (extracted) {
        return extracted;
      }
    } catch {
      // Ignore plain-text logs that are not JSON.
    }
  }

  return undefined;
}

function buildExternalPreviewState(
  baseState: DevServerState,
  previewUrl: string,
  readinessReason?: string
): DevServerState {
  return {
    ...baseState,
    status: 'running',
    url: previewUrl,
    errorMessage: undefined,
    previewId: undefined,
    startDisabled: true,
    startDisabledReason:
      readinessReason || 'Preview is available via the agent-reported PREVIEW_TARGET.',
    externalPreview: true,
  };
}

/**
 * useDevServer - Hook for managing dev server state via backend preview APIs.
 *
 * @param taskId - Task ID for dev server
 * @param attemptId - Attempt ID for dev server
 *
 * @example
 * const { status, url, startServer, stopServer } = useDevServer(taskId, attemptId);
 */
export function useDevServer(
  taskId: string,
  attemptId?: string,
  fallbackPreviewUrl?: string
): UseDevServerReturn {
  const [state, setState] = useState<DevServerState>({
    status: 'idle',
    url: undefined,
    errorMessage: undefined,
    previewId: undefined,
    startDisabled: false,
    startDisabledReason: undefined,
    externalPreview: false,
  });
  const [isLoading, setIsLoading] = useState(false);

  // Hydrate preview state from backend when task/attempt changes.
  useEffect(() => {
    setState({
      status: 'idle',
      url: undefined,
      errorMessage: undefined,
      previewId: undefined,
      startDisabled: false,
      startDisabledReason: undefined,
      externalPreview: false,
    });
    if (!attemptId) {
      return;
    }

    let cancelled = false;
    setIsLoading(true);

    const logsPromise = fallbackPreviewUrl
      ? Promise.resolve([] as AgentLog[])
      : getAttemptLogs(attemptId).catch(() => [] as AgentLog[]);

    Promise.all([getPreviewReadiness(attemptId), getPreview(attemptId), logsPromise])
      .then(([readiness, preview, logs]) => {
        if (cancelled) return;

        const readinessBlocked = !readiness.ready;
        const readinessReason = readiness.reason || undefined;
        const agentPreviewUrl =
          fallbackPreviewUrl || extractPreviewUrlFromAttemptLogs(logs);

        setState((prev) => {
          const baseState: DevServerState = {
            ...prev,
            startDisabled: readinessBlocked,
            startDisabledReason: readinessReason,
            externalPreview: false,
          };

          if (!preview || prev.status === 'starting') {
            if (agentPreviewUrl) {
              return buildExternalPreviewState(
                baseState,
                agentPreviewUrl,
                readinessReason
              );
            }
            return baseState;
          }

          return {
            ...baseState,
            status: 'running',
            url: preview.preview_url,
            errorMessage: undefined,
            previewId: preview.id,
            externalPreview: false,
          };
        });
      })
      .catch((error) => {
        if (cancelled) return;
        setState((prev) => ({
          ...prev,
          errorMessage: parseApiError(error, 'Failed to load preview status'),
          externalPreview: false,
        }));
      })
      .finally(() => {
        if (!cancelled) {
          setIsLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [taskId, attemptId, fallbackPreviewUrl]);

  // Poll attempt logs while the agent is still running so PREVIEW_TARGET can
  // surface without requiring a manual page refresh.
  useEffect(() => {
    if (!attemptId || fallbackPreviewUrl || state.externalPreview || state.previewId) {
      return;
    }

    let cancelled = false;

    const syncPreviewTargetFromLogs = async () => {
      try {
        const logs = await getAttemptLogs(attemptId);
        if (cancelled) {
          return;
        }

        const previewUrl = extractPreviewUrlFromAttemptLogs(logs);
        if (!previewUrl) {
          return;
        }

        setState((prev) => {
          if (prev.externalPreview || prev.previewId) {
            return prev;
          }
          if (prev.status === 'starting' || prev.status === 'stopping') {
            return prev;
          }

          return buildExternalPreviewState(
            prev,
            previewUrl,
            prev.startDisabledReason
          );
        });
      } catch {
        // Ignore transient log fetch errors while the attempt is still running.
      }
    };

    void syncPreviewTargetFromLogs();
    const intervalId = window.setInterval(() => {
      void syncPreviewTargetFromLogs();
    }, 5000);

    return () => {
      cancelled = true;
      window.clearInterval(intervalId);
    };
  }, [
    attemptId,
    fallbackPreviewUrl,
    state.externalPreview,
    state.previewId,
    state.status,
  ]);

  // Poll runtime status while preview is running to detect crashed runtime.
  useEffect(() => {
    if (!attemptId || state.status !== 'running' || state.externalPreview) {
      return;
    }

    let cancelled = false;

    const checkRuntimeStatus = async () => {
      try {
        const runtimeStatus = await getPreviewRuntimeStatus(attemptId);
        if (cancelled) return;
        if (!runtimeStatus.runtime_ready) {
          setState((prev) => {
            if (prev.status !== 'running') return prev;
            return {
              ...prev,
              status: 'error',
              url: undefined,
              errorMessage: mapPreviewErrorMessage(
                runtimeStatus.last_error ||
                runtimeStatus.message ||
                'Preview runtime is not running. Start again to retry.',
              ),
            };
          });
        }
      } catch {
        // Ignore transient polling errors.
      }
    };

    void checkRuntimeStatus();
    const intervalId = window.setInterval(() => {
      void checkRuntimeStatus();
    }, 5000);

    return () => {
      cancelled = true;
      window.clearInterval(intervalId);
    };
  }, [attemptId, state.status, state.externalPreview]);

  const startServer = useCallback(async () => {
    if (state.status === 'running' || isLoading) return;
    if (state.startDisabled) {
      setState((prev) => ({
        ...prev,
        status: 'error',
        errorMessage:
          prev.startDisabledReason || 'Preview cannot be started due to readiness checks',
      }));
      return;
    }
    if (!attemptId) {
      setState({
        status: 'error',
        url: undefined,
        errorMessage: 'Attempt ID is required to start preview',
        previewId: undefined,
        startDisabled: true,
        startDisabledReason: 'Attempt ID is required to start preview',
        externalPreview: false,
      });
      return;
    }

    setIsLoading(true);
    setState((prev) => ({ ...prev, status: 'starting', errorMessage: undefined }));

    try {
      const preview = await createPreview(attemptId);
      setState({
        status: 'running',
        url: preview.preview_url,
        errorMessage: undefined,
        previewId: preview.id,
        startDisabled: false,
        startDisabledReason: undefined,
        externalPreview: false,
      });
    } catch (error) {
      const parsedMessage = parseApiError(error, 'Failed to start preview');
      const blockedByReadiness = isPreviewReadinessBlockingMessage(parsedMessage);

      setState({
        status: 'error',
        url: undefined,
        errorMessage: parsedMessage,
        previewId: undefined,
        startDisabled: blockedByReadiness,
        startDisabledReason: blockedByReadiness ? parsedMessage : undefined,
        externalPreview: false,
      });
    } finally {
      setIsLoading(false);
    }
  }, [state.status, isLoading, attemptId]);

  const stopServer = useCallback(async () => {
    if (isLoading) return;
    if (!attemptId) {
      setState((prev) => ({
        ...prev,
        status: 'error',
        errorMessage: 'Attempt ID is required to stop preview',
      }));
      return;
    }

    if (state.externalPreview) {
      setState((prev) => ({
        ...prev,
        status: 'running',
        errorMessage: 'This preview URL is managed by the agent, not by the local preview runtime.',
      }));
      return;
    }

    if (state.status !== 'running' && !state.previewId) return;

    setIsLoading(true);
    setState((prev) => ({ ...prev, status: 'stopping' }));

    try {
      await deletePreview(state.previewId || attemptId);

      setState({
        status: 'idle',
        url: undefined,
        errorMessage: undefined,
        previewId: undefined,
        startDisabled: state.startDisabled,
        startDisabledReason: state.startDisabledReason,
        externalPreview: false,
      });
    } catch (error) {
      const parsedMessage = parseApiError(error, 'Failed to stop preview');
      if (isPreviewAlreadyStoppedMessage(parsedMessage)) {
        setState((prev) => ({
          ...prev,
          status: 'idle',
          url: undefined,
          errorMessage: undefined,
          previewId: undefined,
          externalPreview: false,
        }));
        return;
      }
      setState((prev) => ({
        ...prev,
        status: 'error',
        errorMessage: parsedMessage,
        externalPreview: false,
      }));
    } finally {
      setIsLoading(false);
    }
  }, [state.status, state.previewId, state.externalPreview, isLoading, attemptId]);

  const restartServer = useCallback(async () => {
    if (state.status !== 'running' || isLoading || state.externalPreview) return;
    await stopServer();
    await startServer();
  }, [state.status, state.externalPreview, isLoading, stopServer, startServer]);

  return {
    status: state.status,
    url: state.url,
    errorMessage: state.errorMessage,
    startServer,
    stopServer,
    restartServer,
    isLoading,
    startDisabled: state.startDisabled,
    startDisabledReason: state.startDisabledReason,
    externalPreview: state.externalPreview,
  };
}
