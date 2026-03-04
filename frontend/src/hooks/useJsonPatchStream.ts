import { useEffect, useReducer, useRef, useCallback } from 'react';
import { applyPatch, deepClone, type Operation } from 'fast-json-patch';
import { logger } from '@/lib/logger';

interface StreamMessage {
  type: 'snapshot' | 'patch' | 'gap_detected';
  path?: string;
  data?: any;
  operation?: PatchOperation;
  sequence: number;
  seq?: number;
  // Gap detection
  oldest_available?: number;
  requested?: number;
}

interface PatchOperation {
  op: 'add' | 'replace' | 'remove' | 'move' | 'copy' | 'test';
  path: string;
  value?: any;
  from?: string;
}

interface PatchState<T> {
  data: T | null;
  sequence: number;
  status: 'disconnected' | 'connecting' | 'syncing' | 'synced' | 'reloading';
  error: string | null;
  lastReloadAt?: number;
}

type PatchAction<T> =
  | { type: 'SNAPSHOT'; data: T; sequence: number }
  | { type: 'PATCH'; operation: PatchOperation; sequence: number }
  | { type: 'GAP_DETECTED'; oldest: number; requested: number }
  | { type: 'ERROR'; error: string }
  | { type: 'RECONNECTING' }
  | { type: 'RELOADING' };

function patchReducer<T>(state: PatchState<T>, action: PatchAction<T>): PatchState<T> {
  switch (action.type) {
    case 'SNAPSHOT':
      return {
        data: action.data,
        sequence: action.sequence,
        status: 'synced',
        error: null,
        lastReloadAt: Date.now(),
      };

    case 'PATCH':
      if (!state.data) return state;

      // Sequence gap detection
      if (action.sequence !== state.sequence + 1 && state.sequence > 0) {
        logger.warn(`Sequence gap: expected ${state.sequence + 1}, got ${action.sequence}`);
        return {
          ...state,
          status: 'reloading',
          error: 'Sequence mismatch, reloading...',
        };
      }

      try {
        const newData = deepClone(state.data);
        applyPatch(newData, [action.operation as Operation]);

        return {
          data: newData as T,
          sequence: action.sequence,
          status: 'synced',
          error: null,
        };
      } catch (error: any) {
        logger.error('Patch application failed:', error);
        return {
          ...state,
          error: error.message,
          status: 'reloading',
        };
      }

    case 'GAP_DETECTED':
      logger.error(`Gap detected: requested ${action.requested}, oldest ${action.oldest}`);
      return {
        ...state,
        status: 'reloading',
        error: 'Sequence gap detected, reloading snapshot',
      };

    case 'RELOADING':
      return { ...state, status: 'reloading' };

    case 'ERROR':
      return { ...state, error: action.error, status: 'disconnected' };

    case 'RECONNECTING':
      return { ...state, status: 'connecting' };

    default:
      return state;
  }
}

export function useJsonPatchStream<T>(
  url: string,
  enabled: boolean = true
): PatchState<T> & { reload: () => void } {
  const [state, dispatch] = useReducer<React.Reducer<PatchState<T>, PatchAction<T>>>(
    patchReducer,
    {
      data: null,
      sequence: 0,
      status: 'disconnected',
      error: null,
    }
  );

  const eventSourceRef = useRef<EventSource | null>(null);
  const reconnectTimeoutRef = useRef<NodeJS.Timeout>();
  const shouldReloadRef = useRef(false);

  const reload = useCallback(() => {
    shouldReloadRef.current = true;
    if (eventSourceRef.current) {
      eventSourceRef.current.close();
    }
  }, []);

  useEffect(() => {
    if (!enabled) return;

    const connect = () => {
      dispatch({ type: 'RECONNECTING' });

      const urlWithSeq = shouldReloadRef.current
        ? url
        : `${url}?since=${state.sequence}`;

      shouldReloadRef.current = false;

      const eventSource = new EventSource(urlWithSeq);
      eventSourceRef.current = eventSource;

      eventSource.onmessage = (event) => {
        try {
          const message: StreamMessage = JSON.parse(event.data);

          switch (message.type) {
            case 'snapshot':
              dispatch({
                type: 'SNAPSHOT',
                data: message.data as T,
                sequence: message.seq || message.sequence,
              });
              break;

            case 'patch':
              dispatch({
                type: 'PATCH',
                operation: message.operation!,
                sequence: message.seq || message.sequence,
              });
              break;

            case 'gap_detected':
              dispatch({
                type: 'GAP_DETECTED',
                oldest: message.oldest_available!,
                requested: message.requested!,
              });
              setTimeout(() => reload(), 100);
              break;
          }
        } catch (error) {
          logger.error('Failed to parse SSE message:', error);
        }
      };

      eventSource.onerror = () => {
        eventSource.close();
        dispatch({ type: 'ERROR', error: 'Connection lost' });
        reconnectTimeoutRef.current = setTimeout(connect, 5000);
      };
    };

    connect();

    return () => {
      if (eventSourceRef.current) {
        eventSourceRef.current.close();
      }
      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current);
      }
    };
  }, [url, enabled, state.sequence, reload]);

  // Auto-reload on reloading status
  useEffect(() => {
    if (state.status === 'reloading') {
      reload();
    }
  }, [state.status, reload]);

  return { ...state, reload };
}
