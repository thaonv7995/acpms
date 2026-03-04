import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { TaskFollowUpSection } from '@/components/tasks-page/TaskFollowUpSection';
import { useFollowUpSend } from '@/hooks/useFollowUpSend';
import { useExecutionProcessReset } from '@/hooks/useExecutionProcessReset';

vi.mock('@/hooks/useFollowUpSend', () => ({
  useFollowUpSend: vi.fn(),
}));

vi.mock('@/hooks/useExecutionProcessReset', () => ({
  useExecutionProcessReset: vi.fn(),
}));

describe('TaskFollowUpSection reset UX', () => {
  beforeEach(() => {
    vi.clearAllMocks();

    vi.mocked(useFollowUpSend).mockReturnValue({
      isSendingFollowUp: false,
      followUpError: null,
      setFollowUpError: vi.fn(),
      onSendFollowUp: vi.fn().mockResolvedValue(undefined),
    });
  });

  it('triggers process reset with retry process context when reset button is clicked', () => {
    const resetProcess = vi.fn().mockResolvedValue(undefined);

    vi.mocked(useExecutionProcessReset).mockReturnValue({
      isResetting: false,
      resetError: null,
      resetInfo: null,
      requiresForceReset: false,
      resetProcess,
      clearResetState: vi.fn(),
    });

    render(
      <TaskFollowUpSection
        sessionId="attempt-1"
        isRunning={false}
        retryProcessId="process-1"
      />
    );

    const resetButton = screen.getByRole('button', { name: /reset/i });
    expect((resetButton as HTMLButtonElement).disabled).toBe(false);

    fireEvent.click(resetButton);
    expect(resetProcess).toHaveBeenCalledWith('process-1');
  });

  it('disables reset while agent is running', () => {
    vi.mocked(useExecutionProcessReset).mockReturnValue({
      isResetting: false,
      resetError: null,
      resetInfo: null,
      requiresForceReset: false,
      resetProcess: vi.fn().mockResolvedValue(undefined),
      clearResetState: vi.fn(),
    });

    render(
      <TaskFollowUpSection
        sessionId="attempt-2"
        isRunning={true}
        retryProcessId="process-2"
      />
    );

    const resetButton = screen.getByRole('button', { name: /reset/i });
    expect((resetButton as HTMLButtonElement).disabled).toBe(true);
  });

  it('renders force-reset state when hook reports dirty-worktree guard', () => {
    vi.mocked(useExecutionProcessReset).mockReturnValue({
      isResetting: false,
      resetError: 'Worktree has uncommitted changes. Click reset again to force a hard reset.',
      resetInfo: null,
      requiresForceReset: true,
      resetProcess: vi.fn().mockResolvedValue(undefined),
      clearResetState: vi.fn(),
    });

    render(
      <TaskFollowUpSection
        sessionId="attempt-3"
        isRunning={false}
        retryProcessId="process-3"
      />
    );

    expect(screen.getByRole('button', { name: /force reset/i })).toBeTruthy();
    expect(
      screen.getByText('Worktree has uncommitted changes. Click reset again to force a hard reset.')
    ).toBeTruthy();
  });

  it('does not render legacy extra toolbar buttons in follow-up form', () => {
    vi.mocked(useExecutionProcessReset).mockReturnValue({
      isResetting: false,
      resetError: null,
      resetInfo: null,
      requiresForceReset: false,
      resetProcess: vi.fn().mockResolvedValue(undefined),
      clearResetState: vi.fn(),
    });

    render(
      <TaskFollowUpSection
        sessionId="attempt-4"
        isRunning={false}
        retryProcessId="process-4"
        taskId="task-4"
        projectId="project-4"
      />
    );

    expect(screen.queryByText(/APPROVALS/i)).toBeNull();
    expect(screen.getByLabelText(/Attach reference files/i)).toBeTruthy();
    expect(screen.queryByLabelText(/Insert comment/i)).toBeNull();
    expect(screen.queryByLabelText(/Insert code block/i)).toBeNull();
  });

  it('disables upload button while attempt is running', () => {
    vi.mocked(useExecutionProcessReset).mockReturnValue({
      isResetting: false,
      resetError: null,
      resetInfo: null,
      requiresForceReset: false,
      resetProcess: vi.fn().mockResolvedValue(undefined),
      clearResetState: vi.fn(),
    });

    render(
      <TaskFollowUpSection
        sessionId="attempt-5"
        isRunning={true}
        retryProcessId="process-5"
        taskId="task-5"
        projectId="project-5"
      />
    );

    const attachButton = screen.getByLabelText(/Attach reference files/i) as HTMLButtonElement;
    expect(attachButton.disabled).toBe(true);
  });
});
