import { describe, it, expect } from 'vitest';
import { mergeExecutionProcessLogs } from '../../../hooks/useTimelineStream';
import type { AgentLog } from '../../../api/taskAttempts';

function buildLog(
  id: string,
  createdAt: string,
  content: string,
  attemptId = 'attempt-1'
): AgentLog {
  return {
    id,
    attempt_id: attemptId,
    log_type: 'normalized',
    content,
    created_at: createdAt,
  };
}

describe('mergeExecutionProcessLogs', () => {
  it('merges logs from historical and latest execution process in process order', () => {
    const processIds = ['process-1', 'process-2'];
    const historical = new Map<string, AgentLog[]>();
    historical.set('process-1', [
      buildLog('a1', '2026-02-27T05:00:00.000Z', 'p1-first'),
      buildLog('a2', '2026-02-27T05:01:00.000Z', 'p1-second'),
    ]);

    const latestLogs = [
      buildLog('b1', '2026-02-27T05:02:00.000Z', 'p2-first'),
      buildLog('b2', '2026-02-27T05:03:00.000Z', 'p2-second'),
    ];

    const merged = mergeExecutionProcessLogs(
      processIds,
      historical,
      'process-2',
      latestLogs
    );

    expect(merged.map((log) => log.id)).toEqual(['a1', 'a2', 'b1', 'b2']);
  });

  it('deduplicates overlapping logs by identity key', () => {
    const processIds = ['process-1', 'process-2'];
    const duplicated = buildLog('dup-1', '2026-02-27T05:04:00.000Z', 'same-log');
    const historical = new Map<string, AgentLog[]>();
    historical.set('process-1', [buildLog('a1', '2026-02-27T05:00:00.000Z', 'p1-first')]);
    historical.set('process-2', [duplicated]);

    const merged = mergeExecutionProcessLogs(
      processIds,
      historical,
      'process-2',
      [duplicated, buildLog('b2', '2026-02-27T05:05:00.000Z', 'new-log')]
    );

    expect(merged.map((log) => log.id)).toEqual(['a1', 'dup-1', 'b2']);
  });
});
