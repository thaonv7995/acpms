import React from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { getPendingApprovalsForProcess } from '@/api/approvals';
import { useApprovalsStream } from '../../../hooks/useApprovalsStream';
import {
  isWsCollectionStreamEnabled,
  useWsCollectionStream,
} from '../../../hooks/useWsCollectionStream';

vi.mock('@/api/approvals', () => ({
  getPendingApprovalsForProcess: vi.fn(),
}));

vi.mock('../../../hooks/useWsCollectionStream', () => ({
  isWsCollectionStreamEnabled: vi.fn(() => false),
  useWsCollectionStream: vi.fn(() => ({
    items: null,
    isStreaming: false,
    error: null,
    reconnect: vi.fn(),
    lastSequenceId: 0,
  })),
}));

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
      },
    },
  });

  return function Wrapper({ children }: { children: React.ReactNode }) {
    return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
  };
}

describe('useApprovalsStream fallback mode', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('returns empty approvals when execution process id is not available yet', async () => {
    const { result } = renderHook(() => useApprovalsStream({}), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(result.current.approvals).toEqual([]);
      expect(result.current.pendingApprovals).toEqual([]);
      expect(result.current.isLoading).toBe(false);
    });

    expect(getPendingApprovalsForProcess).not.toHaveBeenCalled();
    expect(result.current.isStreaming).toBe(false);
  });

  it('prioritizes process-scoped REST endpoint when process id is available', async () => {
    vi.mocked(getPendingApprovalsForProcess).mockResolvedValue([
      {
        id: 'approval-2',
        attempt_id: 'attempt-1',
        execution_process_id: 'process-9',
        tool_use_id: 'tool-2',
        tool_name: 'Read',
        status: 'approved',
        created_at: '2026-02-26T10:01:00.000Z',
      },
    ] as any);

    const { result } = renderHook(
      () => useApprovalsStream({ executionProcessId: 'process-9' }),
      {
        wrapper: createWrapper(),
      }
    );

    await waitFor(() => {
      expect(result.current.approvals).toHaveLength(1);
      expect(result.current.pendingApprovals).toHaveLength(0);
    });

    expect(getPendingApprovalsForProcess).toHaveBeenCalledWith('process-9');
  });

  it('prefers WS stream items over REST snapshot when WS mode is enabled', async () => {
    vi.mocked(isWsCollectionStreamEnabled).mockReturnValue(true);
    vi.mocked(getPendingApprovalsForProcess).mockResolvedValue([
      {
        id: 'approval-rest',
        attempt_id: 'attempt-1',
        execution_process_id: 'process-9',
        tool_use_id: 'tool-rest',
        tool_name: 'Bash',
        status: 'pending',
        created_at: '2026-02-26T10:00:00.000Z',
      },
    ] as any);
    vi.mocked(useWsCollectionStream).mockReturnValue({
      items: [
        {
          id: 'approval-stream-approved',
          attempt_id: 'attempt-1',
          execution_process_id: 'process-9',
          tool_use_id: 'tool-stream-1',
          tool_name: 'Read',
          status: 'approved',
          created_at: '2026-02-26T10:02:00.000Z',
          responded_at: '2026-02-26T10:02:05.000Z',
        },
        {
          id: 'approval-stream-pending',
          attempt_id: 'attempt-1',
          execution_process_id: 'process-9',
          tool_use_id: 'tool-stream-2',
          tool_name: 'Bash',
          status: 'pending',
          created_at: '2026-02-26T10:03:00.000Z',
          responded_at: null,
        },
      ],
      isStreaming: true,
      error: null,
      reconnect: vi.fn(),
      lastSequenceId: 12,
    } as any);

    const { result } = renderHook(
      () => useApprovalsStream({ executionProcessId: 'process-9' }),
      {
        wrapper: createWrapper(),
      }
    );

    await waitFor(() => {
      expect(result.current.approvals.map((approval) => approval.id)).toEqual([
        'approval-stream-approved',
        'approval-stream-pending',
      ]);
      expect(result.current.pendingApprovals.map((approval) => approval.id)).toEqual([
        'approval-stream-pending',
      ]);
      expect(result.current.isStreaming).toBe(true);
    });

    expect(getPendingApprovalsForProcess).toHaveBeenCalledWith('process-9');
  });
});
