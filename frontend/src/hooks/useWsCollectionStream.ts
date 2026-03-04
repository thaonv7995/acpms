import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { getAccessToken } from '@/api/client';
import { logger } from '@/lib/logger';

const WS_AUTH_PROTOCOL = 'acpms-bearer';

export function shouldEnableWsCollectionStream(flagValue: string | undefined): boolean {
  if (!flagValue) {
    return true;
  }
  return flagValue.trim().toLowerCase() !== 'false';
}

const WS_COLLECTION_STREAM_ENABLED = shouldEnableWsCollectionStream(
  import.meta.env.VITE_ENABLE_WS_COLLECTION_STREAM
);

export function isWsCollectionStreamEnabled(): boolean {
  return WS_COLLECTION_STREAM_ENABLED;
}

export type WsCollectionEvent<TItem> =
  | { type: 'snapshot'; items: TItem[] }
  | { type: 'upsert'; item: TItem }
  | { type: 'remove'; id: string };

export type WsCollectionParsedMessage<TItem> =
  | {
      type: 'events';
      events: WsCollectionEvent<TItem>[];
      sequenceId?: number;
    }
  | {
      type: 'gap_detected';
      requestedSinceSeq: number;
      maxAvailableSequenceId: number;
    };

export interface UseWsCollectionStreamOptions<TItem> {
  enabled: boolean;
  url: string | null;
  getId: (item: TItem) => string;
  parseMessage: (rawData: string) => WsCollectionParsedMessage<TItem> | null;
  sortItems?: (items: TItem[]) => TItem[];
}

export interface UseWsCollectionStreamResult<TItem> {
  items: TItem[] | null;
  isStreaming: boolean;
  error: string | null;
  lastSequenceId: number;
  reconnect: () => void;
}

export interface CollectionSequenceDecision {
  shouldIgnore: boolean;
  shouldReset: boolean;
}

export function decideCollectionSequenceAction(
  previousSequence: number,
  incomingSequence: number,
  hasSnapshot: boolean
): CollectionSequenceDecision {
  if (!Number.isFinite(incomingSequence)) {
    return { shouldIgnore: true, shouldReset: false };
  }

  if (hasSnapshot) {
    if (previousSequence > 0 && incomingSequence <= previousSequence) {
      return { shouldIgnore: true, shouldReset: false };
    }
    return { shouldIgnore: false, shouldReset: false };
  }

  if (incomingSequence <= previousSequence) {
    return { shouldIgnore: true, shouldReset: false };
  }

  if (previousSequence > 0 && incomingSequence > previousSequence + 1) {
    return { shouldIgnore: false, shouldReset: true };
  }

  return { shouldIgnore: false, shouldReset: false };
}

export function applyWsCollectionEvents<TItem>(
  previousItems: TItem[] | null,
  events: WsCollectionEvent<TItem>[],
  getId: (item: TItem) => string,
  sortItems?: (items: TItem[]) => TItem[]
): TItem[] {
  let next = [...(previousItems ?? [])];

  for (const event of events) {
    if (event.type === 'snapshot') {
      next = [...event.items];
      continue;
    }

    if (event.type === 'upsert') {
      next = next.filter((item) => getId(item) !== getId(event.item));
      next.push(event.item);
      continue;
    }

    next = next.filter((item) => getId(item) !== event.id);
  }

  return sortItems ? sortItems(next) : next;
}

export function appendSinceSeq(url: string, sinceSeq: number): string {
  if (!Number.isFinite(sinceSeq) || sinceSeq <= 0) {
    return url;
  }

  try {
    const parsedUrl = new URL(url);
    parsedUrl.searchParams.set('since_seq', String(Math.floor(sinceSeq)));
    return parsedUrl.toString();
  } catch {
    const separator = url.includes('?') ? '&' : '?';
    return `${url}${separator}since_seq=${encodeURIComponent(String(Math.floor(sinceSeq)))}`;
  }
}

export function useWsCollectionStream<TItem>({
  enabled,
  url,
  getId,
  parseMessage,
  sortItems,
}: UseWsCollectionStreamOptions<TItem>): UseWsCollectionStreamResult<TItem> {
  const [isStreaming, setIsStreaming] = useState(false);
  const [streamError, setStreamError] = useState<string | null>(null);
  const [items, setItems] = useState<TItem[] | null>(null);

  const wsRef = useRef<WebSocket | null>(null);
  const reconnectNonceRef = useRef(0);
  const lastSequenceRef = useRef(0);
  const getIdRef = useRef(getId);
  const parseMessageRef = useRef(parseMessage);
  const sortItemsRef = useRef(sortItems);
  const [reconnectNonce, setReconnectNonce] = useState(0);

  useEffect(() => {
    getIdRef.current = getId;
  }, [getId]);

  useEffect(() => {
    parseMessageRef.current = parseMessage;
  }, [parseMessage]);

  useEffect(() => {
    sortItemsRef.current = sortItems;
  }, [sortItems]);

  useEffect(() => {
    if (!enabled || !url || !WS_COLLECTION_STREAM_ENABLED) {
      setIsStreaming(false);
      setStreamError(null);
      setItems(null);
      lastSequenceRef.current = 0;
      return;
    }

    const token = getAccessToken();
    const wsUrl = appendSinceSeq(url, lastSequenceRef.current);
    const ws = token ? new WebSocket(wsUrl, [WS_AUTH_PROTOCOL, token]) : new WebSocket(wsUrl);
    wsRef.current = ws;

    ws.onopen = () => {
      setIsStreaming(true);
      setStreamError(null);
    };

    ws.onmessage = (event) => {
      try {
        const parsed = parseMessageRef.current(event.data);
        if (!parsed) return;

        if (parsed.type === 'gap_detected') {
          setStreamError(
            `Stream gap detected (requested ${parsed.requestedSinceSeq}, max ${parsed.maxAvailableSequenceId}), reconnecting`
          );
          setIsStreaming(false);
          setItems(null);
          lastSequenceRef.current = 0;
          setReconnectNonce((value) => value + 1);
          ws.close();
          return;
        }

        if (parsed.events.length === 0) {
          return;
        }

        if (typeof parsed.sequenceId === 'number') {
          const hasSnapshot = parsed.events.some((streamEvent) => streamEvent.type === 'snapshot');
          const previousSequence = lastSequenceRef.current;
          const sequenceDecision = decideCollectionSequenceAction(
            previousSequence,
            parsed.sequenceId,
            hasSnapshot
          );

          if (sequenceDecision.shouldIgnore) {
            return;
          }
          if (sequenceDecision.shouldReset) {
            setStreamError('Stream sequence gap detected, reconnecting from snapshot');
            setIsStreaming(false);
            setItems(null);
            lastSequenceRef.current = 0;
            setReconnectNonce((value) => value + 1);
            ws.close();
            return;
          }

          lastSequenceRef.current = parsed.sequenceId;
        }

        setItems((previousItems) =>
          applyWsCollectionEvents(
            previousItems,
            parsed.events,
            getIdRef.current,
            sortItemsRef.current
          )
        );
      } catch (error) {
        logger.error('Failed to handle collection WS message', error);
      }
    };

    ws.onerror = () => {
      setIsStreaming(false);
      setStreamError('WebSocket connection error');
    };

    ws.onclose = () => {
      setIsStreaming(false);
      wsRef.current = null;
    };

    return () => {
      ws.close();
    };
  }, [enabled, reconnectNonce, url]);

  const reconnect = useCallback(() => {
    reconnectNonceRef.current += 1;
    setReconnectNonce(reconnectNonceRef.current);
  }, []);

  return useMemo(
    () => ({
      items,
      isStreaming,
      error: streamError,
      lastSequenceId: lastSequenceRef.current,
      reconnect,
    }),
    [items, isStreaming, streamError, reconnect]
  );
}
