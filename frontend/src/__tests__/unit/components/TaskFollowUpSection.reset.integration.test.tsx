import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { TaskFollowUpSection } from '@/components/tasks-page/TaskFollowUpSection';
import { useFollowUpSend } from '@/hooks/useFollowUpSend';
import { resetExecutionProcess } from '@/api/executionProcesses';

vi.mock('@/hooks/useFollowUpSend', () => ({
  useFollowUpSend: vi.fn(),
}));

vi.mock('@/api/executionProcesses', () => ({
  resetExecutionProcess: vi.fn(),
}));

describe('TaskFollowUpSection reset integration flow', () => {
  beforeEach(() => {
    vi.clearAllMocks();

    vi.mocked(useFollowUpSend).mockReturnValue({
      isSendingFollowUp: false,
      followUpError: null,
      setFollowUpError: vi.fn(),
      onSendFollowUp: vi.fn().mockResolvedValue(undefined),
    });
  });

  it('supports reset -> force reset workflow when backend reports dirty guard', async () => {
    vi.mocked(resetExecutionProcess)
      .mockRejectedValueOnce(
        new Error('Worktree has uncommitted changes. Set force_when_dirty=true to continue reset.')
      )
      .mockResolvedValueOnce({
        process_id: 'process-9',
        worktree_path: '/tmp/worktree',
        git_reset_applied: true,
        worktree_was_dirty: true,
        force_when_dirty: true,
        requested_by_user_id: 'user-1',
        requested_at: '2026-02-27T11:00:00.000Z',
      } as any);

    render(
      <TaskFollowUpSection
        sessionId="attempt-9"
        isRunning={false}
        retryProcessId="process-9"
      />
    );

    fireEvent.click(screen.getByRole('button', { name: /reset/i }));

    await waitFor(() => {
      expect(
        screen.getByText('Worktree has uncommitted changes. Click reset again to force a hard reset.')
      ).toBeTruthy();
      expect(screen.getByRole('button', { name: /force reset/i })).toBeTruthy();
    });

    fireEvent.click(screen.getByRole('button', { name: /force reset/i }));

    await waitFor(() => {
      expect(
        screen.getByText('Execution process reset completed. Uncommitted changes were discarded.')
      ).toBeTruthy();
    });

    expect(resetExecutionProcess).toHaveBeenNthCalledWith(1, 'process-9', {
      perform_git_reset: true,
      force_when_dirty: false,
    });
    expect(resetExecutionProcess).toHaveBeenNthCalledWith(2, 'process-9', {
      perform_git_reset: true,
      force_when_dirty: true,
    });
  });
});
