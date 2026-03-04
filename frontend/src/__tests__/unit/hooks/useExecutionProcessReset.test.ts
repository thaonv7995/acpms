import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import {
  buildResetSuccessMessage,
  isForceResetRequiredError,
  useExecutionProcessReset,
} from '../../../hooks/useExecutionProcessReset';
import { resetExecutionProcess } from '@/api/executionProcesses';

vi.mock('@/api/executionProcesses', () => ({
  resetExecutionProcess: vi.fn(),
}));

describe('useExecutionProcessReset', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('marks force reset required when API returns dirty worktree guard error', async () => {
    vi.mocked(resetExecutionProcess).mockRejectedValue(
      new Error('Worktree has uncommitted changes. Set force_when_dirty=true to continue reset.')
    );

    const { result } = renderHook(() => useExecutionProcessReset());

    await act(async () => {
      await result.current.resetProcess('process-1');
    });

    expect(resetExecutionProcess).toHaveBeenCalledWith('process-1', {
      perform_git_reset: true,
      force_when_dirty: false,
    });
    expect(result.current.requiresForceReset).toBe(true);
    expect(result.current.resetError).toBe(
      'Worktree has uncommitted changes. Click reset again to force a hard reset.'
    );
  });

  it('retries with force_when_dirty after force flag is set', async () => {
    vi.mocked(resetExecutionProcess)
      .mockRejectedValueOnce(
        new Error('Worktree has uncommitted changes. Set force_when_dirty=true to continue reset.')
      )
      .mockResolvedValueOnce({
        process_id: 'process-1',
        worktree_path: '/tmp/worktree',
        git_reset_applied: true,
        worktree_was_dirty: true,
        force_when_dirty: true,
        requested_by_user_id: 'user-1',
        requested_at: '2026-02-27T10:40:00.000Z',
      } as any);

    const { result } = renderHook(() => useExecutionProcessReset());

    await act(async () => {
      await result.current.resetProcess('process-1');
    });

    await act(async () => {
      await result.current.resetProcess('process-1');
    });

    expect(resetExecutionProcess).toHaveBeenNthCalledWith(1, 'process-1', {
      perform_git_reset: true,
      force_when_dirty: false,
    });
    expect(resetExecutionProcess).toHaveBeenNthCalledWith(2, 'process-1', {
      perform_git_reset: true,
      force_when_dirty: true,
    });
    await waitFor(() => {
      expect(result.current.resetInfo).toBe(
        'Execution process reset completed. Uncommitted changes were discarded.'
      );
      expect(result.current.resetError).toBeNull();
      expect(result.current.requiresForceReset).toBe(false);
    });
  });
});

describe('useExecutionProcessReset helpers', () => {
  it('detects force reset hint in backend error message', () => {
    expect(
      isForceResetRequiredError(
        'Worktree has uncommitted changes. Set force_when_dirty=true to continue reset.'
      )
    ).toBe(true);
    expect(isForceResetRequiredError('random error')).toBe(false);
  });

  it('builds success message variants from reset response', () => {
    expect(
      buildResetSuccessMessage({
        process_id: 'p1',
        worktree_path: '/tmp/w',
        git_reset_applied: false,
        worktree_was_dirty: false,
        force_when_dirty: false,
        requested_by_user_id: 'user-1',
        requested_at: '2026-02-27T10:40:00.000Z',
      })
    ).toBe('Execution process reset acknowledged.');

    expect(
      buildResetSuccessMessage({
        process_id: 'p1',
        worktree_path: '/tmp/w',
        git_reset_applied: true,
        worktree_was_dirty: false,
        force_when_dirty: false,
        requested_by_user_id: 'user-1',
        requested_at: '2026-02-27T10:40:00.000Z',
      })
    ).toBe('Execution process reset completed.');
  });
});
