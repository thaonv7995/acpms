import { useState, useCallback, useEffect } from 'react';
import { DevServerStatus } from '@/components/preview/DevServerControls';
import {
  createPreview,
  deletePreview,
  getPreview,
  getPreviewReadiness,
  getPreviewRuntimeStatus,
} from '@/api/previews';
import { ApiError } from '@/api/client';

interface DevServerState {
  status: DevServerStatus;
  url?: string;
  errorMessage?: string;
  previewId?: string;
  startDisabled: boolean;
  startDisabledReason?: string;
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
}

const PREVIEW_READINESS_BLOCKERS = [
  'preview unavailable: missing cloudflare config',
  'preview unavailable: docker preview runtime is disabled',
  'preview is disabled in project settings',
  'preview not supported for project type',
];

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

/**
 * useDevServer - Hook for managing dev server state via backend preview APIs.
 *
 * @param taskId - Task ID for dev server
 * @param attemptId - Attempt ID for dev server
 *
 * @example
 * const { status, url, startServer, stopServer } = useDevServer(taskId, attemptId);
 */
export function useDevServer(taskId: string, attemptId?: string): UseDevServerReturn {
  const [state, setState] = useState<DevServerState>({
    status: 'idle',
    url: undefined,
    errorMessage: undefined,
    previewId: undefined,
    startDisabled: false,
    startDisabledReason: undefined,
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
    });
    if (!attemptId) {
      return;
    }

    let cancelled = false;
    setIsLoading(true);

    Promise.all([getPreviewReadiness(attemptId), getPreview(attemptId)])
      .then(([readiness, preview]) => {
        if (cancelled) return;

        const readinessBlocked = !readiness.ready;
        const readinessReason = readiness.reason || undefined;

        setState((prev) => {
          const baseState: DevServerState = {
            ...prev,
            startDisabled: readinessBlocked,
            startDisabledReason: readinessReason,
          };

          if (!preview || prev.status === 'starting') {
            return baseState;
          }

          return {
            ...baseState,
            status: 'running',
            url: preview.preview_url,
            errorMessage: undefined,
            previewId: preview.id,
          };
        });
      })
      .catch((error) => {
        if (cancelled) return;
        setState((prev) => ({
          ...prev,
          errorMessage: parseApiError(error, 'Failed to load preview status'),
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
  }, [taskId, attemptId]);

  // Poll runtime status while preview is running to detect crashed runtime.
  useEffect(() => {
    if (!attemptId || state.status !== 'running') {
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
  }, [attemptId, state.status]);

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
        }));
        return;
      }
      setState((prev) => ({
        ...prev,
        status: 'error',
        errorMessage: parsedMessage,
      }));
    } finally {
      setIsLoading(false);
    }
  }, [state.status, state.previewId, isLoading, attemptId]);

  const restartServer = useCallback(async () => {
    if (state.status !== 'running' || isLoading) return;
    await stopServer();
    await startServer();
  }, [state.status, isLoading, stopServer, startServer]);

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
  };
}
