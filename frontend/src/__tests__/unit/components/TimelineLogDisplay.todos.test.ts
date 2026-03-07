import { describe, expect, it } from 'vitest';
import { extractLatestRuntimeTodos } from '@/components/timeline-log/TimelineLogDisplay';
import type { TimelineEntry } from '@/types/timeline-log';

function makeTodoToolEntry(timestamp: string, statuses: string[]): TimelineEntry {
  return {
    id: timestamp,
    type: 'tool_call',
    timestamp,
    toolName: 'TodoWrite',
    actionType: {
      action: 'todo_management',
      todos: [
        {
          content: 'Run preflight checks (references, env vars)',
          status: statuses[0],
        },
        {
          content: 'Scaffold web application project',
          status: statuses[1],
        },
      ],
      operation: 'update',
    },
    status: 'success',
  };
}

describe('extractLatestRuntimeTodos', () => {
  it('prefers the newest todo status over older snapshots', () => {
    const entries: TimelineEntry[] = [
      makeTodoToolEntry('2026-03-07T08:14:40.714205+00:00', [
        'completed',
        'in_progress',
      ]),
      makeTodoToolEntry('2026-03-07T08:14:23.509029+00:00', [
        'in_progress',
        'pending',
      ]),
    ];

    const todos = extractLatestRuntimeTodos(entries);

    expect(todos).toEqual([
      {
        content: 'Run preflight checks (references, env vars)',
        status: 'completed',
      },
      {
        content: 'Scaffold web application project',
        status: 'in_progress',
      },
    ]);
  });
});
