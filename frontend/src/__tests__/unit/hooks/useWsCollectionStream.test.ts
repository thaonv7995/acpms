import { describe, expect, it } from 'vitest';
import {
  applyWsCollectionEvents,
  appendSinceSeq,
  decideCollectionSequenceAction,
  shouldEnableWsCollectionStream,
  type WsCollectionEvent,
} from '../../../hooks/useWsCollectionStream';

interface Item {
  id: string;
  created_at: string;
  value: string;
}

function sortByCreatedAt(items: Item[]): Item[] {
  return [...items].sort((a, b) => {
    const ta = Date.parse(a.created_at);
    const tb = Date.parse(b.created_at);
    if (ta !== tb) return ta - tb;
    return a.id.localeCompare(b.id);
  });
}

describe('applyWsCollectionEvents', () => {
  it('applies snapshot, upsert, and remove in one batch', () => {
    const events: WsCollectionEvent<Item>[] = [
      {
        type: 'snapshot',
        items: [
          { id: 'a', created_at: '2026-02-26T10:00:00.000Z', value: 'A' },
          { id: 'b', created_at: '2026-02-26T10:01:00.000Z', value: 'B' },
        ],
      },
      {
        type: 'upsert',
        item: { id: 'b', created_at: '2026-02-26T10:01:00.000Z', value: 'B2' },
      },
      {
        type: 'remove',
        id: 'a',
      },
    ];

    const result = applyWsCollectionEvents<Item>(null, events, (item) => item.id, sortByCreatedAt);
    expect(result).toEqual([
      { id: 'b', created_at: '2026-02-26T10:01:00.000Z', value: 'B2' },
    ]);
  });

  it('replaces existing item when upsert has the same id', () => {
    const previous: Item[] = [
      { id: 'x', created_at: '2026-02-26T10:00:00.000Z', value: 'old' },
    ];

    const events: WsCollectionEvent<Item>[] = [
      {
        type: 'upsert',
        item: { id: 'x', created_at: '2026-02-26T10:00:00.000Z', value: 'new' },
      },
    ];

    const result = applyWsCollectionEvents<Item>(previous, events, (item) => item.id, sortByCreatedAt);
    expect(result).toEqual([
      { id: 'x', created_at: '2026-02-26T10:00:00.000Z', value: 'new' },
    ]);
  });

  it('keeps deterministic ordering when sort function is provided', () => {
    const previous: Item[] = [
      { id: 'b', created_at: '2026-02-26T10:02:00.000Z', value: 'B' },
    ];

    const events: WsCollectionEvent<Item>[] = [
      {
        type: 'upsert',
        item: { id: 'a', created_at: '2026-02-26T10:01:00.000Z', value: 'A' },
      },
    ];

    const result = applyWsCollectionEvents<Item>(previous, events, (item) => item.id, sortByCreatedAt);
    expect(result.map((item) => item.id)).toEqual(['a', 'b']);
  });
});

describe('appendSinceSeq', () => {
  it('adds since_seq query parameter for positive sequence id', () => {
    const result = appendSinceSeq(
      'ws://localhost:3000/api/v1/approvals/stream/ws?projection=patch',
      12
    );
    expect(result).toContain('projection=patch');
    expect(result).toContain('since_seq=12');
  });

  it('replaces existing since_seq parameter', () => {
    const result = appendSinceSeq(
      'ws://localhost:3000/api/v1/execution-processes/stream/session/ws?session_id=a1&since_seq=3',
      8
    );
    expect(result).toContain('session_id=a1');
    expect(result).toContain('since_seq=8');
    expect(result).not.toContain('since_seq=3');
  });

  it('keeps url unchanged when sequence id is not positive', () => {
    const url = 'ws://localhost:3000/api/v1/approvals/stream/ws?projection=patch';
    expect(appendSinceSeq(url, 0)).toBe(url);
    expect(appendSinceSeq(url, -1)).toBe(url);
  });
});

describe('decideCollectionSequenceAction', () => {
  it('ignores stale non-snapshot events', () => {
    expect(decideCollectionSequenceAction(10, 10, false)).toEqual({
      shouldIgnore: true,
      shouldReset: false,
    });
    expect(decideCollectionSequenceAction(10, 9, false)).toEqual({
      shouldIgnore: true,
      shouldReset: false,
    });
  });

  it('requests reset when non-snapshot sequence has a gap', () => {
    expect(decideCollectionSequenceAction(4, 7, false)).toEqual({
      shouldIgnore: false,
      shouldReset: true,
    });
  });

  it('accepts snapshot sequence only when it advances cursor', () => {
    expect(decideCollectionSequenceAction(8, 9, true)).toEqual({
      shouldIgnore: false,
      shouldReset: false,
    });
    expect(decideCollectionSequenceAction(8, 8, true)).toEqual({
      shouldIgnore: true,
      shouldReset: false,
    });
  });
});

describe('shouldEnableWsCollectionStream', () => {
  it('defaults to enabled when flag is missing', () => {
    expect(shouldEnableWsCollectionStream(undefined)).toBe(true);
    expect(shouldEnableWsCollectionStream('')).toBe(true);
  });

  it('disables stream only when flag is explicitly false', () => {
    expect(shouldEnableWsCollectionStream('false')).toBe(false);
    expect(shouldEnableWsCollectionStream(' FALSE ')).toBe(false);
    expect(shouldEnableWsCollectionStream('0')).toBe(true);
    expect(shouldEnableWsCollectionStream('true')).toBe(true);
  });
});
