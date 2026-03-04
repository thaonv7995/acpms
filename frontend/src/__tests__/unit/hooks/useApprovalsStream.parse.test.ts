import { describe, expect, it } from 'vitest';
import { parseApprovalsWsCollectionMessage } from '../../../hooks/useApprovalsStream';

describe('parseApprovalsWsCollectionMessage', () => {
  it('parses patch message with multiple operations', () => {
    const raw = JSON.stringify({
      type: 'patch',
      sequence_id: 7,
      operations: [
        {
          op: 'add',
          path: '/approvals/a1',
          value: {
            id: 'a1',
            attempt_id: 'attempt-1',
            execution_process_id: 'process-1',
            tool_use_id: 'tool-1',
            tool_name: 'Bash',
            status: 'pending',
            created_at: '2026-02-26T10:00:00.000Z',
            responded_at: null,
          },
        },
        {
          op: 'remove',
          path: '/approvals/a2',
        },
      ],
    });

    const parsed = parseApprovalsWsCollectionMessage(raw);
    expect(parsed).toEqual({
      type: 'events',
      sequenceId: 7,
      events: [
        {
          type: 'upsert',
          item: {
            id: 'a1',
            attempt_id: 'attempt-1',
            execution_process_id: 'process-1',
            tool_use_id: 'tool-1',
            tool_name: 'Bash',
            status: 'pending',
            created_at: '2026-02-26T10:00:00.000Z',
            responded_at: null,
          },
        },
        {
          type: 'remove',
          id: 'a2',
        },
      ],
    });
  });

  it('parses gap_detected message', () => {
    const raw = JSON.stringify({
      type: 'gap_detected',
      requested_since_seq: 12,
      max_available_sequence_id: 3,
    });

    const parsed = parseApprovalsWsCollectionMessage(raw);
    expect(parsed).toEqual({
      type: 'gap_detected',
      requestedSinceSeq: 12,
      maxAvailableSequenceId: 3,
    });
  });

  it('parses legacy snapshot payload', () => {
    const raw = JSON.stringify({
      type: 'snapshot',
      approvals: [
        {
          id: 'a1',
          attempt_id: 'attempt-1',
          execution_process_id: null,
          tool_use_id: 'tool-1',
          tool_name: 'Bash',
          status: 'pending',
          created_at: '2026-02-26T10:00:00.000Z',
          responded_at: null,
        },
      ],
    });

    const parsed = parseApprovalsWsCollectionMessage(raw);
    expect(parsed).toEqual({
      type: 'events',
      sequenceId: undefined,
      events: [
        {
          type: 'snapshot',
          items: [
            {
              id: 'a1',
              attempt_id: 'attempt-1',
              execution_process_id: null,
              tool_use_id: 'tool-1',
              tool_name: 'Bash',
              status: 'pending',
              created_at: '2026-02-26T10:00:00.000Z',
              responded_at: null,
            },
          ],
        },
      ],
    });
  });

  it('parses patch snapshot payload with approvals map', () => {
    const raw = JSON.stringify({
      type: 'snapshot',
      sequence_id: 11,
      data: {
        approvals: {
          a1: {
            id: 'a1',
            attempt_id: 'attempt-1',
            execution_process_id: 'process-1',
            tool_use_id: 'tool-1',
            tool_name: 'Read',
            status: 'approved',
            created_at: '2026-02-26T10:00:00.000Z',
            responded_at: '2026-02-26T10:00:03.000Z',
          },
          a2: {
            id: 'a2',
            attempt_id: 'attempt-1',
            execution_process_id: 'process-1',
            tool_use_id: 'tool-2',
            tool_name: 'Bash',
            status: 'pending',
            created_at: '2026-02-26T10:01:00.000Z',
            responded_at: null,
          },
        },
      },
    });

    const parsed = parseApprovalsWsCollectionMessage(raw);
    expect(parsed).toEqual({
      type: 'events',
      sequenceId: 11,
      events: [
        {
          type: 'snapshot',
          items: [
            {
              id: 'a1',
              attempt_id: 'attempt-1',
              execution_process_id: 'process-1',
              tool_use_id: 'tool-1',
              tool_name: 'Read',
              status: 'approved',
              created_at: '2026-02-26T10:00:00.000Z',
              responded_at: '2026-02-26T10:00:03.000Z',
            },
            {
              id: 'a2',
              attempt_id: 'attempt-1',
              execution_process_id: 'process-1',
              tool_use_id: 'tool-2',
              tool_name: 'Bash',
              status: 'pending',
              created_at: '2026-02-26T10:01:00.000Z',
              responded_at: null,
            },
          ],
        },
      ],
    });
  });

  it('returns null when patch message has no valid operations', () => {
    const raw = JSON.stringify({
      type: 'patch',
      sequence_id: 12,
      operations: [
        {
          op: 'add',
          path: '/invalid/a1',
          value: {
            id: 'a1',
          },
        },
        {
          op: 'copy',
          path: '/approvals/a2',
          value: {
            id: 'a2',
          },
        },
      ],
    });

    const parsed = parseApprovalsWsCollectionMessage(raw);
    expect(parsed).toBeNull();
  });
});
