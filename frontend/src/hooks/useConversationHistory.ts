import { useEffect, useRef, useCallback } from 'react';
import { WebSocketService } from '@/services/websocket-service';
import { applyPatches } from '@/utils/json-patch';
import { WebSocketMessage } from '@/types/websocket.types';
import { authenticatedFetch, getAccessToken } from '@/api/client';
import { logger } from '@/lib/logger';

export type AddEntryType = 'initial' | 'running' | 'historic';

export interface ConversationEntry {
  id: string;
  type: string;
  content: any;
  timestamp: string;
}

export interface PatchTypeWithKey extends ConversationEntry {
  patchKey: string;
  executionProcessId: string;
}

export interface ExecutionProcess {
  id: string;
  status: 'running' | 'completed' | 'failed';
  run_reason?: string;
  created_at: string;
}

export interface UseConversationHistoryParams {
  attemptId: string;
  executionProcesses: ExecutionProcess[];
  onEntriesUpdated: (
    entries: PatchTypeWithKey[],
    addType: AddEntryType,
    loading: boolean
  ) => void;
}

const MIN_INITIAL_ENTRIES = 10;
const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || '/api/v1';
const WS_AUTH_PROTOCOL = 'acpms-bearer';

export function flattenConversationEntriesByProcessOrder(
  displayedProcesses: Map<string, PatchTypeWithKey[]>,
  executionProcesses: ExecutionProcess[]
): PatchTypeWithKey[] {
  const sortedProcessIds = Array.from(displayedProcesses.keys()).sort((a, b) => {
    const processA = executionProcesses.find((p) => p.id === a);
    const processB = executionProcesses.find((p) => p.id === b);

    if (processA && processB) {
      const timeDiff = new Date(processA.created_at).getTime() - new Date(processB.created_at).getTime();
      if (timeDiff !== 0) return timeDiff;
      return processA.id.localeCompare(processB.id);
    }

    if (processA) return -1;
    if (processB) return 1;
    return a.localeCompare(b);
  });

  const allEntries: PatchTypeWithKey[] = [];
  for (const processId of sortedProcessIds) {
    const entries = displayedProcesses.get(processId) || [];
    allEntries.push(...entries);
  }

  return allEntries;
}

export function useConversationHistory({
  attemptId,
  executionProcesses,
  onEntriesUpdated,
}: UseConversationHistoryParams) {
  const displayedProcesses = useRef<Map<string, PatchTypeWithKey[]>>(new Map());
  const streamingProcessIds = useRef<Set<string>>(new Set());
  const wsConnections = useRef<Map<string, WebSocketService>>(new Map());

  const emitAllEntries = useCallback(
    (addType: AddEntryType, loading = false) => {
      const allEntries = flattenConversationEntriesByProcessOrder(
        displayedProcesses.current,
        executionProcesses
      );

      onEntriesUpdated(allEntries, addType, loading);
    },
    [executionProcesses, onEntriesUpdated]
  );

  const loadHistoricEntries = useCallback(
    async (process: ExecutionProcess): Promise<ConversationEntry[]> => {
      const url = `${API_BASE_URL}/execution-processes/${process.id}/normalized-logs`;

      try {
        const response = await authenticatedFetch(url);
        if (!response.ok) {
          throw new Error(`Failed to load entries: ${response.statusText}`);
        }

        const data = await response.json();
        // Backend returns ApiResponse with data field containing logs.
        const logs = Array.isArray(data) ? data : (data.data || data.entries || []);

        return logs.map((log: any) => {
          const logType = log.log_type || log.type || 'normalized';
          const content = log.content || log.message || '';
          const timestamp = log.created_at || log.timestamp || new Date().toISOString();
          const entryId = log.id || `${process.id}-${timestamp}`;

          if (logType === 'stdout' || logType === 'stderr') {
            return {
              id: entryId,
              type: logType.toUpperCase() as 'STDOUT' | 'STDERR',
              content,
              timestamp,
            };
          }

          let normalizedContent: any = {
            entry_type: { type: 'system_message' },
            content,
            timestamp,
          };

          if (typeof content === 'string' && content.trim().startsWith('{')) {
            try {
              normalizedContent = JSON.parse(content);
            } catch {
              // Keep fallback shape when historic normalized payload cannot be parsed.
            }
          }

          return {
            id: entryId,
            type: 'NORMALIZED_ENTRY',
            content: normalizedContent,
            timestamp,
          };
        });
      } catch (error) {
        logger.error(`Failed to load historic entries for process ${process.id}:`, error);
        return [];
      }
    },
    []
  );

  const streamRunningProcess = useCallback(
    (process: ExecutionProcess) => {
      if (streamingProcessIds.current.has(process.id)) {
        return;
      }

      streamingProcessIds.current.add(process.id);

      // Stream normalized log events for this execution process.
      const wsProtocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
      const token = getAccessToken();
      const wsUrl = `${wsProtocol}//${window.location.host}${API_BASE_URL}/execution-processes/${process.id}/normalized-logs/ws`;

      const processEntries: ConversationEntry[] = [];

      const ws = new WebSocketService({
        url: wsUrl,
        protocols: token ? [WS_AUTH_PROTOCOL, token] : undefined,
        onMessage: (message: WebSocketMessage) => {
          // Handle sequenced execution-process log stream payload.
          const msg = message as any;
          const eventPayload = msg.type === 'event' && msg.event ? msg.event : null;

          if (msg.type === 'gap_detected') {
            // Reset this process stream and rely on reconnect flow from WebSocketService.
            processEntries.length = 0;
            displayedProcesses.current.delete(process.id);
            emitAllEntries('running');
            return;
          }

          if (eventPayload?.type === 'Log') {
            const logData = eventPayload;
            const timestamp = logData.created_at || logData.timestamp || new Date().toISOString();
            const content = logData.content || '';

            const entry: ConversationEntry =
              logData.log_type === 'normalized'
                ? {
                    id: logData.id || `${process.id}-${timestamp}`,
                    type: 'NORMALIZED_ENTRY',
                    content: (() => {
                      if (typeof content === 'string' && content.trim().startsWith('{')) {
                        try {
                          return JSON.parse(content);
                        } catch {
                          return {
                            entry_type: { type: 'system_message' },
                            content,
                            timestamp,
                          };
                        }
                      }
                      return {
                        entry_type: { type: 'system_message' },
                        content,
                        timestamp,
                      };
                    })(),
                    timestamp,
                  }
                : {
                    id: logData.id || `${process.id}-${timestamp}`,
                    type: logData.log_type === 'stderr' ? 'STDERR' : 'STDOUT',
                    content,
                    timestamp,
                  };

            processEntries.push(entry);

            const withKeys: PatchTypeWithKey[] = processEntries.map((e, i) => ({
              ...e,
              patchKey: `${process.id}:${i}`,
              executionProcessId: process.id,
            }));
            displayedProcesses.current.set(process.id, withKeys);
            emitAllEntries('running');
          } else if (eventPayload?.type === 'Status') {
            const status = eventPayload.status?.toLowerCase();
            if (status === 'success' || status === 'failed' || status === 'cancelled') {
              streamingProcessIds.current.delete(process.id);
              ws.disconnect();
              wsConnections.current.delete(process.id);
            }
          } else if (message.type === 'patch' && message.data) {
            // Legacy patch format support
            const result = applyPatches(processEntries, message.data.patches);

            if (result.success || result.appliedCount > 0) {
              // Convert to PatchTypeWithKey
              const withKeys: PatchTypeWithKey[] = processEntries.map((entry, i) => ({
                ...entry,
                patchKey: `${process.id}:${i}`,
                executionProcessId: process.id,
              }));

              displayedProcesses.current.set(process.id, withKeys);
              emitAllEntries('running');
            }
          } else if (message.type === 'complete') {
            // Stream finished
            streamingProcessIds.current.delete(process.id);
            ws.disconnect();
            wsConnections.current.delete(process.id);
          }
        },
        onError: (error, errorMessage) => {
          logger.error(`WebSocket error for process ${process.id}:`, error, errorMessage);
        },
      });

      ws.connect();
      wsConnections.current.set(process.id, ws);
    },
    [emitAllEntries]
  );

  // Load initial entries from completed processes
  useEffect(() => {
    if (!executionProcesses.length) return;

    let cancelled = false;

    (async () => {
      onEntriesUpdated([], 'initial', true);

      // Load historic entries from ALL processes (including running ones)
      // For running processes, this shows existing logs while WebSocket streams new ones
      const allProcesses = [...executionProcesses].reverse();

      for (const process of allProcesses) {
        if (cancelled) return;

        const entries = await loadHistoricEntries(process);
        const withKeys: PatchTypeWithKey[] = entries.map((entry, i) => ({
          ...entry,
          patchKey: `${process.id}:${i}`,
          executionProcessId: process.id,
        }));

        displayedProcesses.current.set(process.id, withKeys);

        // Stop if we have enough initial entries
        const totalEntries = Array.from(displayedProcesses.current.values()).reduce(
          (sum, e) => sum + e.length,
          0
        );

        if (totalEntries >= MIN_INITIAL_ENTRIES) break;
      }

      if (!cancelled) {
        emitAllEntries('initial', false);
      }
    })();

    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [attemptId, executionProcesses]);

  // Stream running processes
  useEffect(() => {
    const runningProcesses = executionProcesses.filter(
      (p) => p.status === 'running' && p.run_reason !== 'devserver'
    );

    for (const process of runningProcesses) {
      streamRunningProcess(process);
    }

    // Cleanup on unmount
    return () => {
      wsConnections.current.forEach((ws) => ws.disconnect());
      wsConnections.current.clear();
      streamingProcessIds.current.clear();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [executionProcesses]);

  // Reset on attempt change
  useEffect(() => {
    displayedProcesses.current.clear();
    streamingProcessIds.current.clear();
    wsConnections.current.forEach((ws) => ws.disconnect());
    wsConnections.current.clear();
    onEntriesUpdated([], 'initial', true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [attemptId]);
}
