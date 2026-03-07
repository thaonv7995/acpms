import { useState, useCallback, useEffect } from 'react';
import { DevServerStatus } from '@/components/preview/DevServerControls';
import {
  createPreview,
  getPreview,
  getPreviewControl,
  getPreviewReadiness,
  getPreviewRuntimeStatus,
  stopPreviewForAttempt,
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
  previewRevision: number;
  lastExternalPreviewSignal?: string;
  dismissedExternalPreviewSignal?: string;
  canStopPreview: boolean;
  dismissOnly: boolean;
}

interface UseDevServerReturn {
  status: DevServerStatus;
  url?: string;
  errorMessage?: string;
  startServer: () => Promise<boolean>;
  stopServer: () => Promise<boolean>;
  dismissPreview: () => void;
  restartServer: () => Promise<boolean>;
  isLoading: boolean;
  startDisabled: boolean;
  startDisabledReason?: string;
  externalPreview: boolean;
  previewRevision: number;
  canStopPreview: boolean;
  dismissOnly: boolean;
  cloudflareReady: boolean;
  missingCloudflareFields: string[];
}

interface PreviewLogSignal {
  url: string;
  signalKey: string;
}

function getDismissedExternalPreviewStorageKey(attemptId: string): string {
  return `acpms:preview:dismissed-external-signal:${attemptId}`;
}

function readDismissedExternalPreviewSignal(
  attemptId?: string
): string | undefined {
  if (!attemptId || typeof window === 'undefined') {
    return undefined;
  }
  try {
    return window.localStorage.getItem(
      getDismissedExternalPreviewStorageKey(attemptId)
    ) || undefined;
  } catch {
    return undefined;
  }
}

function writeDismissedExternalPreviewSignal(
  attemptId: string,
  signalKey?: string
): void {
  if (typeof window === 'undefined') {
    return;
  }
  try {
    const storageKey = getDismissedExternalPreviewStorageKey(attemptId);
    if (signalKey && signalKey.length > 0) {
      window.localStorage.setItem(storageKey, signalKey);
    } else {
      window.localStorage.removeItem(storageKey);
    }
  } catch {
    // Ignore storage access failures.
  }
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
  const trimmed = candidate.trim().replace(/^[<({\["'`]+/g, '');
  const firstBoundary = trimmed.search(/[\s`"'*|]/);
  const slice = firstBoundary >= 0 ? trimmed.slice(0, firstBoundary) : trimmed;
  const normalized = slice.replace(/[),.;\]}>\"'`]+$/g, '');
  if (!normalized) {
    return undefined;
  }
  if (/<[^>]*port\b|{[^}]*port\b/i.test(normalized)) {
    return undefined;
  }
  return normalized;
}

function isLoopbackPreviewUrl(candidate?: string): boolean {
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

function shouldPreferExternalPreviewSignal(
  currentUrl: string | undefined,
  nextUrl: string
): boolean {
  if (!currentUrl) {
    return true;
  }

  if (currentUrl === nextUrl) {
    return true;
  }

  return isLoopbackPreviewUrl(currentUrl) && !isLoopbackPreviewUrl(nextUrl);
}

export function extractPreviewUrlFromText(text: string): string | undefined {
  const sanitized = stripAnsiSequences(text);

  for (const regex of [PREVIEW_URL_REGEX, PREVIEW_TARGET_REGEX]) {
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
  return extractPreviewSignalFromAttemptLogs(logs)?.url;
}

export function extractPreviewSignalFromAttemptLogs(
  logs: AgentLog[]
): PreviewLogSignal | undefined {
  for (let index = logs.length - 1; index >= 0; index -= 1) {
    const log = logs[index];
    const content = log?.content;
    if (typeof content !== 'string' || content.length === 0) {
      continue;
    }

    const direct = extractPreviewUrlFromText(content);
    if (direct) {
      return {
        url: direct,
        signalKey: `${log.id}:${log.created_at}:${direct}`,
      };
    }

    try {
      const parsed = JSON.parse(content) as unknown;
      const extracted = extractPreviewUrlFromStructuredLog(parsed);
      if (extracted) {
        return {
          url: extracted,
          signalKey: `${log.id}:${log.created_at}:${extracted}`,
        };
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
  readinessReason?: string,
  signalKey?: string
): DevServerState {
  const shouldBumpRevision =
    typeof signalKey === 'string' &&
    signalKey.length > 0 &&
    signalKey !== baseState.lastExternalPreviewSignal;

  return {
    ...baseState,
    status: 'running',
    url: previewUrl,
    errorMessage: undefined,
    previewId: undefined,
    startDisabled: true,
    startDisabledReason:
      readinessReason || 'Preview is available via the agent-reported preview URL.',
    externalPreview: true,
    previewRevision: shouldBumpRevision
      ? baseState.previewRevision + 1
      : baseState.previewRevision,
    lastExternalPreviewSignal: signalKey || baseState.lastExternalPreviewSignal,
    dismissedExternalPreviewSignal: baseState.dismissedExternalPreviewSignal,
    canStopPreview: baseState.canStopPreview,
    dismissOnly: baseState.dismissOnly,
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
  fallbackPreviewUrl?: string,
  autoStartOnMount = false
): UseDevServerReturn {
  const initialDismissedSignal = readDismissedExternalPreviewSignal(attemptId);
  const [state, setState] = useState<DevServerState>({
    status: 'idle',
    url: undefined,
    errorMessage: undefined,
    previewId: undefined,
    startDisabled: false,
    startDisabledReason: undefined,
    externalPreview: false,
    previewRevision: 0,
    lastExternalPreviewSignal: undefined,
    dismissedExternalPreviewSignal: initialDismissedSignal,
    canStopPreview: false,
    dismissOnly: false,
  });
  const [isLoading, setIsLoading] = useState(false);
  const [hasHydrated, setHasHydrated] = useState(false);
  const [cloudflareReady, setCloudflareReady] = useState(true);
  const [missingCloudflareFields, setMissingCloudflareFields] = useState<string[]>([]);

  // Hydrate preview state from backend when task/attempt changes.
  useEffect(() => {
    setHasHydrated(false);
    setCloudflareReady(true);
    setMissingCloudflareFields([]);
    const dismissedExternalPreviewSignal =
      readDismissedExternalPreviewSignal(attemptId);
    setState({
      status: 'idle',
      url: undefined,
      errorMessage: undefined,
      previewId: undefined,
      startDisabled: false,
      startDisabledReason: undefined,
      externalPreview: false,
      previewRevision: 0,
      lastExternalPreviewSignal: undefined,
      dismissedExternalPreviewSignal,
      canStopPreview: false,
      dismissOnly: false,
    });
    if (!attemptId) {
      return;
    }

    let cancelled = false;
    setIsLoading(true);

    const logsPromise = fallbackPreviewUrl
      ? Promise.resolve([] as AgentLog[])
      : getAttemptLogs(attemptId).catch(() => [] as AgentLog[]);

    Promise.all([
      getPreviewReadiness(attemptId),
      getPreview(attemptId),
      getPreviewControl(attemptId),
      logsPromise,
    ])
      .then(([readiness, preview, control, logs]) => {
        if (cancelled) return;

        const readinessBlocked = !readiness.ready;
        const readinessReason = readiness.reason || undefined;
        setCloudflareReady(readiness.cloudflare_ready);
        setMissingCloudflareFields(readiness.missing_cloudflare_fields || []);
        const agentPreviewSignal =
          typeof fallbackPreviewUrl === 'string' && fallbackPreviewUrl.trim().length > 0
            ? {
                url: fallbackPreviewUrl,
                signalKey: `fallback:${fallbackPreviewUrl}`,
              }
            : extractPreviewSignalFromAttemptLogs(logs);

        setState((prev) => {
          const baseState: DevServerState = {
            ...prev,
            startDisabled: readinessBlocked,
            startDisabledReason: readinessReason,
            externalPreview: false,
            canStopPreview: control.controllable,
            dismissOnly: control.action === 'dismiss',
          };

          if (prev.status === 'starting') {
            return baseState;
          }

          if (
            agentPreviewSignal &&
            agentPreviewSignal.signalKey !== prev.dismissedExternalPreviewSignal &&
            (!preview ||
              (preview.preview_url !== agentPreviewSignal.url &&
                shouldPreferExternalPreviewSignal(
                  preview.preview_url,
                  agentPreviewSignal.url
                )))
          ) {
            return buildExternalPreviewState(
              baseState,
              agentPreviewSignal.url,
              readinessReason,
              agentPreviewSignal.signalKey
            );
          }

          if (!preview) {
            if (!control.preview_available) {
              return {
                ...baseState,
                status: 'idle',
                url: undefined,
                errorMessage: undefined,
                previewId: undefined,
                previewRevision: 0,
                lastExternalPreviewSignal: undefined,
              };
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
            previewRevision: 0,
            lastExternalPreviewSignal: undefined,
            dismissedExternalPreviewSignal:
              prev.dismissedExternalPreviewSignal,
            canStopPreview: true,
            dismissOnly: false,
          };
        });
      })
      .catch((error) => {
        if (cancelled) return;
        setCloudflareReady(true);
        setMissingCloudflareFields([]);
        setState((prev) => ({
          ...prev,
          errorMessage: parseApiError(error, 'Failed to load preview status'),
          externalPreview: false,
          dismissedExternalPreviewSignal:
            prev.dismissedExternalPreviewSignal,
          canStopPreview: false,
          dismissOnly: false,
        }));
      })
      .finally(() => {
        if (!cancelled) {
          setIsLoading(false);
          setHasHydrated(true);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [taskId, attemptId, fallbackPreviewUrl]);

  // Poll attempt logs while the agent is still running so PREVIEW_TARGET can
  // surface without requiring a manual page refresh.
  useEffect(() => {
    if (!attemptId) {
      return;
    }

    let cancelled = false;

    const syncPreviewTargetFromLogs = async () => {
      try {
        const logs = await getAttemptLogs(attemptId);
        if (cancelled) {
          return;
        }

        const previewSignal = extractPreviewSignalFromAttemptLogs(logs);
        if (!previewSignal) {
          return;
        }

        setState((prev) => {
          if (prev.status === 'starting' || prev.status === 'stopping') {
            return prev;
          }
          if (
            previewSignal.signalKey ===
            prev.dismissedExternalPreviewSignal
          ) {
            return prev;
          }
          if (prev.previewId && previewSignal.url === prev.url) {
            return prev;
          }

          const shouldAdoptPreviewSignal =
            !prev.previewId ||
            prev.externalPreview ||
            shouldPreferExternalPreviewSignal(prev.url, previewSignal.url);

          if (!shouldAdoptPreviewSignal) {
            return prev;
          }

          return buildExternalPreviewState(
            {
              ...prev,
              canStopPreview: prev.canStopPreview,
              dismissOnly: prev.dismissOnly || !prev.canStopPreview,
            },
            previewSignal.url,
            prev.startDisabledReason,
            previewSignal.signalKey
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
    state.previewId,
    state.status,
  ]);

  // Poll preview info while a managed preview is running so the panel updates
  // when the backend promotes a local URL to a public tunnel URL.
  useEffect(() => {
    if (!attemptId || state.status !== 'running' || state.externalPreview) {
      return;
    }

    let cancelled = false;

    const syncPreviewInfo = async () => {
      try {
        const preview = await getPreview(attemptId);
        if (cancelled || !preview) {
          return;
        }

        setState((prev) => {
          if (prev.status === 'starting' || prev.status === 'stopping') {
            return prev;
          }

          const urlChanged = preview.preview_url !== prev.url;
          const previewChanged = preview.id !== prev.previewId;
          if (!urlChanged && !previewChanged) {
            return prev;
          }

          return {
            ...prev,
            status: 'running',
            url: preview.preview_url,
            errorMessage: undefined,
            previewId: preview.id,
            externalPreview: false,
            previewRevision: urlChanged
              ? prev.previewRevision + 1
              : prev.previewRevision,
            lastExternalPreviewSignal: undefined,
            canStopPreview: true,
            dismissOnly: false,
          };
        });
      } catch {
        // Ignore transient preview-info polling errors.
      }
    };

    void syncPreviewInfo();
    const intervalId = window.setInterval(() => {
      void syncPreviewInfo();
    }, 5000);

    return () => {
      cancelled = true;
      window.clearInterval(intervalId);
    };
  }, [attemptId, state.status, state.externalPreview]);

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
              previewRevision: 0,
              lastExternalPreviewSignal: undefined,
              canStopPreview: false,
              dismissOnly: false,
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
    if (state.status === 'running' || isLoading) return false;
    if (state.startDisabled) {
      setState((prev) => ({
        ...prev,
        status: 'error',
        errorMessage:
          prev.startDisabledReason || 'Preview cannot be started due to readiness checks',
      }));
      return false;
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
        previewRevision: 0,
        lastExternalPreviewSignal: undefined,
        dismissedExternalPreviewSignal:
          state.dismissedExternalPreviewSignal,
        canStopPreview: false,
        dismissOnly: false,
      });
      return false;
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
        previewRevision: 0,
        lastExternalPreviewSignal: undefined,
        dismissedExternalPreviewSignal: undefined,
        canStopPreview: true,
        dismissOnly: false,
      });
      writeDismissedExternalPreviewSignal(attemptId, undefined);
      return true;
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
        previewRevision: 0,
        lastExternalPreviewSignal: undefined,
        dismissedExternalPreviewSignal: state.dismissedExternalPreviewSignal,
        canStopPreview: false,
        dismissOnly: false,
      });
      return false;
    } finally {
      setIsLoading(false);
    }
  }, [state.status, state.startDisabled, state.startDisabledReason, isLoading, attemptId]);

  const stopServer = useCallback(async () => {
    if (isLoading) return false;
    if (!attemptId) {
      setState((prev) => ({
        ...prev,
        status: 'error',
        errorMessage: 'Attempt ID is required to stop preview',
      }));
      return false;
    }

    if (state.status !== 'running' && !state.previewId) return true;

    setIsLoading(true);
    setState((prev) => ({ ...prev, status: 'stopping' }));

    try {
      await stopPreviewForAttempt(attemptId);

      const dismissedSignal = state.externalPreview
        ? state.lastExternalPreviewSignal
        : undefined;
      writeDismissedExternalPreviewSignal(attemptId, dismissedSignal);

      setState({
        status: 'idle',
        url: undefined,
        errorMessage: undefined,
        previewId: undefined,
        startDisabled: state.startDisabled,
        startDisabledReason: state.startDisabledReason,
        externalPreview: false,
        previewRevision: 0,
        lastExternalPreviewSignal: undefined,
        dismissedExternalPreviewSignal: dismissedSignal,
        canStopPreview: false,
        dismissOnly: false,
      });
      return true;
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
          previewRevision: 0,
          lastExternalPreviewSignal: undefined,
          dismissedExternalPreviewSignal:
            state.externalPreview
              ? state.lastExternalPreviewSignal
              : prev.dismissedExternalPreviewSignal,
          canStopPreview: false,
          dismissOnly: false,
        }));
        if (state.externalPreview) {
          writeDismissedExternalPreviewSignal(
            attemptId,
            state.lastExternalPreviewSignal
          );
        }
        return true;
      }
      setState((prev) => ({
        ...prev,
        status: 'error',
        errorMessage: parsedMessage,
        externalPreview: false,
        previewRevision: 0,
        lastExternalPreviewSignal: undefined,
        dismissedExternalPreviewSignal:
          prev.dismissedExternalPreviewSignal,
        canStopPreview: false,
        dismissOnly: false,
      }));
      return false;
    } finally {
      setIsLoading(false);
    }
  }, [state.status, state.previewId, state.externalPreview, isLoading, attemptId]);

  const restartServer = useCallback(async () => {
    if (state.status !== 'running' || isLoading || state.externalPreview) {
      return false;
    }
    const stopped = await stopServer();
    if (!stopped) {
      return false;
    }
    return startServer();
  }, [state.status, state.externalPreview, isLoading, stopServer, startServer]);

  const dismissPreview = useCallback(() => {
    if (!attemptId) {
      return;
    }
    const dismissedSignal =
      state.lastExternalPreviewSignal || (state.url ? `fallback:${state.url}` : undefined);
    writeDismissedExternalPreviewSignal(attemptId, dismissedSignal);
    setState((prev) => ({
      ...prev,
      status: 'idle',
      url: undefined,
      errorMessage: undefined,
      previewId: undefined,
      externalPreview: false,
      previewRevision: 0,
      lastExternalPreviewSignal: undefined,
      dismissedExternalPreviewSignal: dismissedSignal,
      canStopPreview: false,
      dismissOnly: false,
    }));
  }, [attemptId, state.lastExternalPreviewSignal, state.url]);

  useEffect(() => {
    if (
      !autoStartOnMount ||
      !attemptId ||
      !hasHydrated ||
      isLoading ||
      state.startDisabled ||
      state.previewId ||
      state.externalPreview ||
      state.url ||
      state.status !== 'idle'
    ) {
      return;
    }

    void startServer();
  }, [
    autoStartOnMount,
    attemptId,
    hasHydrated,
    isLoading,
    startServer,
    state.startDisabled,
    state.previewId,
    state.externalPreview,
    state.url,
    state.status,
  ]);

  return {
    status: state.status,
    url: state.url,
    errorMessage: state.errorMessage,
    startServer,
    stopServer,
    dismissPreview,
    restartServer,
    isLoading,
    startDisabled: state.startDisabled,
    startDisabledReason: state.startDisabledReason,
    externalPreview: state.externalPreview,
    previewRevision: state.previewRevision,
    canStopPreview: state.canStopPreview,
    dismissOnly: state.dismissOnly,
    cloudflareReady,
    missingCloudflareFields,
  };
}
