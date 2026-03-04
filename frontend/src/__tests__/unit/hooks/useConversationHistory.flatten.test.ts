import { describe, expect, it } from 'vitest';
import {
  flattenConversationEntriesByProcessOrder,
  type PatchTypeWithKey,
  type ExecutionProcess,
} from '../../../hooks/useConversationHistory';

function entry(processId: string, idx: number): PatchTypeWithKey {
  return {
    id: `${processId}-entry-${idx}`,
    type: 'NORMALIZED_ENTRY',
    content: { message: `${processId}-${idx}` },
    timestamp: `2026-02-26T10:0${idx}:00.000Z`,
    patchKey: `${processId}:${idx}`,
    executionProcessId: processId,
  };
}

describe('flattenConversationEntriesByProcessOrder', () => {
  it('flattens entries by process created_at order regardless of insertion order', () => {
    const displayed = new Map<string, PatchTypeWithKey[]>();
    displayed.set('process-b', [entry('process-b', 0)]);
    displayed.set('process-a', [entry('process-a', 0), entry('process-a', 1)]);

    const executionProcesses: ExecutionProcess[] = [
      {
        id: 'process-b',
        status: 'running',
        created_at: '2026-02-26T10:01:00.000Z',
      },
      {
        id: 'process-a',
        status: 'completed',
        created_at: '2026-02-26T10:00:00.000Z',
      },
    ];

    const flattened = flattenConversationEntriesByProcessOrder(displayed, executionProcesses);
    expect(flattened.map((item) => item.id)).toEqual([
      'process-a-entry-0',
      'process-a-entry-1',
      'process-b-entry-0',
    ]);
  });

  it('uses deterministic process id fallback when process metadata is missing', () => {
    const displayed = new Map<string, PatchTypeWithKey[]>();
    displayed.set('process-z', [entry('process-z', 0)]);
    displayed.set('process-a', [entry('process-a', 0)]);

    const flattened = flattenConversationEntriesByProcessOrder(displayed, []);
    expect(flattened.map((item) => item.executionProcessId)).toEqual(['process-a', 'process-z']);
  });

  it('uses process id as tie-breaker when created_at timestamps are equal', () => {
    const displayed = new Map<string, PatchTypeWithKey[]>();
    displayed.set('process-2', [entry('process-2', 0)]);
    displayed.set('process-1', [entry('process-1', 0)]);

    const executionProcesses: ExecutionProcess[] = [
      {
        id: 'process-2',
        status: 'running',
        created_at: '2026-02-26T10:00:00.000Z',
      },
      {
        id: 'process-1',
        status: 'running',
        created_at: '2026-02-26T10:00:00.000Z',
      },
    ];

    const flattened = flattenConversationEntriesByProcessOrder(displayed, executionProcesses);
    expect(flattened.map((item) => item.executionProcessId)).toEqual(['process-1', 'process-2']);
  });
});
