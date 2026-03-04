import { describe, expect, it } from 'vitest';
import { parseExecutionProcessesWsCollectionMessage } from '../../../hooks/useExecutionProcessesStream';

describe('parseExecutionProcessesWsCollectionMessage', () => {
  it('parses sequenced snapshot envelope', () => {
    const raw = JSON.stringify({
      sequence_id: 4,
      message: {
        type: 'snapshot',
        processes: [
          {
            id: 'p1',
            attempt_id: 'a1',
            process_id: 123,
            worktree_path: '/tmp/worktree',
            branch_name: 'feature/demo',
            created_at: '2026-02-26T10:00:00.000Z',
          },
        ],
      },
    });

    const parsed = parseExecutionProcessesWsCollectionMessage(raw);
    expect(parsed).toEqual({
      type: 'events',
      sequenceId: 4,
      events: [
        {
          type: 'snapshot',
          items: [
            {
              id: 'p1',
              attempt_id: 'a1',
              process_id: 123,
              worktree_path: '/tmp/worktree',
              branch_name: 'feature/demo',
              created_at: '2026-02-26T10:00:00.000Z',
            },
          ],
        },
      ],
    });
  });

  it('parses legacy upsert message', () => {
    const raw = JSON.stringify({
      type: 'upsert',
      process: {
        id: 'p2',
        attempt_id: 'a1',
        process_id: 456,
        worktree_path: '/tmp/worktree-2',
        branch_name: 'feature/demo-2',
        created_at: '2026-02-26T10:01:00.000Z',
      },
    });

    const parsed = parseExecutionProcessesWsCollectionMessage(raw);
    expect(parsed).toEqual({
      type: 'events',
      sequenceId: undefined,
      events: [
        {
          type: 'upsert',
          item: {
            id: 'p2',
            attempt_id: 'a1',
            process_id: 456,
            worktree_path: '/tmp/worktree-2',
            branch_name: 'feature/demo-2',
            created_at: '2026-02-26T10:01:00.000Z',
          },
        },
      ],
    });
  });

  it('parses gap_detected message', () => {
    const raw = JSON.stringify({
      type: 'gap_detected',
      requested_since_seq: 9,
      max_available_sequence_id: 2,
    });

    const parsed = parseExecutionProcessesWsCollectionMessage(raw);
    expect(parsed).toEqual({
      type: 'gap_detected',
      requestedSinceSeq: 9,
      maxAvailableSequenceId: 2,
    });
  });
});
