import { useEffect, useMemo, useRef, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { getAccessToken, API_PREFIX, getWsBaseUrl } from '@/api/client';
import {
  getExecutionProcessNormalizedLogs,
  getExecutionProcessRawLogs,
} from '@/api/executionProcesses';
import type { AgentLog } from '@/api/taskAttempts';
import { logger } from '@/lib/logger';
const WS_AUTH_PROTOCOL = 'acpms-bearer';

type LogStreamMode = 'raw' | 'normalized';

interface AgentEventPayload {
  type: 'Log' | 'Status' | 'ApprovalRequest' | 'UserMessage';
  attempt_id: string;
  log_type?: string;
  content?: string;
  timestamp: string;
  created_at?: string;
  id?: string;
  status?: string;
}

interface SequencedAgentEvent {
  type?: 'event';
  sequence_id: number;
  event: AgentEventPayload;
}

interface GapDetectedMessage {
  type: 'gap_detected';
  requested_since_seq: number;
  max_available_sequence_id: number;
}

export type ParsedExecutionProcessLogStreamMessage =
  | {
      type: 'event';
      sequenceId: number;
      event: AgentEventPayload;
    }
  | {
      type: 'gap_detected';
      requestedSinceSeq: number;
      maxAvailableSequenceId: number;
    };

export interface UseExecutionProcessLogsStreamResult {
  logs: AgentLog[];
  isLoading: boolean;
  isStreaming: boolean;
  error: string | null;
  attemptStatus: string | null;
  lastSequenceId: number;
  reconnect: () => void;
  refetch: () => Promise<unknown>;
}

function sortLogs(logs: AgentLog[]): AgentLog[] {
  return [...logs].sort((a, b) => {
    const ta = Date.parse(a.created_at);
    const tb = Date.parse(b.created_at);
    if (ta !== tb) return ta - tb;
    return a.id.localeCompare(b.id);
  });
}

function logKey(log: AgentLog): string {
  return `${log.id}|${log.created_at}|${log.log_type}|${log.content}`;
}

function normalizeLogEvent(event: AgentEventPayload): AgentLog | null {
  if (event.type !== 'Log') return null;
  return {
    id: event.id || `${event.attempt_id}-${event.timestamp}`,
    attempt_id: event.attempt_id,
    log_type: event.log_type || 'stdout',
    content: event.content || '',
    created_at: event.created_at || event.timestamp,
  };
}

export function parseExecutionProcessLogStreamMessage(
  rawData: string
): ParsedExecutionProcessLogStreamMessage | null {
  const payload = JSON.parse(rawData) as SequencedAgentEvent | GapDetectedMessage;

  if ((payload as GapDetectedMessage).type === 'gap_detected') {
    const gap = payload as GapDetectedMessage;
    return {
      type: 'gap_detected',
      requestedSinceSeq: gap.requested_since_seq,
      maxAvailableSequenceId: gap.max_available_sequence_id,
    };
  }

  const sequenced = (payload as SequencedAgentEvent).event
    ? (payload as SequencedAgentEvent)
    : null;
  if (!sequenced || typeof sequenced.sequence_id !== 'number') {
    return null;
  }

  return {
    type: 'event',
    sequenceId: sequenced.sequence_id,
    event: sequenced.event,
  };
}

export function classifySequenceAdvance(
  lastSequence: number,
  incomingSequence: number
): 'accept' | 'ignore' | 'gap' {
  if (incomingSequence <= lastSequence) {
    return 'ignore';
  }
  if (lastSequence > 0 && incomingSequence > lastSequence + 1) {
    return 'gap';
  }
  return 'accept';
}

export function useExecutionProcessLogsStream(
  processId: string | undefined,
  mode: LogStreamMode
): UseExecutionProcessLogsStreamResult {
  const [isStreaming, setIsStreaming] = useState(false);
  const [streamError, setStreamError] = useState<string | null>(null);
  const [wsLogs, setWsLogs] = useState<AgentLog[]>([]);
  const [attemptStatus, setAttemptStatus] = useState<string | null>(null);
  const [reconnectNonce, setReconnectNonce] = useState(0);
  const lastSequenceRef = useRef(0);

  const query = useQuery({
    queryKey: ['execution-process-logs', processId, mode],
    queryFn: () => {
      if (!processId) return Promise.resolve([]);
      if (mode === 'normalized') return getExecutionProcessNormalizedLogs(processId);
      return getExecutionProcessRawLogs(processId);
    },
    enabled: Boolean(processId),
    staleTime: 30_000,
  });

  useEffect(() => {
    setWsLogs([]);
    setAttemptStatus(null);
    setStreamError(null);
    setIsStreaming(false);
    lastSequenceRef.current = 0;
  }, [mode, processId]);

  useEffect(() => {
    if (!processId) return;
    if (!query.data) return;
    if (lastSequenceRef.current > 0) return;
    lastSequenceRef.current = query.data.length;
  }, [processId, query.data]);

  useEffect(() => {
    if (!processId) return;

    const token = getAccessToken();
    const endpoint = mode === 'normalized' ? 'normalized-logs' : 'raw-logs';
    const wsUrl = `${getWsBaseUrl()}${API_PREFIX}/execution-processes/${processId}/${endpoint}/ws?since_seq=${lastSequenceRef.current}`;
    const ws = token
      ? new WebSocket(wsUrl, [WS_AUTH_PROTOCOL, token])
      : new WebSocket(wsUrl);

    ws.onopen = () => {
      setIsStreaming(true);
      setStreamError(null);
    };

    ws.onmessage = (event) => {
      try {
        const parsed = parseExecutionProcessLogStreamMessage(event.data);
        if (!parsed) {
          return;
        }

        if (parsed.type === 'gap_detected') {
          setStreamError(
            `Stream gap detected (requested ${parsed.requestedSinceSeq}, max ${parsed.maxAvailableSequenceId}), resyncing`
          );
          setWsLogs([]);
          setIsStreaming(false);
          lastSequenceRef.current = 0;
          void query.refetch().finally(() => {
            setReconnectNonce((value) => value + 1);
          });
          ws.close();
          return;
        }

        const sequenceId = parsed.sequenceId;
        const lastSequence = lastSequenceRef.current;

        const sequenceDecision = classifySequenceAdvance(lastSequence, sequenceId);
        if (sequenceDecision === 'ignore') {
          return;
        }

        if (sequenceDecision === 'gap') {
          setStreamError('Stream gap detected, resyncing from snapshot');
          setWsLogs([]);
          setIsStreaming(false);
          lastSequenceRef.current = 0;
          void query.refetch().finally(() => {
            setReconnectNonce((value) => value + 1);
          });
          ws.close();
          return;
        }

        lastSequenceRef.current = sequenceId;
        const evt = parsed.event;

        if (evt.type === 'Status') {
          setAttemptStatus(evt.status ? evt.status.toLowerCase() : null);
          return;
        }

        const nextLog = normalizeLogEvent(evt);
        if (!nextLog) return;

        setWsLogs((prev) => {
          const existing = new Set(prev.map(logKey));
          const nextKey = logKey(nextLog);
          if (existing.has(nextKey)) return prev;
          return sortLogs([...prev, nextLog]);
        });
      } catch (error) {
        logger.error('Failed to parse execution process logs stream message', error);
      }
    };

    ws.onerror = () => {
      setIsStreaming(false);
      setStreamError('Execution process logs stream connection error');
    };

    ws.onclose = () => {
      setIsStreaming(false);
    };

    return () => {
      ws.close();
    };
  }, [mode, processId, query.refetch, reconnectNonce]);

  const logs = useMemo(() => {
    const base = sortLogs(query.data || []);
    if (wsLogs.length === 0) return base;

    const seen = new Set(base.map(logKey));
    const merged = [...base];
    for (const log of wsLogs) {
      const key = logKey(log);
      if (seen.has(key)) continue;
      merged.push(log);
      seen.add(key);
    }
    return sortLogs(merged);
  }, [query.data, wsLogs]);

  const error = streamError || (query.error instanceof Error ? query.error.message : null);

  return {
    logs,
    isLoading: query.isLoading,
    isStreaming,
    error,
    attemptStatus,
    lastSequenceId: lastSequenceRef.current,
    reconnect: () => setReconnectNonce((value) => value + 1),
    refetch: query.refetch,
  };
}
