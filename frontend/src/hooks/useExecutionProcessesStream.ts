import { useCallback, useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { API_PREFIX, getWsBaseUrl } from '@/api/client';
import {
  getExecutionProcesses,
  type ExecutionProcess,
} from '@/api/executionProcesses';
import {
  isWsCollectionStreamEnabled,
  useWsCollectionStream,
  type WsCollectionParsedMessage,
} from './useWsCollectionStream';

type ExecutionProcessesWsMessage =
  | { type: 'snapshot'; processes: ExecutionProcess[] }
  | { type: 'upsert'; process: ExecutionProcess }
  | { type: 'remove'; process_id: string };

interface SequencedExecutionProcessesWsEnvelope {
  sequence_id: number;
  message: ExecutionProcessesWsMessage;
}

interface GapDetectedMessage {
  type: 'gap_detected';
  requested_since_seq: number;
  max_available_sequence_id: number;
}

export function parseExecutionProcessesWsCollectionMessage(
  rawData: string
): WsCollectionParsedMessage<ExecutionProcess> | null {
  const parsed = JSON.parse(rawData) as
    | ExecutionProcessesWsMessage
    | SequencedExecutionProcessesWsEnvelope
    | GapDetectedMessage;

  if ((parsed as GapDetectedMessage).type === 'gap_detected') {
    const gap = parsed as GapDetectedMessage;
    return {
      type: 'gap_detected',
      requestedSinceSeq: gap.requested_since_seq,
      maxAvailableSequenceId: gap.max_available_sequence_id,
    };
  }

  const envelope = parsed as SequencedExecutionProcessesWsEnvelope;
  const sequenceId = typeof envelope.sequence_id === 'number' ? envelope.sequence_id : undefined;
  const message = envelope.message ? envelope.message : (parsed as ExecutionProcessesWsMessage);

  if (message.type === 'snapshot') {
    return {
      type: 'events',
      sequenceId,
      events: [{ type: 'snapshot', items: message.processes || [] }],
    };
  }
  if (message.type === 'upsert') {
    return {
      type: 'events',
      sequenceId,
      events: [{ type: 'upsert', item: message.process }],
    };
  }
  if (message.type === 'remove') {
    return {
      type: 'events',
      sequenceId,
      events: [{ type: 'remove', id: message.process_id }],
    };
  }
  return null;
}

function sortByCreatedAt(list: ExecutionProcess[]): ExecutionProcess[] {
  return [...list].sort((a, b) => {
    const ta = Date.parse(a.created_at);
    const tb = Date.parse(b.created_at);
    if (ta !== tb) return ta - tb;
    return a.id.localeCompare(b.id);
  });
}

export interface UseExecutionProcessesStreamResult {
  processes: ExecutionProcess[];
  isLoading: boolean;
  isStreaming: boolean;
  error: string | null;
  reconnect: () => void;
  refetch: () => Promise<unknown>;
}

export function useExecutionProcessesStream(
  attemptId: string | undefined
): UseExecutionProcessesStreamResult {
  const wsEnabled = isWsCollectionStreamEnabled();
  const query = useQuery({
    queryKey: ['execution-processes', attemptId],
    queryFn: () => getExecutionProcesses(attemptId!),
    enabled: Boolean(attemptId),
    staleTime: 30_000,
    refetchInterval: wsEnabled ? false : 2_000,
  });

  const wsUrl = useMemo(() => {
    if (!attemptId) return null;
    return `${getWsBaseUrl()}${API_PREFIX}/execution-processes/stream/session/ws?session_id=${encodeURIComponent(attemptId)}`;
  }, [attemptId]);

  const parseMessage = useCallback(
    (rawData: string): WsCollectionParsedMessage<ExecutionProcess> | null =>
      parseExecutionProcessesWsCollectionMessage(rawData),
    []
  );

  const stream = useWsCollectionStream<ExecutionProcess>({
    enabled: Boolean(attemptId) && wsEnabled,
    url: wsUrl,
    getId: (process) => process.id,
    parseMessage,
    sortItems: sortByCreatedAt,
  });

  const processes = useMemo(() => {
    if (stream.items) return stream.items;
    return sortByCreatedAt(query.data || []);
  }, [query.data, stream.items]);

  const error = stream.error || (query.error instanceof Error ? query.error.message : null);

  return {
    processes,
    isLoading: query.isLoading,
    isStreaming: stream.isStreaming,
    error,
    reconnect: stream.reconnect,
    refetch: query.refetch,
  };
}
