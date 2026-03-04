// WebSocket hook with JSON Patch support for real-time diff streaming
import { useState, useEffect, useCallback, useRef } from 'react';
import { applyPatch, Operation } from 'fast-json-patch';
import { getWsBaseUrl } from '@/api/client';
import { logger } from '@/lib/logger';

interface WebSocketStreamOptions<T> {
  url: string;
  enabled?: boolean;
  initialData?: T;
  onMessage?: (data: T) => void;
  onError?: (error: Event) => void;
  onOpen?: () => void;
  onClose?: () => void;
  reconnectAttempts?: number;
  reconnectInterval?: number;
}

interface UseWebSocketStreamResult<T> {
  data: T | null;
  isConnected: boolean;
  isConnecting: boolean;
  error: Error | null;
  send: (message: unknown) => void;
  reconnect: () => void;
  disconnect: () => void;
}

export function useJsonPatchWsStream<T>({
  url,
  enabled = true,
  initialData,
  onMessage,
  onError,
  onOpen,
  onClose,
  reconnectAttempts = 3,
  reconnectInterval = 2000,
}: WebSocketStreamOptions<T>): UseWebSocketStreamResult<T> {
  const [data, setData] = useState<T | null>(initialData ?? null);
  const [isConnected, setIsConnected] = useState(false);
  const [isConnecting, setIsConnecting] = useState(false);
  const [error, setError] = useState<Error | null>(null);

  const wsRef = useRef<WebSocket | null>(null);
  const reconnectCountRef = useRef(0);
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout>>();

  const connect = useCallback(() => {
    if (!enabled || !url) return;

    setIsConnecting(true);
    setError(null);

    try {
      const ws = new WebSocket(url);
      wsRef.current = ws;

      ws.onopen = () => {
        setIsConnected(true);
        setIsConnecting(false);
        reconnectCountRef.current = 0;
        onOpen?.();
      };

      ws.onmessage = (event) => {
        try {
          const message = JSON.parse(event.data);

          // Handle different message types
          if (message.type === 'patch') {
            // Apply JSON Patch operations
            setData((prev) => {
              if (!prev) return prev;
              try {
                const operations = message.operations as Operation[];
                const result = applyPatch(prev, operations, true, false);
                return result.newDocument;
              } catch (e) {
                logger.error('Failed to apply patch:', e);
                return prev;
              }
            });
          } else if (message.type === 'full') {
            // Full state replacement
            setData(message.data as T);
          } else if (message.type === 'error') {
            setError(new Error(message.error || 'WebSocket error'));
          }

          onMessage?.(data as T);
        } catch (e) {
          logger.error('Failed to parse WebSocket message:', e);
        }
      };

      ws.onerror = (event) => {
        setError(new Error('WebSocket connection error'));
        onError?.(event);
      };

      ws.onclose = () => {
        setIsConnected(false);
        setIsConnecting(false);
        wsRef.current = null;
        onClose?.();

        // Attempt reconnection
        if (
          enabled &&
          reconnectCountRef.current < reconnectAttempts
        ) {
          reconnectCountRef.current += 1;
          reconnectTimeoutRef.current = setTimeout(() => {
            connect();
          }, reconnectInterval);
        }
      };
    } catch (e) {
      setIsConnecting(false);
      setError(e instanceof Error ? e : new Error('Failed to connect'));
    }
  }, [
    url,
    enabled,
    onMessage,
    onError,
    onOpen,
    onClose,
    reconnectAttempts,
    reconnectInterval,
    data,
  ]);

  const disconnect = useCallback(() => {
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current);
    }
    reconnectCountRef.current = reconnectAttempts; // Prevent reconnection
    wsRef.current?.close();
  }, [reconnectAttempts]);

  const reconnect = useCallback(() => {
    disconnect();
    reconnectCountRef.current = 0;
    setTimeout(connect, 100);
  }, [connect, disconnect]);

  const send = useCallback((message: unknown) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(message));
    }
  }, []);

  useEffect(() => {
    connect();
    return () => {
      disconnect();
    };
  }, [connect, disconnect]);

  return {
    data,
    isConnected,
    isConnecting,
    error,
    send,
    reconnect,
    disconnect,
  };
}

// Specific hook for diff streaming
interface DiffStreamData {
  diffs: import('../types/diff').FileDiff[];
  summary: import('../types/diff').DiffSummary;
}

export function useDiffWsStream(
  attemptId: string | undefined,
  enabled = true
) {
  const wsUrl = attemptId
    ? `${getWsBaseUrl()}/ws/attempts/${attemptId}/diffs`
    : '';

  return useJsonPatchWsStream<DiffStreamData>({
    url: wsUrl,
    enabled: enabled && !!attemptId,
    initialData: {
      diffs: [],
      summary: {
        total_files: 0,
        total_additions: 0,
        total_deletions: 0,
        files_added: 0,
        files_modified: 0,
        files_deleted: 0,
        files_renamed: 0,
      },
    },
  });
}
