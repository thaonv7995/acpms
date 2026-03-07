import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { PreviewPanelWrapper } from '@/pages/project-tasks/preview-panel-wrapper';
import { useDevServer } from '@/hooks/useDevServer';
import { useExecutionProcessesStream } from '@/hooks/useExecutionProcessesStream';
import { followUpExecutionProcess } from '@/api/executionProcesses';
import { sendAttemptInput } from '@/api/taskAttempts';

vi.mock('@/hooks/useDevServer', () => ({
  useDevServer: vi.fn(),
}));

vi.mock('@/hooks/useExecutionProcessesStream', () => ({
  useExecutionProcessesStream: vi.fn(),
}));

vi.mock('@/api/executionProcesses', () => ({
  followUpExecutionProcess: vi.fn(),
}));

vi.mock('@/api/taskAttempts', () => ({
  sendAttemptInput: vi.fn(),
}));

describe('PreviewPanelWrapper', () => {
  beforeEach(() => {
    vi.clearAllMocks();

    vi.mocked(useDevServer).mockReturnValue({
      status: 'idle',
      url: undefined,
      errorMessage: undefined,
      startServer: vi.fn().mockResolvedValue(undefined),
      stopServer: vi.fn().mockResolvedValue(undefined),
      dismissPreview: vi.fn(),
      restartServer: vi.fn().mockResolvedValue(undefined),
      isLoading: false,
      startDisabled: true,
      startDisabledReason: 'Preview unavailable: Docker preview runtime is disabled',
      externalPreview: false,
      previewRevision: 0,
      canStopPreview: false,
      dismissOnly: false,
    });

    vi.mocked(useExecutionProcessesStream).mockReturnValue({
      processes: [
        {
          id: 'process-1',
          attempt_id: 'attempt-1',
          process_id: 123,
          worktree_path: null,
          branch_name: null,
          created_at: '2026-03-07T00:00:00.000Z',
        },
      ],
      isLoading: false,
      isStreaming: false,
      error: null,
      reconnect: vi.fn(),
      refetch: vi.fn(),
    });
  });

  it('requests an agent preview follow-up when managed docker preview is disabled', async () => {
    const onFollowUpAttemptCreated = vi.fn();

    vi.mocked(followUpExecutionProcess).mockResolvedValue({
      id: 'attempt-2',
      task_id: 'task-1',
      status: 'QUEUED',
      started_at: null,
      completed_at: null,
      error_message: null,
      metadata: {},
      created_at: new Date().toISOString(),
    } as never);

    render(
      <PreviewPanelWrapper
        taskId="task-1"
        attemptId="attempt-1"
        attemptStatus="SUCCESS"
        onFollowUpAttemptCreated={onFollowUpAttemptCreated}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: /request agent preview/i }));

    await waitFor(() => {
      expect(followUpExecutionProcess).toHaveBeenCalledWith(
        'process-1',
        expect.stringContaining('Deploy a preview for the latest code in this attempt.')
      );
    });

    expect(onFollowUpAttemptCreated).toHaveBeenCalledWith('attempt-2');
    expect(sendAttemptInput).not.toHaveBeenCalled();
  });

  it('shows stop and requests an agent follow-up to stop preview when a preview URL exists', async () => {
    vi.mocked(useDevServer).mockReturnValue({
      status: 'running',
      url: 'http://localhost:42574',
      errorMessage: undefined,
      startServer: vi.fn().mockResolvedValue(undefined),
      stopServer: vi.fn().mockResolvedValue(undefined),
      dismissPreview: vi.fn(),
      restartServer: vi.fn().mockResolvedValue(undefined),
      isLoading: false,
      startDisabled: false,
      startDisabledReason: undefined,
      externalPreview: false,
      previewRevision: 0,
      canStopPreview: false,
      dismissOnly: false,
    });

    vi.mocked(sendAttemptInput).mockResolvedValue(undefined as never);

    render(
      <PreviewPanelWrapper
        taskId="task-1"
        attemptId="attempt-1"
        attemptStatus="RUNNING"
      />
    );

    fireEvent.click(screen.getByRole('button', { name: /stop preview/i }));

    await waitFor(() => {
      expect(sendAttemptInput).toHaveBeenCalledWith(
        'attempt-1',
        expect.stringContaining('Stop the preview that is currently associated with this attempt.')
      );
    });
  });
});
