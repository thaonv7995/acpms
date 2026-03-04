import { describe, expect, it } from 'vitest';
import { normalizeAgentLog, normalizeAgentLogs, type AgentLogWire } from '../../../api/taskAttempts';

describe('normalizeAgentLog', () => {
  it('maps backend dto keys (type/message/timestamp) into canonical agent log fields', () => {
    const wireLog: AgentLogWire = {
      id: 'log-1',
      attempt_id: 'attempt-1',
      type: 'normalized',
      message: '{"type":"Action"}',
      timestamp: '2026-02-27T12:00:00.000Z',
    };

    expect(normalizeAgentLog(wireLog)).toEqual({
      id: 'log-1',
      attempt_id: 'attempt-1',
      log_type: 'normalized',
      content: '{"type":"Action"}',
      created_at: '2026-02-27T12:00:00.000Z',
    });
  });

  it('keeps canonical fields when already present', () => {
    const canonicalLog: AgentLogWire = {
      id: 'log-2',
      attempt_id: 'attempt-1',
      log_type: 'stdout',
      content: 'hello',
      created_at: '2026-02-27T12:01:00.000Z',
      type: 'stderr',
      message: 'ignored',
      timestamp: '2026-02-27T12:01:30.000Z',
    };

    expect(normalizeAgentLog(canonicalLog)).toEqual({
      id: 'log-2',
      attempt_id: 'attempt-1',
      log_type: 'stdout',
      content: 'hello',
      created_at: '2026-02-27T12:01:00.000Z',
    });
  });
});

describe('normalizeAgentLogs', () => {
  it('normalizes a list of mixed log payloads', () => {
    const logs = normalizeAgentLogs([
      {
        id: 'a',
        attempt_id: 'attempt-1',
        type: 'stdout',
        message: 'first',
        timestamp: '2026-02-27T12:00:00.000Z',
      },
      {
        id: 'b',
        attempt_id: 'attempt-1',
        log_type: 'stderr',
        content: 'second',
        created_at: '2026-02-27T12:00:01.000Z',
      },
    ]);

    expect(logs).toEqual([
      {
        id: 'a',
        attempt_id: 'attempt-1',
        log_type: 'stdout',
        content: 'first',
        created_at: '2026-02-27T12:00:00.000Z',
      },
      {
        id: 'b',
        attempt_id: 'attempt-1',
        log_type: 'stderr',
        content: 'second',
        created_at: '2026-02-27T12:00:01.000Z',
      },
    ]);
  });
});
