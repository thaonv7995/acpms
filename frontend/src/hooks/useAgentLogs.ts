// Custom hook for Agent Logs data fetching + websocket updates
import { useState, useEffect, useCallback, useRef } from 'react';
import { getAgentStatuses } from '../api/agentLogs';
import type { AgentStatus } from '../api/agentLogs';
import { getAccessToken, getWsBaseUrl } from '../api/client';
import { logger } from '@/lib/logger';

interface UseAgentLogsResult {
    statuses: AgentStatus[];
    loading: boolean;
    error: string | null;
    refetch: () => void;
}

type StatusWsMessage =
    | { type: 'snapshot'; statuses: AgentStatus[] }
    | { type: 'upsert'; status: AgentStatus }
    | { type: 'remove'; attempt_id: string };

const WS_AUTH_PROTOCOL = 'acpms-bearer';
const STATUS_WS_PATHS = ['/ws/agent-activity/status', '/api/v1/ws/agent-activity/status'] as const;

export function useAgentLogs(): UseAgentLogsResult {
    const [statuses, setStatuses] = useState<AgentStatus[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const wsRef = useRef<WebSocket | null>(null);
    const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    const isConnectingRef = useRef(false);
    const hasReceivedSnapshotRef = useRef(false);

    const fetchData = useCallback(async (showLoading: boolean = true): Promise<void> => {
        if (showLoading) {
            setLoading(true);
        }

        try {
            const statusesData = await getAgentStatuses();
            setStatuses(statusesData);
        } catch (err) {
            setError(err instanceof Error ? err.message : 'Failed to load agent logs');
        } finally {
            if (showLoading) {
                setLoading(false);
            }
        }
    }, []);

    const clearReconnectTimer = useCallback(() => {
        if (reconnectTimeoutRef.current) {
            clearTimeout(reconnectTimeoutRef.current);
            reconnectTimeoutRef.current = null;
        }
    }, []);

    const disconnect = useCallback((code = 1000, reason = 'cleanup') => {
        clearReconnectTimer();
        if (wsRef.current) {
            wsRef.current.close(code, reason);
            wsRef.current = null;
        }
    }, [clearReconnectTimer]);

    const connect = useCallback((pathIndex = 0) => {
        if (wsRef.current && (wsRef.current.readyState === WebSocket.OPEN || wsRef.current.readyState === WebSocket.CONNECTING)) {
            return;
        }
        if (isConnectingRef.current) return;

        const token = getAccessToken();
        if (!token) {
            setError('Not authenticated');
            setLoading(false);
            return;
        }

        isConnectingRef.current = true;
        const wsPath = STATUS_WS_PATHS[pathIndex] || STATUS_WS_PATHS[0];
        const ws = new WebSocket(`${getWsBaseUrl()}${wsPath}`, [WS_AUTH_PROTOCOL, token]);
        wsRef.current = ws;

        ws.onopen = () => {
            setError(null);
            clearReconnectTimer();
            isConnectingRef.current = false;
        };

        ws.onmessage = (event) => {
            try {
                const message = JSON.parse(event.data) as StatusWsMessage;

                if (message.type === 'snapshot') {
                    hasReceivedSnapshotRef.current = true;
                    setStatuses(message.statuses || []);
                    setLoading(false);
                    return;
                }

                if (message.type === 'upsert') {
                    const next = message.status;
                    setStatuses((prev) => {
                        const idx = prev.findIndex((s) => s.id === next.id);
                        if (idx === -1) return [next, ...prev].slice(0, 50);
                        const updated = prev.slice();
                        updated[idx] = next;
                        return updated;
                    });
                    return;
                }

                if (message.type === 'remove') {
                    setStatuses((prev) => prev.filter((s) => s.id !== message.attempt_id));
                }
            } catch (err) {
                logger.error('[useAgentLogs] Failed to parse WS message:', err);
            }
        };

        ws.onerror = () => {
            setError('WebSocket connection error');
        };

        ws.onclose = (event) => {
            wsRef.current = null;
            isConnectingRef.current = false;
            if (event.code !== 1000) {
                // Fallback: if we never got a snapshot, fetch via HTTP so user has data
                if (!hasReceivedSnapshotRef.current) {
                    void fetchData(false);
                }
                const hasFallback = pathIndex + 1 < STATUS_WS_PATHS.length;
                const nextPathIndex = hasFallback ? pathIndex + 1 : 0;
                if (hasFallback) {
                    reconnectTimeoutRef.current = setTimeout(() => {
                        void connect(nextPathIndex);
                    }, 300);
                } else {
                    reconnectTimeoutRef.current = setTimeout(() => {
                        void connect(nextPathIndex);
                    }, 3000);
                }
            }
        };
    }, [clearReconnectTimer, fetchData]);

    useEffect(() => {
        setError(null);
        // Connect only; server sends snapshot on WS open. No HTTP preflight to avoid double fetch on mount
        // and extra load on every reconnect.
        connect();

        return () => {
            disconnect();
        };
    }, [connect, disconnect]);

    return {
        statuses,
        loading,
        error,
        refetch: () => { void fetchData(true); },
    };
}
