// Hook for real-time streaming of agent logs for a project
// Supports multiple agents running simultaneously
import { useState, useEffect, useCallback, useRef } from 'react';
import { apiGet, API_PREFIX, getWsBaseUrl } from '../api/client';
import { logger } from '@/lib/logger';
const WS_AUTH_PROTOCOL = 'acpms-bearer';

// Types for agent events
export interface AgentLogEntry {
  id: string;
  type: 'Log' | 'Status';
  attempt_id: string;
  task_id: string;
  task_title: string;
  log_type?: string; // 'stdout' | 'stderr' for Log type
  content?: string; // for Log type
  status?: string; // for Status type
  timestamp: string;
}

export interface ActiveAgent {
  attempt_id: string;
  task_id: string;
  task_title: string;
  task_type: string;
  started_at: string | null;
}

export interface UseProjectAgentLogsResult {
  logs: AgentLogEntry[];
  activeAgents: ActiveAgent[];
  isConnected: boolean;
  isLoading: boolean;
  error: string | null;
  clearLogs: () => void;
  reconnect: () => void;
}

// Fetch active agents for a project
async function fetchActiveAgents(projectId: string): Promise<ActiveAgent[]> {
  // apiGet already extracts body.data via handleResponse
  return await apiGet<ActiveAgent[]>(`${API_PREFIX}/projects/${projectId}/agents/active`);
}

export function useProjectAgentLogs(
  projectId: string | undefined,
  options?: { maxLogs?: number; autoConnect?: boolean }
): UseProjectAgentLogsResult {
  const { maxLogs = 200, autoConnect = true } = options || {};

  const [logs, setLogs] = useState<AgentLogEntry[]>([]);
  const [activeAgents, setActiveAgents] = useState<ActiveAgent[]>([]);
  const [isConnected, setIsConnected] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout>>();
  const logIdCounter = useRef(0);

  // Fetch active agents on mount and periodically
  useEffect(() => {
    if (!projectId) return;

    const loadActiveAgents = async () => {
      try {
        const agents = await fetchActiveAgents(projectId);
        setActiveAgents(agents);
      } catch (err) {
        logger.error('Failed to fetch active agents:', err);
      } finally {
        setIsLoading(false);
      }
    };

    loadActiveAgents();

    // Refresh active agents every 10 seconds
    const interval = setInterval(loadActiveAgents, 10000);
    return () => clearInterval(interval);
  }, [projectId]);

  // Connect to WebSocket
  const connect = useCallback(() => {
    if (!projectId || wsRef.current?.readyState === WebSocket.OPEN) return;

    const token = localStorage.getItem('acpms_token');
    if (!token) {
      setError('Not authenticated');
      return;
    }

    // Close existing connection
    if (wsRef.current) {
      wsRef.current.close();
    }

    const ws = new WebSocket(
      `${getWsBaseUrl()}/api/v1/projects/${projectId}/agents/ws`,
      [WS_AUTH_PROTOCOL, token]
    );
    wsRef.current = ws;

    ws.onopen = () => {
      setIsConnected(true);
      setError(null);
      logger.log('[ProjectAgentLogs] WebSocket connected');
    };

    ws.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data);

        // Server uses #[serde(flatten)] so fields are at root level, not nested under 'event'
        // Support both flat (from ProjectAgentEvent) and nested (fallback) structures
        const eventType = data.type || data.event?.type;
        const logEntry: AgentLogEntry = {
          id: `log-${++logIdCounter.current}`,
          type: eventType,
          attempt_id: data.attempt_id || data.event?.attempt_id,
          task_id: data.task_id,
          task_title: data.task_title,
          timestamp: data.timestamp || data.event?.timestamp || new Date().toISOString(),
        };

        if (eventType === 'Log') {
          // Flat structure: data.log_type, data.content
          logEntry.log_type = data.log_type || data.event?.log_type;
          logEntry.content = data.content || data.event?.content;
        } else if (eventType === 'Status') {
          // Flat structure: data.status
          logEntry.status = data.status || data.event?.status;
        }

        setLogs((prev) => {
          const newLogs = [...prev, logEntry];
          // Keep only the last maxLogs entries
          return newLogs.slice(-maxLogs);
        });
      } catch (err) {
        logger.error('[ProjectAgentLogs] Failed to parse message:', err);
      }
    };

    ws.onerror = (event) => {
      logger.error('[ProjectAgentLogs] WebSocket error:', event);
      setError('WebSocket connection error');
    };

    ws.onclose = (event) => {
      setIsConnected(false);
      wsRef.current = null;
      logger.log('[ProjectAgentLogs] WebSocket closed:', event.code, event.reason);

      // Auto-reconnect after 3 seconds if not intentionally closed
      if (event.code !== 1000 && autoConnect) {
        reconnectTimeoutRef.current = setTimeout(() => {
          logger.log('[ProjectAgentLogs] Attempting reconnect...');
          connect();
        }, 3000);
      }
    };
  }, [projectId, maxLogs, autoConnect]);

  // Auto-connect on mount
  useEffect(() => {
    if (autoConnect && projectId) {
      connect();
    }

    return () => {
      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current);
      }
      if (wsRef.current) {
        wsRef.current.close(1000, 'Component unmounted');
      }
    };
  }, [connect, autoConnect, projectId]);

  // Clear logs
  const clearLogs = useCallback(() => {
    setLogs([]);
  }, []);

  // Manual reconnect
  const reconnect = useCallback(() => {
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current);
    }
    connect();
  }, [connect]);

  return {
    logs,
    activeAgents,
    isConnected,
    isLoading,
    error,
    clearLogs,
    reconnect,
  };
}
