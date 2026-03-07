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
      startServer: vi.fn().mockResolvedValue(false),
      stopServer: vi.fn().mockResolvedValue(true),
      dismissPreview: vi.fn(),
      restartServer: vi.fn().mockResolvedValue(false),
      isLoading: false,
      startDisabled: true,
      startDisabledReason: 'Preview unavailable: Docker preview runtime is disabled',
      externalPreview: false,
      previewRevision: 0,
      canStopPreview: false,
      dismissOnly: false,
      cloudflareReady: true,
      missingCloudflareFields: [],
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

  it('falls back to an agent preview follow-up when the hard preview start flow fails', async () => {
    const onFollowUpAttemptCreated = vi.fn();
    const startServer = vi.fn().mockResolvedValue(false);

    vi.mocked(useDevServer).mockReturnValue({
      status: 'idle',
      url: undefined,
      errorMessage: undefined,
      startServer,
      stopServer: vi.fn().mockResolvedValue(true),
      dismissPreview: vi.fn(),
      restartServer: vi.fn().mockResolvedValue(false),
      isLoading: false,
      startDisabled: true,
      startDisabledReason: 'Preview unavailable: Docker preview runtime is disabled',
      externalPreview: false,
      previewRevision: 0,
      canStopPreview: false,
      dismissOnly: false,
      cloudflareReady: true,
      missingCloudflareFields: [],
    });

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

    fireEvent.click(screen.getByRole('button', { name: /start preview/i }));

    await waitFor(() => {
      expect(startServer).toHaveBeenCalled();
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
      startServer: vi.fn().mockResolvedValue(true),
      stopServer: vi.fn().mockResolvedValue(false),
      dismissPreview: vi.fn(),
      restartServer: vi.fn().mockResolvedValue(false),
      isLoading: false,
      startDisabled: false,
      startDisabledReason: undefined,
      externalPreview: false,
      previewRevision: 0,
      canStopPreview: false,
      dismissOnly: false,
      cloudflareReady: true,
      missingCloudflareFields: [],
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

  it('does not create a follow-up when the hard preview start flow succeeds', async () => {
    const startServer = vi.fn().mockResolvedValue(true);

    vi.mocked(useDevServer).mockReturnValue({
      status: 'idle',
      url: undefined,
      errorMessage: undefined,
      startServer,
      stopServer: vi.fn().mockResolvedValue(true),
      dismissPreview: vi.fn(),
      restartServer: vi.fn().mockResolvedValue(true),
      isLoading: false,
      startDisabled: false,
      startDisabledReason: undefined,
      externalPreview: false,
      previewRevision: 0,
      canStopPreview: false,
      dismissOnly: false,
      cloudflareReady: true,
      missingCloudflareFields: [],
    });

    render(
      <PreviewPanelWrapper
        taskId="task-1"
        attemptId="attempt-1"
        attemptStatus="RUNNING"
      />
    );

    fireEvent.click(screen.getByRole('button', { name: /start preview/i }));

    await waitFor(() => {
      expect(startServer).toHaveBeenCalled();
    });

    expect(sendAttemptInput).not.toHaveBeenCalled();
    expect(followUpExecutionProcess).not.toHaveBeenCalled();
  });

  it('does not send an agent stop follow-up when the hard stop flow succeeds', async () => {
    const stopServer = vi.fn().mockResolvedValue(true);

    vi.mocked(useDevServer).mockReturnValue({
      status: 'running',
      url: 'http://localhost:42574',
      errorMessage: undefined,
      startServer: vi.fn().mockResolvedValue(true),
      stopServer,
      dismissPreview: vi.fn(),
      restartServer: vi.fn().mockResolvedValue(true),
      isLoading: false,
      startDisabled: false,
      startDisabledReason: undefined,
      externalPreview: false,
      previewRevision: 0,
      canStopPreview: true,
      dismissOnly: false,
      cloudflareReady: true,
      missingCloudflareFields: [],
    });

    render(
      <PreviewPanelWrapper
        taskId="task-1"
        attemptId="attempt-1"
        attemptStatus="RUNNING"
      />
    );

    fireEvent.click(screen.getByRole('button', { name: /stop preview/i }));

    await waitFor(() => {
      expect(stopServer).toHaveBeenCalled();
    });

    expect(sendAttemptInput).not.toHaveBeenCalled();
    expect(followUpExecutionProcess).not.toHaveBeenCalled();
  });

  it('shows a public-url-unavailable badge when preview is local and cloudflare settings are missing', () => {
    vi.mocked(useDevServer).mockReturnValue({
      status: 'running',
      url: 'http://127.0.0.1:4174',
      errorMessage: undefined,
      startServer: vi.fn().mockResolvedValue(true),
      stopServer: vi.fn().mockResolvedValue(true),
      dismissPreview: vi.fn(),
      restartServer: vi.fn().mockResolvedValue(true),
      isLoading: false,
      startDisabled: false,
      startDisabledReason: undefined,
      externalPreview: false,
      previewRevision: 0,
      canStopPreview: true,
      dismissOnly: false,
      cloudflareReady: false,
      missingCloudflareFields: ['cloudflare_account_id', 'cloudflare_api_token'],
    });

    render(
      <PreviewPanelWrapper
        taskId="task-1"
        attemptId="attempt-1"
        attemptStatus="RUNNING"
      />
    );

    expect(screen.getByText('Public URL unavailable')).toBeTruthy();
  });
});
