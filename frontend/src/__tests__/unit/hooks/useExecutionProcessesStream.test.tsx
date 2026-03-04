import React from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { getExecutionProcesses } from '@/api/executionProcesses';
import { useExecutionProcessesStream } from '../../../hooks/useExecutionProcessesStream';
import {
  isWsCollectionStreamEnabled,
  useWsCollectionStream,
} from '../../../hooks/useWsCollectionStream';

vi.mock('@/api/executionProcesses', () => ({
  getExecutionProcesses: vi.fn(),
}));

vi.mock('../../../hooks/useWsCollectionStream', () => ({
  isWsCollectionStreamEnabled: vi.fn(),
  useWsCollectionStream: vi.fn(),
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

describe('useExecutionProcessesStream', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(useWsCollectionStream).mockReturnValue({
      items: null,
      isStreaming: false,
      error: null,
      reconnect: vi.fn(),
      lastSequenceId: 0,
    } as any);
  });

  it('uses REST fallback and sorts by created_at when WS is disabled', async () => {
    vi.mocked(isWsCollectionStreamEnabled).mockReturnValue(false);
    vi.mocked(getExecutionProcesses).mockResolvedValue([
      {
        id: 'process-2',
        attempt_id: 'attempt-1',
        process_id: 2,
        worktree_path: '/tmp/p2',
        branch_name: 'b2',
        created_at: '2026-02-26T10:01:00.000Z',
      },
      {
        id: 'process-1',
        attempt_id: 'attempt-1',
        process_id: 1,
        worktree_path: '/tmp/p1',
        branch_name: 'b1',
        created_at: '2026-02-26T10:00:00.000Z',
      },
    ] as any);

    const { result } = renderHook(() => useExecutionProcessesStream('attempt-1'), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(result.current.processes.map((process) => process.id)).toEqual([
        'process-1',
        'process-2',
      ]);
    });

    expect(getExecutionProcesses).toHaveBeenCalledWith('attempt-1');
    expect(result.current.isStreaming).toBe(false);
  });

  it('prefers stream items when WS stream has live data', async () => {
    vi.mocked(isWsCollectionStreamEnabled).mockReturnValue(true);
    vi.mocked(getExecutionProcesses).mockResolvedValue([
      {
        id: 'process-from-rest',
        attempt_id: 'attempt-1',
        process_id: 1,
        worktree_path: '/tmp/rest',
        branch_name: 'rest',
        created_at: '2026-02-26T10:00:00.000Z',
      },
    ] as any);

    vi.mocked(useWsCollectionStream).mockReturnValue({
      items: [
        {
          id: 'process-from-stream',
          attempt_id: 'attempt-1',
          process_id: 3,
          worktree_path: '/tmp/stream',
          branch_name: 'stream',
          created_at: '2026-02-26T10:02:00.000Z',
        },
      ],
      isStreaming: true,
      error: null,
      reconnect: vi.fn(),
      lastSequenceId: 8,
    } as any);

    const { result } = renderHook(() => useExecutionProcessesStream('attempt-1'), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(result.current.processes.map((process) => process.id)).toEqual([
        'process-from-stream',
      ]);
    });

    expect(result.current.isStreaming).toBe(true);
  });
});
