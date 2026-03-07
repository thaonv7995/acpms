import { renderHook } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { usePinnedTodos } from '@/hooks/usePinnedTodos';
import type { NormalizedEntry } from '@/bindings/NormalizedEntry';

function makeTodoEntry(timestamp: string, statuses: string[]): NormalizedEntry {
  return {
    timestamp,
    entry_type: {
      type: 'tool_use',
      tool_name: 'TodoWrite',
      action_type: {
        action: 'todo_management',
        todos: [
          {
            content: 'Run preflight checks (references, env vars)',
            status: statuses[0],
            active_form: 'Running preflight checks',
          },
          {
            content: 'Scaffold web application project',
            status: statuses[1],
            active_form: 'Scaffolding web application',
          },
        ],
        operation: 'update',
      },
      status: {
        status: 'success',
      },
    },
    content: 'Todo list updated (2)',
  } as NormalizedEntry;
}

describe('usePinnedTodos', () => {
  it('keeps the newest todo snapshot even when entries arrive newest-first', () => {
    const latestEntry = makeTodoEntry('2026-03-07T08:14:40.714205+00:00', [
      'completed',
      'in_progress',
    ]);
    const olderEntry = makeTodoEntry('2026-03-07T08:14:23.509029+00:00', [
      'in_progress',
      'pending',
    ]);

    const { result } = renderHook(() => usePinnedTodos([latestEntry, olderEntry]));

    expect(result.current.todos[0]?.status).toBe('completed');
    expect(result.current.todos[1]?.status).toBe('in_progress');
    expect(result.current.lastUpdated).toBe('2026-03-07T08:14:40.714205+00:00');
  });
});
