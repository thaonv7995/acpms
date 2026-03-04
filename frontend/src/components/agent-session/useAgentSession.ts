/**
 * useAgentSession - Hook for managing agent session state and WebSocket
 * Combines REST API for initial data with WebSocket for real-time updates
 */

import { useState, useEffect, useCallback, useRef } from 'react';
import type { AgentSessionState } from './types';
import {
  fetchAttemptLogs,
  fetchAttemptStatus,
  sendAttemptInput,
  mapStatusToState,
  transformLog,
} from './utils';
import { getAccessToken, getWsBaseUrl } from '@/api/client';
import { logger } from '@/lib/logger';
const WS_AUTH_PROTOCOL = 'acpms-bearer';

interface UseAgentSessionOptions {
  attemptId: string | undefined;
  enabled?: boolean;
  onStatusChange?: (status: AgentSessionState['status']) => void;
}

interface UseAgentSessionResult {
  state: AgentSessionState;
  isConnected: boolean;
  isLoading: boolean;
  error: string | null;
  sendMessage: (message: string) => Promise<void>;
  refresh: () => void;
  clearLogs: () => void;
}

export function useAgentSession({
  attemptId,
  enabled = true,
  onStatusChange,
}: UseAgentSessionOptions): UseAgentSessionResult {
  const [state, setState] = useState<AgentSessionState>({
    status: 'idle',
    logs: [],
  });
  const [isConnected, setIsConnected] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout>>();
  const reconnectAttemptsRef = useRef(0);

  // Fetch initial logs
  const loadLogs = useCallback(async () => {
    if (!attemptId) return;
    setIsLoading(true);
    setError(null);

    try {
      const logs = await fetchAttemptLogs(attemptId);
      setState((prev) => ({ ...prev, attemptId, logs }));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch logs');
    } finally {
      setIsLoading(false);
    }
  }, [attemptId]);

  // Fetch attempt status
  const loadStatus = useCallback(async () => {
    if (!attemptId) return;

    try {
      const data = await fetchAttemptStatus(attemptId);
      const newStatus = mapStatusToState(data.status);

      setState((prev) => {
        if (prev.status !== newStatus) {
          onStatusChange?.(newStatus);
        }
        return { ...prev, status: newStatus, branch: data.branch };
      });
    } catch (err) {
      logger.error('Failed to fetch status:', err);
    }
  }, [attemptId, onStatusChange]);

  // Handle WebSocket messages
  const handleWsMessage = useCallback(
    (event: MessageEvent) => {
      try {
        const data = JSON.parse(event.data);

        if (data.type === 'Log') {
          const newLog = transformLog({
            id: data.id || crypto.randomUUID(),
            attempt_id: data.attempt_id || attemptId || '',
            log_type: data.log_type,
            content: data.content,
            created_at: data.timestamp || new Date().toISOString(),
            metadata: data.metadata,
          });
          setState((prev) => ({ ...prev, logs: [...prev.logs, newLog] }));
        } else if (data.type === 'Status') {
          const newStatus = mapStatusToState(data.status);
          setState((prev) => {
            if (prev.status !== newStatus) onStatusChange?.(newStatus);
            return { ...prev, status: newStatus };
          });
        } else if (data.type === 'DiffSummary') {
          setState((prev) => ({
            ...prev,
            diffSummary: {
              filesChanged: data.total_files || 0,
              additions: data.total_additions || 0,
              deletions: data.total_deletions || 0,
              filesAdded: data.files_added,
              filesModified: data.files_modified,
              filesDeleted: data.files_deleted,
            },
          }));
        }
      } catch (err) {
        logger.error('Failed to parse WS message:', err);
      }
    },
    [attemptId, onStatusChange]
  );

  // Connect WebSocket
  const connectWs = useCallback(() => {
    if (!attemptId || !enabled) return;

    wsRef.current?.close();

    const token = getAccessToken();
    const wsUrl = `${getWsBaseUrl()}/api/v1/attempts/${attemptId}/logs/ws`;
    const ws = token
      ? new WebSocket(wsUrl, [WS_AUTH_PROTOCOL, token])
      : new WebSocket(wsUrl);
    wsRef.current = ws;

    ws.onopen = () => {
      setIsConnected(true);
      reconnectAttemptsRef.current = 0;
    };

    ws.onmessage = handleWsMessage;
    ws.onerror = () => setError('WebSocket error');

    ws.onclose = () => {
      setIsConnected(false);
      wsRef.current = null;

      if (enabled && reconnectAttemptsRef.current < 5) {
        reconnectAttemptsRef.current += 1;
        const delay = Math.min(1000 * Math.pow(2, reconnectAttemptsRef.current), 30000);
        reconnectTimeoutRef.current = setTimeout(connectWs, delay);
      }
    };
  }, [attemptId, enabled, handleWsMessage]);

  // Send message
  const sendMessage = useCallback(
    async (message: string) => {
      if (!attemptId) return;

      try {
        await sendAttemptInput(attemptId, message);
        setState((prev) => ({
          ...prev,
          logs: [
            ...prev.logs,
            {
              id: crypto.randomUUID(),
              type: 'user_input',
              content: message,
              timestamp: new Date().toISOString(),
            },
          ],
        }));
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to send');
        throw err;
      }
    },
    [attemptId]
  );

  const refresh = useCallback(() => {
    loadLogs();
    loadStatus();
  }, [loadLogs, loadStatus]);

  const clearLogs = useCallback(() => {
    setState((prev) => ({ ...prev, logs: [] }));
  }, []);

  // Initialize - only when attemptId or enabled changes
  useEffect(() => {
    if (!attemptId || !enabled) return;

    loadLogs();
    loadStatus();
    connectWs();

    return () => {
      clearTimeout(reconnectTimeoutRef.current);
      wsRef.current?.close();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [attemptId, enabled]);

  return { state, isConnected, isLoading, error, sendMessage, refresh, clearLogs };
}
