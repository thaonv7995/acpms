import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import { useFollowUpSend } from '../../../hooks/useFollowUpSend';
import { sendAttemptInput } from '@/api/taskAttempts';
import { followUpExecutionProcess } from '@/api/executionProcesses';

vi.mock('@/api/taskAttempts', () => ({
  sendAttemptInput: vi.fn(),
}));

vi.mock('@/api/executionProcesses', () => ({
  followUpExecutionProcess: vi.fn(),
}));

describe('useFollowUpSend', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('sends live input when attempt is running', async () => {
    const cleanupSpy = vi.fn();
    vi.mocked(sendAttemptInput).mockResolvedValue(undefined as any);

    const { result } = renderHook(() =>
      useFollowUpSend({
        sessionId: 'attempt-1',
        isRunning: true,
        message: 'ship this change',
        retryProcessId: null,
        onAfterSendCleanup: cleanupSpy,
      })
    );

    await act(async () => {
      await result.current.onSendFollowUp();
    });

    expect(sendAttemptInput).toHaveBeenCalledWith('attempt-1', 'ship this change');
    expect(followUpExecutionProcess).not.toHaveBeenCalled();
    expect(cleanupSpy).toHaveBeenCalledTimes(1);
    expect(result.current.followUpError).toBeNull();
  });

  it('uses process-scoped follow-up when attempt is not running', async () => {
    const cleanupSpy = vi.fn();
    vi.mocked(followUpExecutionProcess).mockResolvedValue(undefined as any);

    const { result } = renderHook(() =>
      useFollowUpSend({
        sessionId: 'attempt-1',
        isRunning: false,
        message: 'continue from review feedback',
        retryProcessId: 'process-9',
        onAfterSendCleanup: cleanupSpy,
      })
    );

    await act(async () => {
      await result.current.onSendFollowUp();
    });

    expect(followUpExecutionProcess).toHaveBeenCalledWith('process-9', 'continue from review feedback');
    expect(sendAttemptInput).not.toHaveBeenCalled();
    expect(cleanupSpy).toHaveBeenCalledTimes(1);
    expect(result.current.followUpError).toBeNull();
  });

  it('returns explicit error when process context is missing in non-running state', async () => {
    const cleanupSpy = vi.fn();

    const { result } = renderHook(() =>
      useFollowUpSend({
        sessionId: 'attempt-1',
        isRunning: false,
        message: 'resume please',
        retryProcessId: null,
        onAfterSendCleanup: cleanupSpy,
      })
    );

    await act(async () => {
      await result.current.onSendFollowUp();
    });

    await waitFor(() => {
      expect(result.current.followUpError).toBe(
        'Execution process context is not ready yet. Please try again in a moment.'
      );
    });

    expect(sendAttemptInput).not.toHaveBeenCalled();
    expect(followUpExecutionProcess).not.toHaveBeenCalled();
    expect(cleanupSpy).not.toHaveBeenCalled();
  });
});
