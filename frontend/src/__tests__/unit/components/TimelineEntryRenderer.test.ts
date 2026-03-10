import { describe, expect, it } from 'vitest';
import { getToolDiffTargetId } from '@/components/timeline-log/TimelineEntryRenderer';
import type { ToolCallEntry } from '@/types/timeline-log';

function buildToolCallEntry(overrides: Partial<ToolCallEntry> = {}): ToolCallEntry {
  return {
    id: 'tool-1',
    type: 'tool_call',
    timestamp: '2026-03-10T10:00:00.000Z',
    toolName: 'Edit',
    actionType: {
      action: 'file_edit',
      path: 'src/App.tsx',
    },
    status: 'success',
    ...overrides,
  };
}

describe('getToolDiffTargetId', () => {
  it('prefers the real diff id when available', () => {
    expect(
      getToolDiffTargetId(buildToolCallEntry({ diffId: 'diff-123' }))
    ).toBe('diff-123');
  });

  it('falls back to a synthetic file target when only path is available', () => {
    expect(getToolDiffTargetId(buildToolCallEntry())).toBe('file:src/App.tsx');
  });

  it('returns null when no diff id or file path exists', () => {
    expect(
      getToolDiffTargetId(
        buildToolCallEntry({
          actionType: {
            action: 'command_run',
            command: 'npm test',
          },
        })
      )
    ).toBeNull();
  });
});
