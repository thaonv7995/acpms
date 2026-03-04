import { describe, expect, it } from 'vitest';
import {
  classifySequenceAdvance,
  parseExecutionProcessLogStreamMessage,
} from '../../../hooks/useExecutionProcessLogsStream';

describe('parseExecutionProcessLogStreamMessage', () => {
  it('parses sequenced event message', () => {
    const raw = JSON.stringify({
      type: 'event',
      sequence_id: 5,
      event: {
        type: 'Log',
        attempt_id: 'attempt-1',
        log_type: 'normalized',
        content: '{"entry_type":{"type":"assistant_message"},"content":"ok"}',
        timestamp: '2026-02-26T10:00:00.000Z',
      },
    });

    expect(parseExecutionProcessLogStreamMessage(raw)).toEqual({
      type: 'event',
      sequenceId: 5,
      event: {
        type: 'Log',
        attempt_id: 'attempt-1',
        log_type: 'normalized',
        content: '{"entry_type":{"type":"assistant_message"},"content":"ok"}',
        timestamp: '2026-02-26T10:00:00.000Z',
      },
    });
  });

  it('parses gap_detected message', () => {
    const raw = JSON.stringify({
      type: 'gap_detected',
      requested_since_seq: 11,
      max_available_sequence_id: 3,
    });

    expect(parseExecutionProcessLogStreamMessage(raw)).toEqual({
      type: 'gap_detected',
      requestedSinceSeq: 11,
      maxAvailableSequenceId: 3,
    });
  });

  it('returns null for invalid payload', () => {
    expect(parseExecutionProcessLogStreamMessage('{"foo":"bar"}')).toBeNull();
  });
});

describe('classifySequenceAdvance', () => {
  it('accepts monotonic next sequence', () => {
    expect(classifySequenceAdvance(4, 5)).toBe('accept');
  });

  it('ignores duplicate or older sequence', () => {
    expect(classifySequenceAdvance(5, 5)).toBe('ignore');
    expect(classifySequenceAdvance(5, 4)).toBe('ignore');
  });

  it('detects gap when sequence jumps', () => {
    expect(classifySequenceAdvance(5, 8)).toBe('gap');
  });

  it('accepts first event when last sequence is zero', () => {
    expect(classifySequenceAdvance(0, 10)).toBe('accept');
  });
});
