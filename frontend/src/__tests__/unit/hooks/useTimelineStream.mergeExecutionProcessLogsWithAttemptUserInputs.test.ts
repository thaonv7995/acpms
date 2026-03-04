import { describe, expect, it } from 'vitest';
import {
  isAttemptUserInputLog,
  mergeExecutionProcessLogsWithAttemptUserInputs,
} from '../../../hooks/useTimelineStream';
import type { AgentLog } from '../../../api/taskAttempts';

function buildLog(params: {
  id: string;
  createdAt: string;
  logType: string;
  content: string;
}): AgentLog {
  return {
    id: params.id,
    attempt_id: 'attempt-1',
    log_type: params.logType,
    content: params.content,
    created_at: params.createdAt,
  };
}

describe('isAttemptUserInputLog', () => {
  it('accepts user and stdin logs', () => {
    expect(
      isAttemptUserInputLog(
        buildLog({
          id: 'u-1',
          createdAt: '2026-02-27T10:00:00.000Z',
          logType: 'user',
          content: 'follow-up',
        })
      )
    ).toBe(true);

    expect(
      isAttemptUserInputLog(
        buildLog({
          id: 'u-2',
          createdAt: '2026-02-27T10:00:01.000Z',
          logType: 'stdin',
          content: 'follow-up via stdin',
        })
      )
    ).toBe(true);
  });

  it('rejects non-user logs', () => {
    expect(
      isAttemptUserInputLog(
        buildLog({
          id: 'n-1',
          createdAt: '2026-02-27T10:00:02.000Z',
          logType: 'normalized',
          content: '{}',
        })
      )
    ).toBe(false);
  });
});

describe('mergeExecutionProcessLogsWithAttemptUserInputs', () => {
  it('includes follow-up user logs even when execution-process logs are normalized only', () => {
    const processLogs = [
      buildLog({
        id: 'p-1',
        createdAt: '2026-02-27T10:01:00.000Z',
        logType: 'normalized',
        content: '{"entry_type":{"type":"assistant_message"},"content":"Working..."}',
      }),
      buildLog({
        id: 'p-2',
        createdAt: '2026-02-27T10:02:00.000Z',
        logType: 'normalized',
        content: '{"entry_type":{"type":"tool_use"},"content":"Edit file"}',
      }),
    ];

    const attemptLogs = [
      buildLog({
        id: 'u-1',
        createdAt: '2026-02-27T10:00:30.000Z',
        logType: 'user',
        content: 'Please add retry logic',
      }),
      buildLog({
        id: 's-1',
        createdAt: '2026-02-27T10:00:31.000Z',
        logType: 'system',
        content: 'status message',
      }),
    ];

    const merged = mergeExecutionProcessLogsWithAttemptUserInputs(processLogs, attemptLogs);
    expect(merged.map((log) => log.id)).toEqual(['u-1', 'p-1', 'p-2']);
    expect(merged.find((log) => log.id === 'u-1')?.log_type).toBe('user');
  });

  it('deduplicates when the same user log appears in both streams', () => {
    const userLog = buildLog({
      id: 'u-dup',
      createdAt: '2026-02-27T10:00:30.000Z',
      logType: 'user',
      content: 'same follow-up',
    });

    const merged = mergeExecutionProcessLogsWithAttemptUserInputs(
      [userLog],
      [userLog]
    );

    expect(merged).toHaveLength(1);
    expect(merged[0].id).toBe('u-dup');
  });
});
