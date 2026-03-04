import { describe, expect, it } from 'vitest';
import type { AgentLog } from '../../../api/taskAttempts';
import {
  extractLatestTokenUsageInfo,
  extractTokenUsageInfo,
} from '../../../hooks/useTimelineStream';

function buildLog(overrides: Partial<AgentLog>): AgentLog {
  return {
    id: overrides.id ?? 'log-1',
    attempt_id: overrides.attempt_id ?? 'attempt-1',
    log_type: overrides.log_type ?? 'normalized',
    content: overrides.content ?? '',
    created_at: overrides.created_at ?? '2026-02-27T00:00:00.000Z',
  };
}

describe('extractTokenUsageInfo', () => {
  it('extracts token usage from normalized token_usage_info entries', () => {
    const info = extractTokenUsageInfo(
      buildLog({
        content: JSON.stringify({
          timestamp: '2026-02-27T10:00:00.000Z',
          entry_type: {
            type: 'token_usage_info',
            input_tokens: 1200,
            output_tokens: 340,
            total_tokens: 1540,
          },
          content: '',
        }),
      })
    );

    expect(info).toEqual({
      inputTokens: 1200,
      outputTokens: 340,
      totalTokens: 1540,
      modelContextWindow: undefined,
    });
  });

  it('falls back to input + output when total_tokens is missing', () => {
    const info = extractTokenUsageInfo(
      buildLog({
        content: JSON.stringify({
          entry_type: {
            type: 'token_usage_info',
            input_tokens: 10,
            output_tokens: 5,
          },
          content: '',
        }),
      })
    );

    expect(info).toEqual({
      inputTokens: 10,
      outputTokens: 5,
      totalTokens: 15,
      modelContextWindow: undefined,
    });
  });

  it('returns null for non-token entries or malformed payloads', () => {
    expect(
      extractTokenUsageInfo(
        buildLog({
          content: JSON.stringify({
            entry_type: { type: 'assistant_message' },
            content: 'hello',
          }),
        })
      )
    ).toBeNull();

    expect(
      extractTokenUsageInfo(
        buildLog({
          log_type: 'stdout',
          content: 'plain text',
        })
      )
    ).toBeNull();

    expect(
      extractTokenUsageInfo(
        buildLog({
          content: '{invalid-json',
        })
      )
    ).toBeNull();
  });
});

describe('extractLatestTokenUsageInfo', () => {
  it('returns latest token usage from log stream', () => {
    const logs: AgentLog[] = [
      buildLog({
        id: 'a',
        created_at: '2026-02-27T10:00:00.000Z',
        content: JSON.stringify({
          entry_type: {
            type: 'token_usage_info',
            input_tokens: 10,
            output_tokens: 20,
            total_tokens: 30,
          },
          content: '',
        }),
      }),
      buildLog({
        id: 'b',
        created_at: '2026-02-27T10:00:01.000Z',
        content: JSON.stringify({
          entry_type: { type: 'assistant_message' },
          content: 'next',
        }),
      }),
      buildLog({
        id: 'c',
        created_at: '2026-02-27T10:00:02.000Z',
        content: JSON.stringify({
          entry_type: {
            type: 'token_usage_info',
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
          },
          content: '',
        }),
      }),
    ];

    expect(extractLatestTokenUsageInfo(logs)).toEqual({
      inputTokens: 100,
      outputTokens: 50,
      totalTokens: 150,
      modelContextWindow: undefined,
    });
  });
});
