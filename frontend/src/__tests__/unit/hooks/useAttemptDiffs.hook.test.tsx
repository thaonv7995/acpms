import { renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useAttemptDiffs } from '@/hooks/useAttemptDiffs';
import { getAttemptDiff, getAttemptDiffSummary } from '@/api/taskAttempts';

vi.mock('@/api/taskAttempts', () => ({
  getAttemptDiff: vi.fn(),
  getAttemptDiffSummary: vi.fn(),
}));

describe('useAttemptDiffs', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('uses only the live diff endpoint while polling is enabled', async () => {
    vi.mocked(getAttemptDiff).mockResolvedValue({
      files: [
        {
          change: 'modified',
          old_path: 'src/App.tsx',
          new_path: 'src/App.tsx',
          old_content: null,
          new_content: null,
          additions: 5,
          deletions: 2,
        },
      ],
      total_files: 1,
      total_additions: 5,
      total_deletions: 2,
    });
    vi.mocked(getAttemptDiffSummary).mockResolvedValue({ files: [] });

    const { result } = renderHook(() =>
      useAttemptDiffs('attempt-1', { enablePolling: true })
    );

    await waitFor(() => {
      expect(result.current.diffs).toHaveLength(1);
    });

    expect(getAttemptDiff).toHaveBeenCalledTimes(1);
    expect(getAttemptDiffSummary).not.toHaveBeenCalled();
  });
});
