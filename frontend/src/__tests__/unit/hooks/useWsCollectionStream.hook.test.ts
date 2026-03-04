import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi, afterEach } from 'vitest';
import { getAccessToken } from '@/api/client';
import {
  useWsCollectionStream,
  type WsCollectionParsedMessage,
} from '../../../hooks/useWsCollectionStream';

vi.mock('@/api/client', () => ({
  getAccessToken: vi.fn(),
}));

interface Item {
  id: string;
  created_at: string;
  value: string;
}

type TestMessage =
  | {
      type: 'snapshot';
      sequence_id: number;
      items: Item[];
    }
  | {
      type: 'upsert';
      sequence_id: number;
      item: Item;
    }
  | {
      type: 'remove';
      sequence_id: number;
      id: string;
    }
  | {
      type: 'gap_detected';
      requested_since_seq: number;
      max_available_sequence_id: number;
    };

class MockWebSocket {
  static instances: MockWebSocket[] = [];

  readonly url: string;
  readonly protocols: string[];

  onopen: ((event: Event) => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;
  onclose: ((event: CloseEvent) => void) | null = null;

  close = vi.fn(() => {
    this.onclose?.({} as CloseEvent);
  });

  constructor(url: string, protocols?: string | string[]) {
    this.url = url;
    if (Array.isArray(protocols)) {
      this.protocols = protocols;
    } else if (typeof protocols === 'string') {
      this.protocols = [protocols];
    } else {
      this.protocols = [];
    }
    MockWebSocket.instances.push(this);
  }

  emitOpen() {
    this.onopen?.(new Event('open'));
  }

  emit(data: TestMessage) {
    this.onmessage?.({ data: JSON.stringify(data) } as MessageEvent);
  }

  emitError() {
    this.onerror?.(new Event('error'));
  }
}

function parseMessage(rawData: string): WsCollectionParsedMessage<Item> | null {
  const payload = JSON.parse(rawData) as TestMessage;

  if (payload.type === 'gap_detected') {
    return {
      type: 'gap_detected',
      requestedSinceSeq: payload.requested_since_seq,
      maxAvailableSequenceId: payload.max_available_sequence_id,
    };
  }

  if (payload.type === 'snapshot') {
    return {
      type: 'events',
      sequenceId: payload.sequence_id,
      events: [{ type: 'snapshot', items: payload.items }],
    };
  }

  if (payload.type === 'upsert') {
    return {
      type: 'events',
      sequenceId: payload.sequence_id,
      events: [{ type: 'upsert', item: payload.item }],
    };
  }

  if (payload.type === 'remove') {
    return {
      type: 'events',
      sequenceId: payload.sequence_id,
      events: [{ type: 'remove', id: payload.id }],
    };
  }

  return null;
}

function renderStreamHook() {
  return renderHook(() =>
    useWsCollectionStream<Item>({
      enabled: true,
      url: 'ws://localhost:3000/api/v1/execution-processes/stream/session/ws?session_id=attempt-1',
      getId: (item) => item.id,
      parseMessage,
    })
  );
}

function renderInlineCallbackStreamHook() {
  return renderHook(() =>
    useWsCollectionStream<Item>({
      enabled: true,
      url: 'ws://localhost:3000/api/v1/execution-processes/stream/session/ws?session_id=attempt-1',
      getId: (item) => item.id,
      parseMessage: (rawData: string): WsCollectionParsedMessage<Item> | null => parseMessage(rawData),
      sortItems: (items) => [...items],
    })
  );
}

describe('useWsCollectionStream hook', () => {
  beforeEach(() => {
    MockWebSocket.instances = [];
    vi.stubGlobal('WebSocket', MockWebSocket as unknown as typeof WebSocket);
    vi.mocked(getAccessToken).mockReturnValue('token-1');
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.clearAllMocks();
  });

  it('uses since_seq cursor when reconnecting manually', async () => {
    const { result } = renderStreamHook();

    const initialCount = MockWebSocket.instances.length;
    expect(initialCount).toBeGreaterThan(0);
    expect(MockWebSocket.instances[initialCount - 1].protocols).toEqual([
      'acpms-bearer',
      'token-1',
    ]);

    act(() => {
      const ws = MockWebSocket.instances[initialCount - 1];
      ws.emitOpen();
      ws.emit({
        type: 'snapshot',
        sequence_id: 4,
        items: [{ id: 'item-1', created_at: '2026-02-26T10:00:00.000Z', value: 'one' }],
      });
    });

    await waitFor(() => {
      expect(result.current.lastSequenceId).toBe(4);
      expect(result.current.items?.map((item) => item.id)).toEqual(['item-1']);
    });

    act(() => {
      result.current.reconnect();
    });

    await waitFor(() => {
      expect(MockWebSocket.instances.length).toBeGreaterThan(initialCount);
    });

    expect(MockWebSocket.instances[MockWebSocket.instances.length - 1].url).toContain('since_seq=4');
  });

  it('does not reconnect when callback identities change across rerenders', async () => {
    const { result, rerender } = renderInlineCallbackStreamHook();
    const initialCount = MockWebSocket.instances.length;

    expect(initialCount).toBeGreaterThan(0);

    act(() => {
      const ws = MockWebSocket.instances[initialCount - 1];
      ws.emitOpen();
      ws.emit({
        type: 'snapshot',
        sequence_id: 1,
        items: [{ id: 'item-1', created_at: '2026-02-26T10:00:00.000Z', value: 'one' }],
      });
    });

    await waitFor(() => {
      expect(result.current.lastSequenceId).toBe(1);
      expect(result.current.items?.map((item) => item.id)).toEqual(['item-1']);
    });

    act(() => {
      rerender();
    });

    expect(MockWebSocket.instances.length).toBe(initialCount);

    act(() => {
      const ws = MockWebSocket.instances[initialCount - 1];
      ws.emit({
        type: 'upsert',
        sequence_id: 2,
        item: { id: 'item-2', created_at: '2026-02-26T10:01:00.000Z', value: 'two' },
      });
    });

    await waitFor(() => {
      expect(result.current.lastSequenceId).toBe(2);
      expect(result.current.items?.map((item) => item.id)).toEqual(['item-1', 'item-2']);
    });

    expect(MockWebSocket.instances.length).toBe(initialCount);
  });

  it('resets state and reconnects from snapshot when receiving gap_detected', async () => {
    const { result } = renderStreamHook();
    const initialCount = MockWebSocket.instances.length;

    expect(initialCount).toBeGreaterThan(0);

    act(() => {
      const ws = MockWebSocket.instances[initialCount - 1];
      ws.emitOpen();
      ws.emit({
        type: 'snapshot',
        sequence_id: 2,
        items: [{ id: 'item-1', created_at: '2026-02-26T10:00:00.000Z', value: 'one' }],
      });
    });

    await waitFor(() => {
      expect(result.current.lastSequenceId).toBe(2);
      expect(result.current.items?.length).toBe(1);
    });

    act(() => {
      const ws = MockWebSocket.instances[initialCount - 1];
      ws.emit({
        type: 'gap_detected',
        requested_since_seq: 99,
        max_available_sequence_id: 7,
      });
    });

    await waitFor(() => {
      expect(MockWebSocket.instances.length).toBeGreaterThan(initialCount);
    });

    expect(result.current.items).toBeNull();
    expect(result.current.error).toContain('Stream gap detected');
    expect(MockWebSocket.instances[MockWebSocket.instances.length - 1].url).not.toContain(
      'since_seq='
    );
  });

  it('resets and reconnects when live sequence skips ahead without snapshot', async () => {
    const { result } = renderStreamHook();
    const initialCount = MockWebSocket.instances.length;

    expect(initialCount).toBeGreaterThan(0);

    act(() => {
      const ws = MockWebSocket.instances[initialCount - 1];
      ws.emitOpen();
      ws.emit({
        type: 'snapshot',
        sequence_id: 3,
        items: [{ id: 'item-1', created_at: '2026-02-26T10:00:00.000Z', value: 'one' }],
      });
    });

    await waitFor(() => {
      expect(result.current.lastSequenceId).toBe(3);
    });

    act(() => {
      const ws = MockWebSocket.instances[initialCount - 1];
      ws.emit({
        type: 'upsert',
        sequence_id: 6,
        item: { id: 'item-2', created_at: '2026-02-26T10:01:00.000Z', value: 'two' },
      });
    });

    await waitFor(() => {
      expect(MockWebSocket.instances.length).toBeGreaterThan(initialCount);
    });

    expect(result.current.items).toBeNull();
    expect(result.current.error).toBe('Stream sequence gap detected, reconnecting from snapshot');
    expect(MockWebSocket.instances[MockWebSocket.instances.length - 1].url).not.toContain(
      'since_seq='
    );
  });

  it('ignores stale snapshot and stale upsert events after cursor has advanced', async () => {
    const { result } = renderStreamHook();
    const initialCount = MockWebSocket.instances.length;

    expect(initialCount).toBeGreaterThan(0);

    act(() => {
      const ws = MockWebSocket.instances[initialCount - 1];
      ws.emitOpen();
      ws.emit({
        type: 'snapshot',
        sequence_id: 5,
        items: [{ id: 'item-live', created_at: '2026-02-26T10:00:00.000Z', value: 'live' }],
      });
    });

    await waitFor(() => {
      expect(result.current.lastSequenceId).toBe(5);
      expect(result.current.items?.map((item) => item.id)).toEqual(['item-live']);
    });

    act(() => {
      const ws = MockWebSocket.instances[initialCount - 1];
      ws.emit({
        type: 'snapshot',
        sequence_id: 4,
        items: [{ id: 'item-stale', created_at: '2026-02-26T10:01:00.000Z', value: 'stale' }],
      });
      ws.emit({
        type: 'upsert',
        sequence_id: 5,
        item: { id: 'item-stale-upsert', created_at: '2026-02-26T10:02:00.000Z', value: 'stale' },
      });
    });

    await waitFor(() => {
      expect(result.current.lastSequenceId).toBe(5);
      expect(result.current.items?.map((item) => item.id)).toEqual(['item-live']);
    });
  });

  it('applies remove events after upsert updates', async () => {
    const { result } = renderStreamHook();
    const initialCount = MockWebSocket.instances.length;

    expect(initialCount).toBeGreaterThan(0);

    act(() => {
      const ws = MockWebSocket.instances[initialCount - 1];
      ws.emitOpen();
      ws.emit({
        type: 'snapshot',
        sequence_id: 1,
        items: [
          { id: 'item-1', created_at: '2026-02-26T10:00:00.000Z', value: 'one' },
          { id: 'item-2', created_at: '2026-02-26T10:01:00.000Z', value: 'two' },
        ],
      });
      ws.emit({
        type: 'upsert',
        sequence_id: 2,
        item: { id: 'item-3', created_at: '2026-02-26T10:02:00.000Z', value: 'three' },
      });
      ws.emit({
        type: 'remove',
        sequence_id: 3,
        id: 'item-2',
      });
    });

    await waitFor(() => {
      expect(result.current.lastSequenceId).toBe(3);
      expect(result.current.items?.map((item) => item.id)).toEqual(['item-1', 'item-3']);
    });
  });

  it('ignores duplicate sequence events that arrive after sequence has advanced', async () => {
    const { result } = renderStreamHook();
    const initialCount = MockWebSocket.instances.length;

    expect(initialCount).toBeGreaterThan(0);

    act(() => {
      const ws = MockWebSocket.instances[initialCount - 1];
      ws.emitOpen();
      ws.emit({
        type: 'snapshot',
        sequence_id: 7,
        items: [{ id: 'item-1', created_at: '2026-02-26T10:00:00.000Z', value: 'one' }],
      });
      ws.emit({
        type: 'upsert',
        sequence_id: 8,
        item: { id: 'item-2', created_at: '2026-02-26T10:01:00.000Z', value: 'two' },
      });
      ws.emit({
        type: 'upsert',
        sequence_id: 8,
        item: { id: 'item-dup', created_at: '2026-02-26T10:02:00.000Z', value: 'dup' },
      });
    });

    await waitFor(() => {
      expect(result.current.lastSequenceId).toBe(8);
      expect(result.current.items?.map((item) => item.id)).toEqual(['item-1', 'item-2']);
    });
  });

  it('keeps since_seq cursor for reconnect after websocket error', async () => {
    const { result } = renderStreamHook();
    const initialCount = MockWebSocket.instances.length;

    expect(initialCount).toBeGreaterThan(0);

    act(() => {
      const ws = MockWebSocket.instances[initialCount - 1];
      ws.emitOpen();
      ws.emit({
        type: 'snapshot',
        sequence_id: 9,
        items: [{ id: 'item-1', created_at: '2026-02-26T10:00:00.000Z', value: 'one' }],
      });
      ws.emitError();
    });

    await waitFor(() => {
      expect(result.current.lastSequenceId).toBe(9);
      expect(result.current.error).toBe('WebSocket connection error');
      expect(result.current.isStreaming).toBe(false);
    });

    act(() => {
      result.current.reconnect();
    });

    await waitFor(() => {
      expect(MockWebSocket.instances.length).toBeGreaterThan(initialCount);
    });

    expect(MockWebSocket.instances[MockWebSocket.instances.length - 1].url).toContain(
      'since_seq=9'
    );
  });
});
