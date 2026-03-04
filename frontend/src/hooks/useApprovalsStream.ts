import { useCallback, useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { API_PREFIX, getWsBaseUrl } from '@/api/client';
import { getPendingApprovalsForProcess } from '@/api/approvals';
import {
  isWsCollectionStreamEnabled,
  useWsCollectionStream,
  type WsCollectionParsedMessage,
} from './useWsCollectionStream';

export interface ApprovalStreamItem {
  id: string;
  attempt_id: string;
  execution_process_id?: string | null;
  tool_use_id: string;
  tool_name: string;
  status: 'pending' | 'approved' | 'denied' | 'timed_out';
  created_at: string;
  responded_at?: string | null;
}

type ApprovalsWsMessage =
  | { type: 'snapshot'; approvals: ApprovalStreamItem[] }
  | { type: 'upsert'; approval: ApprovalStreamItem }
  | { type: 'remove'; approval_id: string }
  | {
      type: 'snapshot';
      sequence_id: number;
      data?: {
        approvals?: Record<string, ApprovalStreamItem>;
      };
    }
  | {
      type: 'patch';
      sequence_id: number;
      operations: Array<{
        op: 'add' | 'replace' | 'remove';
        path: string;
        value?: ApprovalStreamItem;
      }>;
    }
  | {
      type: 'gap_detected';
      requested_since_seq: number;
      max_available_sequence_id: number;
    };

export function parseApprovalsWsCollectionMessage(
  rawData: string
): WsCollectionParsedMessage<ApprovalStreamItem> | null {
  const message = JSON.parse(rawData) as ApprovalsWsMessage & { data?: any; operations?: any[] };

  if (message.type === 'gap_detected') {
    return {
      type: 'gap_detected',
      requestedSinceSeq: message.requested_since_seq,
      maxAvailableSequenceId: message.max_available_sequence_id,
    };
  }

  if (message.type === 'snapshot' && Array.isArray((message as any).approvals)) {
    return {
      type: 'events',
      sequenceId: (message as any).sequence_id,
      events: [{ type: 'snapshot', items: (message as any).approvals || [] }],
    };
  }

  if (message.type === 'snapshot' && message.data?.approvals) {
    return {
      type: 'events',
      sequenceId: (message as any).sequence_id,
      events: [{ type: 'snapshot', items: Object.values(message.data.approvals) }],
    };
  }

  if (message.type === 'upsert') {
    return {
      type: 'events',
      events: [{ type: 'upsert', item: message.approval }],
    };
  }

  if (message.type === 'remove') {
    return {
      type: 'events',
      events: [{ type: 'remove', id: message.approval_id }],
    };
  }

  if (message.type === 'patch' && Array.isArray(message.operations) && message.operations.length > 0) {
    const events: Array<
      | { type: 'upsert'; item: ApprovalStreamItem }
      | { type: 'remove'; id: string }
    > = [];
    for (const op of message.operations) {
      const id = op.path?.startsWith('/approvals/') ? op.path.slice('/approvals/'.length) : '';
      if (!id) continue;
      if (op.op === 'remove') {
        events.push({ type: 'remove', id });
        continue;
      }
      if ((op.op === 'add' || op.op === 'replace') && op.value) {
        events.push({ type: 'upsert', item: op.value });
      }
    }

    if (events.length === 0) {
      return null;
    }

    return {
      type: 'events',
      sequenceId: message.sequence_id,
      events,
    };
  }

  return null;
}

function sortByCreatedAt(list: ApprovalStreamItem[]): ApprovalStreamItem[] {
  return [...list].sort((a, b) => {
    const ta = Date.parse(a.created_at);
    const tb = Date.parse(b.created_at);
    if (ta !== tb) return ta - tb;
    return a.id.localeCompare(b.id);
  });
}

export interface UseApprovalsStreamParams {
  executionProcessId?: string;
}

export interface UseApprovalsStreamResult {
  approvals: ApprovalStreamItem[];
  pendingApprovals: ApprovalStreamItem[];
  isLoading: boolean;
  isStreaming: boolean;
  error: string | null;
  reconnect: () => void;
}

export function useApprovalsStream({
  executionProcessId,
}: UseApprovalsStreamParams): UseApprovalsStreamResult {
  const enabled = Boolean(executionProcessId);
  const wsEnabled = isWsCollectionStreamEnabled();

  const query = useQuery({
    queryKey: ['pending-approvals', executionProcessId ?? null],
    queryFn: async () => {
      if (executionProcessId) {
        return getPendingApprovalsForProcess(executionProcessId);
      }
      return [];
    },
    enabled,
    staleTime: 30_000,
    refetchInterval: wsEnabled ? false : 2_000,
  });

  const wsUrl = useMemo(() => {
    if (!executionProcessId) return null;
    const params = new URLSearchParams();
    params.set('execution_process_id', executionProcessId);
    params.set('projection', 'patch');
    return `${getWsBaseUrl()}${API_PREFIX}/approvals/stream/ws?${params.toString()}`;
  }, [executionProcessId]);

  const parseMessage = useCallback(
    (rawData: string): WsCollectionParsedMessage<ApprovalStreamItem> | null =>
      parseApprovalsWsCollectionMessage(rawData),
    []
  );

  const stream = useWsCollectionStream<ApprovalStreamItem>({
    enabled: enabled && wsEnabled,
    url: wsUrl,
    getId: (approval) => approval.id,
    parseMessage,
    sortItems: sortByCreatedAt,
  });

  const approvals = useMemo(() => {
    if (stream.items) return stream.items;
    const fallback = query.data ?? [];
    return fallback.map((approval) => ({
      id: approval.id,
      attempt_id: approval.attempt_id,
      execution_process_id: approval.execution_process_id,
      tool_use_id: approval.tool_use_id,
      tool_name: approval.tool_name,
      status: approval.status,
      created_at: approval.created_at,
      responded_at: null,
    }));
  }, [query.data, stream.items]);

  const pendingApprovals = useMemo(
    () => approvals.filter((approval) => approval.status === 'pending'),
    [approvals]
  );

  const error = stream.error || (query.error instanceof Error ? query.error.message : null);

  return {
    approvals,
    pendingApprovals,
    isLoading: query.isLoading,
    isStreaming: stream.isStreaming,
    error,
    reconnect: stream.reconnect,
  };
}
