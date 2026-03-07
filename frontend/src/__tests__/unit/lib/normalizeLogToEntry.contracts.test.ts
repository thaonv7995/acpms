import { describe, expect, it } from 'vitest';
import { normalizeLogToEntry, type AgentLogLike } from '../../../lib/normalizeLogToEntry';

describe('normalizeLogToEntry internal .acpms contracts', () => {
  it('hides normalized file tool calls for ACPMS output contracts', () => {
    const log: AgentLogLike = {
      id: 'log-1',
      log_type: 'normalized',
      created_at: '2026-03-07T08:00:00Z',
      content: JSON.stringify({
        entry_type: {
          type: 'tool_use',
          tool_name: 'Write',
          action_type: {
            action: 'file_write',
            path: '/tmp/worktree/.acpms/preview-output.json',
          },
          status: { status: 'success' },
        },
        timestamp: '2026-03-07T08:00:00Z',
      }),
    };

    expect(normalizeLogToEntry(log, 0)).toEqual([]);
  });

  it('hides legacy file change rows for ACPMS output contracts', () => {
    const log: AgentLogLike = {
      id: 'log-2',
      log_type: 'file_change',
      created_at: '2026-03-07T08:01:00Z',
      content: JSON.stringify({
        path: '.acpms/init-output.json',
        change_type: 'Modified',
        lines_added: 1,
        lines_removed: 0,
      }),
    };

    expect(normalizeLogToEntry(log, 0)).toEqual([]);
  });

  it('keeps normal source file changes visible', () => {
    const log: AgentLogLike = {
      id: 'log-3',
      log_type: 'file_change',
      created_at: '2026-03-07T08:02:00Z',
      content: JSON.stringify({
        path: 'src/App.tsx',
        change_type: 'Modified',
        lines_added: 12,
        lines_removed: 4,
      }),
    };

    expect(normalizeLogToEntry(log, 0)).toEqual([
      expect.objectContaining({
        type: 'file_change',
        path: 'src/App.tsx',
        linesAdded: 12,
        linesRemoved: 4,
      }),
    ]);
  });
});
