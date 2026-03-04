// Hook for SSE streaming of attempt logs (like vibe-kanban)
import { useEffect, useState, useRef, useCallback } from 'react';
import { API_PREFIX, apiGet, getAccessToken, getWsBaseUrl } from '../api/client';
import { normalizeAgentLogs, type AgentLogWire } from '../api/taskAttempts';
import { applyLogPatches } from '@/utils/stream-patch';

interface LogEntry {
  id?: string;
  attempt_id: string;
  log_type: string;
  content: string;
  timestamp?: string;
  created_at?: string;
  tool_name?: string | null;
}

interface AttemptState {
  id: string;
  status: string;
  started_at?: string;
  completed_at?: string;
}

interface StreamMessage {
  type: 'Snapshot' | 'Patch' | 'GapDetected';
  seq?: number;
  path?: string;
  data?: {
    attempt: AttemptState | null;
    logs: LogEntry[];
    has_more_logs?: boolean;
    snapshot_limit?: number;
  };
  operation?: {
    op: string;
    path: string;
    value: any;
  };
}

interface UseAttemptStreamResult {
  logs: LogEntry[];
  attempt: AttemptState | null;
  isConnected: boolean;
  isLoading: boolean;
  error: string | null;
  connectionState: AttemptStreamConnectionState;
  lastEventAt: number | null;
  hasMoreOlder: boolean;
  isLoadingOlder: boolean;
  loadOlder: () => Promise<void>;
  reconnect: () => void;
}

export type AttemptStreamConnectionState =
  | 'idle'
  | 'connecting'
  | 'live'
  | 'reconnecting'
  | 'stale'
  | 'offline';

const API_URL = import.meta.env.VITE_API_URL || 'http://localhost:3000';
const STALE_THRESHOLD_MS = 15_000;
const STALE_CHECK_INTERVAL_MS = 2_000;
const RECONNECT_SHORT_DELAY_MS = 2_000;
const RECONNECT_LONG_DELAY_MS = 5_000;
const ACTIVE_ATTEMPT_STATUSES = new Set(['running', 'queued']);
const LOG_PAGE_SIZE = 50;
const LOG_PAGE_FETCH_SIZE = LOG_PAGE_SIZE + 1;
/** R4: Vibe Kanban-style debounce. Log patches: 100ms (reduce re-renders during fast streaming). Status: immediate (infrequent, UX-critical). */
const LOG_PATCH_DEBOUNCE_MS = 100;

function getLogTimestamp(log: LogEntry): string {
  return log.created_at ?? log.timestamp ?? '';
}

function buildOlderLogsUrl(
  attemptId: string,
  limit: number,
  before: string,
  beforeId?: string
): string {
  const params = new URLSearchParams();
  params.set('limit', String(limit));
  params.set('before', before);
  if (beforeId) params.set('before_id', beforeId);
  return `${API_PREFIX}/attempts/${attemptId}/logs?${params.toString()}`;
}

export function useAttemptStream(attemptId: string | undefined): UseAttemptStreamResult {
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [attempt, setAttempt] = useState<AttemptState | null>(null);
  const [isConnected, setIsConnected] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [connectionState, setConnectionState] = useState<AttemptStreamConnectionState>('idle');
  const [lastEventAt, setLastEventAt] = useState<number | null>(null);
  const [hasMoreOlder, setHasMoreOlder] = useState(false);
  const [isLoadingOlder, setIsLoadingOlder] = useState(false);

  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const abortControllerRef = useRef<AbortController | null>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectAttemptsRef = useRef(0);
  const wsFailureCountRef = useRef(0);
  const WS_FALLBACK_THRESHOLD = 2;
  const lastSeqRef = useRef<number>(0);
  const attemptStatusRef = useRef<string | undefined>(undefined);
  const pendingLogPatchesRef = useRef<LogEntry[]>([]);
  const logDebounceTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const loadOlder = useCallback(async () => {
    if (!attemptId) return;
    if (isLoadingOlder || !hasMoreOlder) return;
    if (logs.length === 0) {
      setHasMoreOlder(false);
      return;
    }

    const oldestLoaded = logs[0];
    const before = getLogTimestamp(oldestLoaded);
    if (!before) {
      setHasMoreOlder(false);
      return;
    }

    setIsLoadingOlder(true);
    try {
      const url = buildOlderLogsUrl(attemptId, LOG_PAGE_FETCH_SIZE, before, oldestLoaded.id);
      const olderLogsRaw = await apiGet<AgentLogWire[]>(url);
      const olderLogs = normalizeAgentLogs(olderLogsRaw);
      if (!olderLogs || olderLogs.length === 0) {
        setHasMoreOlder(false);
        return;
      }

      const nextHasMore = olderLogs.length > LOG_PAGE_SIZE;
      const page = nextHasMore ? olderLogs.slice(olderLogs.length - LOG_PAGE_SIZE) : olderLogs;

      setLogs((prev) => {
        const existingKeys = new Set(
          prev.map((log) => `${log.id ?? ''}|${getLogTimestamp(log)}|${log.log_type}`)
        );
        const deduped = page.filter((log) => {
          const key = `${log.id ?? ''}|${getLogTimestamp(log)}|${log.log_type}`;
          return !existingKeys.has(key);
        });
        if (deduped.length === 0) return prev;
        return [...deduped, ...prev];
      });
      setHasMoreOlder(nextHasMore);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to load older logs');
    } finally {
      setIsLoadingOlder(false);
    }
  }, [attemptId, hasMoreOlder, isLoadingOlder, logs]);

  const clearReconnectTimer = useCallback(() => {
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current);
      reconnectTimeoutRef.current = null;
    }
  }, []);

  const cancelActiveStream = useCallback(() => {
    if (abortControllerRef.current) {
      abortControllerRef.current.abort();
      abortControllerRef.current = null;
    }
    if (wsRef.current) {
      wsRef.current.close();
      wsRef.current = null;
    }
  }, []);

  const shouldReconnect = useCallback(() => {
    const status = attemptStatusRef.current?.toLowerCase();
    if (!status) return true;
    return ACTIVE_ATTEMPT_STATUSES.has(status);
  }, []);

  const flushPendingLogPatches = useCallback(() => {
    if (logDebounceTimeoutRef.current) {
      clearTimeout(logDebounceTimeoutRef.current);
      logDebounceTimeoutRef.current = null;
    }
    const pending = pendingLogPatchesRef.current;
    if (pending.length === 0) return;
    pendingLogPatchesRef.current = [];

    setLogs((prev) => applyLogPatches(prev, pending) as LogEntry[]);
  }, []);

  const handleMessage = useCallback((message: StreamMessage) => {
    switch (message.type) {
      case 'Snapshot':
        if (logDebounceTimeoutRef.current) {
          clearTimeout(logDebounceTimeoutRef.current);
          logDebounceTimeoutRef.current = null;
        }
        pendingLogPatchesRef.current = [];
        if (message.data) {
          if (message.data.attempt) {
            attemptStatusRef.current = message.data.attempt.status;
            setAttempt(message.data.attempt);
          }
          const rawLogs = message.data.logs || [];
          // Defensive: filter snapshot logs to this attempt (prevents cross-contamination)
          const snapshotLogs =
            attemptId && rawLogs.length > 0
              ? rawLogs.filter((log: LogEntry) => String(log.attempt_id) === String(attemptId))
              : rawLogs;
          setLogs(snapshotLogs);
          if (typeof message.data.has_more_logs === 'boolean') {
            setHasMoreOlder(message.data.has_more_logs);
          } else {
            setHasMoreOlder(snapshotLogs.length >= LOG_PAGE_SIZE);
          }
        }
        if (message.seq) {
          lastSeqRef.current = message.seq;
        }
        break;

      case 'Patch':
        if (message.seq) {
          lastSeqRef.current = message.seq;
        }
        if (message.operation) {
          const { op, path, value } = message.operation;

          if (path === '/logs/-' && op === 'add') {
            const logEntry = value as LogEntry;
            // Defensive: ignore patches for wrong attempt (prevents cross-contamination when
            // multiple attempt streams are active, e.g. Agent Stream page with 2+ running sessions)
            if (attemptId && logEntry.attempt_id && String(logEntry.attempt_id) !== String(attemptId)) {
              return;
            }
            pendingLogPatchesRef.current.push(logEntry);
            if (logDebounceTimeoutRef.current) {
              clearTimeout(logDebounceTimeoutRef.current);
            }
            logDebounceTimeoutRef.current = setTimeout(flushPendingLogPatches, LOG_PATCH_DEBOUNCE_MS);
          } else if (path === '/status' && op === 'replace') {
            const nextStatus = String(value);
            attemptStatusRef.current = nextStatus;
            setAttempt(prev => prev ? { ...prev, status: nextStatus } : null);
          }
        }
        break;

      case 'GapDetected':
        if (logDebounceTimeoutRef.current) {
          clearTimeout(logDebounceTimeoutRef.current);
          logDebounceTimeoutRef.current = null;
        }
        pendingLogPatchesRef.current = [];
        setLogs([]);
        setHasMoreOlder(false);
        lastSeqRef.current = 0;
        break;
    }
  }, [attemptId, flushPendingLogPatches]);

  const connect = useCallback(() => {
    if (!attemptId) {
      setConnectionState('idle');
      setIsLoading(false);
      setIsConnected(false);
      return;
    }

    clearReconnectTimer();
    cancelActiveStream();

    setIsLoading(true);
    setError(null);
    setConnectionState(
      reconnectAttemptsRef.current > 0 ? 'reconnecting' : 'connecting'
    );

    const token = getAccessToken();
    if (!token) {
      setError('Not authenticated');
      setIsLoading(false);
      setIsConnected(false);
      setConnectionState('offline');
      return;
    }

    const since = lastSeqRef.current;
    const streamUrl = `${API_URL}/api/v1/attempts/${attemptId}/stream?since=${since}`;
    const wsStreamUrl = `${getWsBaseUrl().replace(/\/$/, '')}/api/v1/attempts/${attemptId}/stream/ws?since=${since}`;

    const scheduleReconnect = (delayMs: number) => {
      if (!shouldReconnect()) {
        setConnectionState('offline');
        return;
      }
      setConnectionState('reconnecting');
      reconnectTimeoutRef.current = setTimeout(() => {
        reconnectAttemptsRef.current += 1;
        connect();
      }, delayMs);
    };

    const tryWebSocket = () => {
      const ws = new WebSocket(wsStreamUrl, ['acpms-bearer', token]);
      wsRef.current = ws;

      ws.onopen = () => {
        reconnectAttemptsRef.current = 0;
        wsFailureCountRef.current = 0;
        setIsConnected(true);
        setIsLoading(false);
        setConnectionState('live');
        setLastEventAt(Date.now());
      };

      ws.onmessage = (event) => {
        try {
          const message = JSON.parse(event.data) as StreamMessage;
          setLastEventAt(Date.now());
          setConnectionState((prev) => (prev === 'stale' ? 'live' : prev));
          handleMessage(message);
        } catch (e) {
          setError(e instanceof Error ? e.message : 'Failed to parse stream message');
        }
      };

      ws.onerror = () => {
        wsFailureCountRef.current += 1;
        if (wsFailureCountRef.current >= WS_FALLBACK_THRESHOLD) {
          ws.close();
          wsRef.current = null;
          fetchSSE();
        }
      };

      ws.onclose = () => {
        wsRef.current = null;
        setIsConnected(false);
        if (wsFailureCountRef.current < WS_FALLBACK_THRESHOLD) {
          scheduleReconnect(RECONNECT_SHORT_DELAY_MS);
        }
        // If we fell back to SSE, onclose fires after ws.close() - don't double-schedule
      };
    };

    const fetchSSE = async () => {
      const abortController = new AbortController();
      abortControllerRef.current = abortController;

      try {
        const response = await fetch(streamUrl, {
          headers: {
            'Authorization': `Bearer ${token}`,
            'Accept': 'text/event-stream',
          },
          signal: abortController.signal,
        });

        if (!response.ok) {
          throw new Error(`HTTP ${response.status}: ${response.statusText}`);
        }

        const reader = response.body?.getReader();
        if (!reader) {
          throw new Error('No response body');
        }

        reconnectAttemptsRef.current = 0;
        setIsConnected(true);
        setIsLoading(false);
        setConnectionState('live');
        setLastEventAt(Date.now());

        const decoder = new TextDecoder();
        let buffer = '';

        while (true) {
          const { done, value } = await reader.read();
          if (done) break;

          buffer += decoder.decode(value, { stream: true });
          const lines = buffer.split('\n');
          buffer = lines.pop() || '';

          for (const line of lines) {
            if (line.startsWith('data: ')) {
              const data = line.slice(6);
              if (data && data !== 'error') {
                try {
                  const message = JSON.parse(data) as StreamMessage;
                  setLastEventAt(Date.now());
                  setConnectionState((prev) => (prev === 'stale' ? 'live' : prev));
                  handleMessage(message);
                } catch (e) {
                  setError(
                    e instanceof Error ? e.message : 'Failed to parse stream message'
                  );
                }
              }
            }
          }
        }

        setIsConnected(false);
        if (abortController.signal.aborted) return;
        scheduleReconnect(RECONNECT_SHORT_DELAY_MS);
      } catch (e) {
        if (abortController.signal.aborted) return;

        setError(e instanceof Error ? e.message : 'Connection failed');
        setIsConnected(false);
        setIsLoading(false);
        scheduleReconnect(RECONNECT_LONG_DELAY_MS);
      }
    };

    if (wsFailureCountRef.current >= WS_FALLBACK_THRESHOLD) {
      fetchSSE();
    } else {
      tryWebSocket();
    }
  }, [attemptId, cancelActiveStream, clearReconnectTimer, shouldReconnect, handleMessage]);

  useEffect(() => {
    if (!attemptId || !isConnected) return;

    const timer = setInterval(() => {
      if (!shouldReconnect()) return;
      if (!lastEventAt) return;

      const isStale = Date.now() - lastEventAt > STALE_THRESHOLD_MS;
      setConnectionState((prev) => {
        if (isStale && prev === 'live') return 'stale';
        if (!isStale && prev === 'stale') return 'live';
        return prev;
      });
    }, STALE_CHECK_INTERVAL_MS);

    return () => clearInterval(timer);
  }, [attemptId, isConnected, lastEventAt, shouldReconnect]);

  // Connect when attemptId changes
  useEffect(() => {
    if (!attemptId) {
      if (logDebounceTimeoutRef.current) {
        clearTimeout(logDebounceTimeoutRef.current);
        logDebounceTimeoutRef.current = null;
      }
      pendingLogPatchesRef.current = [];
      setLogs([]);
      setAttempt(null);
      setIsConnected(false);
      setIsLoading(false);
      setIsLoadingOlder(false);
      setHasMoreOlder(false);
      setError(null);
      setConnectionState('idle');
      setLastEventAt(null);
      attemptStatusRef.current = undefined;
      lastSeqRef.current = 0;
      reconnectAttemptsRef.current = 0;
      wsFailureCountRef.current = 0;
      clearReconnectTimer();
      cancelActiveStream();
      return;
    }

    if (logDebounceTimeoutRef.current) {
      clearTimeout(logDebounceTimeoutRef.current);
      logDebounceTimeoutRef.current = null;
    }
    pendingLogPatchesRef.current = [];
    setLogs([]);
    setAttempt(null);
    setIsConnected(false);
    setIsLoading(true);
    setIsLoadingOlder(false);
    setHasMoreOlder(false);
    setError(null);
    setConnectionState('connecting');
    setLastEventAt(null);
    attemptStatusRef.current = undefined;
    lastSeqRef.current = 0;
    reconnectAttemptsRef.current = 0;
    wsFailureCountRef.current = 0;
    connect();

    return () => {
      if (logDebounceTimeoutRef.current) {
        clearTimeout(logDebounceTimeoutRef.current);
        logDebounceTimeoutRef.current = null;
      }
      pendingLogPatchesRef.current = [];
      clearReconnectTimer();
      cancelActiveStream();
    };
  }, [attemptId, connect, clearReconnectTimer, cancelActiveStream]);

  const reconnect = useCallback(() => {
    reconnectAttemptsRef.current = 0;
    clearReconnectTimer();
    cancelActiveStream();
    connect();
  }, [cancelActiveStream, clearReconnectTimer, connect]);

  return {
    logs,
    attempt,
    isConnected,
    isLoading,
    error,
    connectionState,
    lastEventAt,
    hasMoreOlder,
    isLoadingOlder,
    loadOlder,
    reconnect,
  };
}
