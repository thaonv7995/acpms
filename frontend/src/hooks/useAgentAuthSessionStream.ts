import { useCallback, useMemo } from 'react';
import { API_PREFIX } from '@/api/client';
import type { AgentAuthSession } from '@/api/settings';
import {
    isWsCollectionStreamEnabled,
    useWsCollectionStream,
    type WsCollectionParsedMessage,
} from './useWsCollectionStream';

const WS_URL = import.meta.env.VITE_WS_URL || 'ws://localhost:3000';

type AgentAuthSessionWsMessage =
    | {
          type: 'snapshot';
          sequence_id: number;
          session: AgentAuthSession;
      }
    | {
          type: 'upsert';
          sequence_id: number;
          session: AgentAuthSession;
      }
    | {
          type: 'gap_detected';
          requested_since_seq: number;
          max_available_sequence_id: number;
      };

export function parseAgentAuthSessionWsMessage(
    rawData: string
): WsCollectionParsedMessage<AgentAuthSession> | null {
    const parsed = JSON.parse(rawData) as AgentAuthSessionWsMessage;

    if (parsed.type === 'gap_detected') {
        return {
            type: 'gap_detected',
            requestedSinceSeq: parsed.requested_since_seq,
            maxAvailableSequenceId: parsed.max_available_sequence_id,
        };
    }

    if (parsed.type === 'snapshot') {
        return {
            type: 'events',
            sequenceId: parsed.sequence_id,
            events: [{ type: 'snapshot', items: [parsed.session] }],
        };
    }

    if (parsed.type === 'upsert') {
        return {
            type: 'events',
            sequenceId: parsed.sequence_id,
            events: [{ type: 'upsert', item: parsed.session }],
        };
    }

    return null;
}

export interface UseAgentAuthSessionStreamResult {
    session: AgentAuthSession | null;
    isStreaming: boolean;
    error: string | null;
    reconnect: () => void;
}

export function useAgentAuthSessionStream(
    sessionId: string | undefined
): UseAgentAuthSessionStreamResult {
    const wsEnabled = isWsCollectionStreamEnabled();

    const wsUrl = useMemo(() => {
        if (!sessionId) return null;
        return `${WS_URL}${API_PREFIX}/agent/auth/sessions/${encodeURIComponent(sessionId)}/ws`;
    }, [sessionId]);

    const parseMessage = useCallback(
        (rawData: string): WsCollectionParsedMessage<AgentAuthSession> | null =>
            parseAgentAuthSessionWsMessage(rawData),
        []
    );

    const stream = useWsCollectionStream<AgentAuthSession>({
        enabled: Boolean(sessionId) && wsEnabled,
        url: wsUrl,
        getId: (session) => session.session_id,
        parseMessage,
    });

    return {
        session: stream.items?.[0] ?? null,
        isStreaming: stream.isStreaming,
        error: stream.error,
        reconnect: stream.reconnect,
    };
}
