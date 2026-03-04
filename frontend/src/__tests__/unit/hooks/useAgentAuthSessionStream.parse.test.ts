import { describe, expect, it } from 'vitest';
import { parseAgentAuthSessionWsMessage } from '../../../hooks/useAgentAuthSessionStream';

const baseSession = {
  session_id: 'session-1',
  provider: 'openai-codex',
  flow_type: 'device_flow' as const,
  status: 'waiting_user_action' as const,
  created_at: '2026-02-27T10:00:00.000Z',
  updated_at: '2026-02-27T10:00:00.000Z',
  expires_at: '2026-02-27T10:05:00.000Z',
  process_pid: 1234,
  allowed_loopback_port: null,
  last_seq: 1,
  last_error: null,
  result: null,
  action_url: 'https://github.com/login/device',
  action_code: 'ABCD-1234',
  action_hint: 'Open URL and paste code',
};

describe('parseAgentAuthSessionWsMessage', () => {
  it('parses snapshot payload', () => {
    const raw = JSON.stringify({
      type: 'snapshot',
      sequence_id: 7,
      session: baseSession,
    });

    const parsed = parseAgentAuthSessionWsMessage(raw);
    expect(parsed).toEqual({
      type: 'events',
      sequenceId: 7,
      events: [{ type: 'snapshot', items: [baseSession] }],
    });
  });

  it('parses upsert payload', () => {
    const raw = JSON.stringify({
      type: 'upsert',
      sequence_id: 8,
      session: {
        ...baseSession,
        status: 'verifying',
        last_seq: 8,
      },
    });

    const parsed = parseAgentAuthSessionWsMessage(raw);
    expect(parsed).toEqual({
      type: 'events',
      sequenceId: 8,
      events: [
        {
          type: 'upsert',
          item: {
            ...baseSession,
            status: 'verifying',
            last_seq: 8,
          },
        },
      ],
    });
  });

  it('parses gap_detected payload', () => {
    const raw = JSON.stringify({
      type: 'gap_detected',
      requested_since_seq: 12,
      max_available_sequence_id: 3,
    });

    const parsed = parseAgentAuthSessionWsMessage(raw);
    expect(parsed).toEqual({
      type: 'gap_detected',
      requestedSinceSeq: 12,
      maxAvailableSequenceId: 3,
    });
  });

  it('returns null for unknown payload type', () => {
    const raw = JSON.stringify({
      type: 'noop',
    });
    expect(parseAgentAuthSessionWsMessage(raw)).toBeNull();
  });
});
