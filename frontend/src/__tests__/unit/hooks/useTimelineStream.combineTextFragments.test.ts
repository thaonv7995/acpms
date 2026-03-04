import { describe, it, expect } from 'vitest';
import { combineTextFragments } from '../../../hooks/useTimelineStream';
import type { TimelineEntry } from '../../../types/timeline-log';

describe('combineTextFragments', () => {
  it('merges streamed stdout token fragments into a single assistant message', () => {
    const entries: TimelineEntry[] = [
      {
        id: 'a',
        type: 'assistant_message',
        timestamp: '2026-02-08T00:00:00.000Z',
        content: 'Hello',
        source: 'stdout',
      },
      {
        id: 'b',
        type: 'assistant_message',
        timestamp: '2026-02-08T00:00:00.500Z',
        content: ' world',
        source: 'stdout',
      },
    ];

    const combined = combineTextFragments(entries);
    expect(combined).toHaveLength(1);
    expect(combined[0].type).toBe('assistant_message');
    expect(combined[0].content).toBe('Hello world');
  });

  it('does not merge stdout tool/status marker lines (not token fragments)', () => {
    const entries: TimelineEntry[] = [
      {
        id: 'a',
        type: 'assistant_message',
        timestamp: '2026-02-08T00:00:00.000Z',
        content: 'Using tool: Bash echo hello',
        source: 'stdout',
      },
      {
        id: 'b',
        type: 'assistant_message',
        timestamp: '2026-02-08T00:00:00.200Z',
        content: '✓ Bash completed',
        source: 'stdout',
      },
    ];

    const combined = combineTextFragments(entries);
    expect(combined).toHaveLength(2);
    expect(combined[0].content).toBe('Using tool: Bash echo hello');
    expect(combined[1].content).toBe('✓ Bash completed');
  });

  it('flushes the token buffer before a stdout marker line', () => {
    const entries: TimelineEntry[] = [
      {
        id: 'a',
        type: 'assistant_message',
        timestamp: '2026-02-08T00:00:00.000Z',
        content: 'Hello',
        source: 'stdout',
      },
      {
        id: 'b',
        type: 'assistant_message',
        timestamp: '2026-02-08T00:00:00.100Z',
        content: ' world',
        source: 'stdout',
      },
      {
        id: 'c',
        type: 'assistant_message',
        timestamp: '2026-02-08T00:00:00.200Z',
        content: '  Using tool: Bash echo hello',
        source: 'stdout',
      },
      {
        id: 'd',
        type: 'assistant_message',
        timestamp: '2026-02-08T00:00:00.300Z',
        content: 'done',
        source: 'stdout',
      },
    ];

    const combined = combineTextFragments(entries);
    expect(combined).toHaveLength(3);
    expect(combined[0].content).toBe('Hello world');
    expect(combined[1].content).toBe('  Using tool: Bash echo hello');
    expect(combined[2].content).toBe('done');
  });
});

