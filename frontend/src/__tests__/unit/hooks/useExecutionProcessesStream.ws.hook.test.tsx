import React from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { getExecutionProcesses } from '@/api/executionProcesses';
import { getAccessToken } from '@/api/client';
import { useExecutionProcessesStream } from '../../../hooks/useExecutionProcessesStream';

vi.mock('@/api/executionProcesses', () => ({
  getExecutionProcesses: vi.fn(),
}));

vi.mock('@/api/client', () => ({
  API_PREFIX: '/api/v1',
  getAccessToken: vi.fn(),
}));

type StreamMessage =
  | {
      sequence_id: number;
      message: {
        type: 'snapshot';
        processes: Array<{
          id: string;
          attempt_id: string;
          process_id: number;
          worktree_path: string;
          branch_name: string;
          created_at: string;
        }>;
      };
    }
  | {
      sequence_id: number;
      message: {
        type: 'upsert';
        process: {
          id: string;
          attempt_id: string;
          process_id: number;
          worktree_path: string;
          branch_name: string;
          created_at: string;
        };
      };
    }
  | {
      sequence_id: number;
      message: {
        type: 'remove';
        process_id: string;
      };
    }
  | {
      type: 'gap_detected';
      requested_since_seq: number;
      max_available_sequence_id: number;
    };

class MockWebSocket {
  static instances: MockWebSocket[] = [];

  readonly url: string;
  readonly protocols: string[];

  onopen: ((event: Event) => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;
  onclose: ((event: CloseEvent) => void) | null = null;

  close = vi.fn(() => {
    this.onclose?.({} as CloseEvent);
  });

  constructor(url: string, protocols?: string | string[]) {
    this.url = url;
    if (Array.isArray(protocols)) {
      this.protocols = protocols;
    } else if (typeof protocols === 'string') {
      this.protocols = [protocols];
    } else {
      this.protocols = [];
    }
    MockWebSocket.instances.push(this);
  }

  emitOpen() {
    this.onopen?.(new Event('open'));
  }

  emit(message: StreamMessage) {
    this.onmessage?.({ data: JSON.stringify(message) } as MessageEvent);
  }
}

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

describe('useExecutionProcessesStream with real ws collection stream hook', () => {
  beforeEach(() => {
    MockWebSocket.instances = [];
    vi.stubGlobal('WebSocket', MockWebSocket as unknown as typeof WebSocket);
    vi.clearAllMocks();
    vi.mocked(getAccessToken).mockReturnValue('token-processes');
    vi.mocked(getExecutionProcesses).mockResolvedValue([] as any);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.clearAllMocks();
  });

  it('applies sequenced events and reconnects with since_seq cursor', async () => {
    const { result } = renderHook(() => useExecutionProcessesStream('attempt-1'), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(MockWebSocket.instances.length).toBeGreaterThan(0);
    });

    const initialCount = MockWebSocket.instances.length;
    const firstWs = MockWebSocket.instances[initialCount - 1];
    expect(firstWs.protocols).toEqual(['acpms-bearer', 'token-processes']);
    expect(firstWs.url).toContain('session_id=attempt-1');

    act(() => {
      firstWs.emitOpen();
      firstWs.emit({
        sequence_id: 1,
        message: {
          type: 'snapshot',
          processes: [
            {
              id: 'p1',
              attempt_id: 'attempt-1',
              process_id: 1,
              worktree_path: '/tmp/p1',
              branch_name: 'b1',
              created_at: '2026-02-26T10:00:00.000Z',
            },
          ],
        },
      });
      firstWs.emit({
        sequence_id: 2,
        message: {
          type: 'upsert',
          process: {
            id: 'p2',
            attempt_id: 'attempt-1',
            process_id: 2,
            worktree_path: '/tmp/p2',
            branch_name: 'b2',
            created_at: '2026-02-26T10:01:00.000Z',
          },
        },
      });
      firstWs.emit({
        sequence_id: 3,
        message: {
          type: 'remove',
          process_id: 'p1',
        },
      });
    });

    await waitFor(() => {
      expect(result.current.processes.map((process) => process.id)).toEqual(['p2']);
    });

    act(() => {
      result.current.reconnect();
    });

    await waitFor(() => {
      expect(MockWebSocket.instances.length).toBeGreaterThan(initialCount);
    });

    const reconnectWs = MockWebSocket.instances[MockWebSocket.instances.length - 1];
    expect(reconnectWs.url).toContain('since_seq=3');
  });

  it('resets and reconnects from snapshot when stream emits gap_detected', async () => {
    const { result } = renderHook(() => useExecutionProcessesStream('attempt-2'), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(MockWebSocket.instances.length).toBeGreaterThan(0);
    });

    const initialCount = MockWebSocket.instances.length;
    const firstWs = MockWebSocket.instances[initialCount - 1];

    act(() => {
      firstWs.emitOpen();
      firstWs.emit({
        sequence_id: 4,
        message: {
          type: 'snapshot',
          processes: [
            {
              id: 'p-gap',
              attempt_id: 'attempt-2',
              process_id: 9,
              worktree_path: '/tmp/p-gap',
              branch_name: 'b-gap',
              created_at: '2026-02-26T10:00:00.000Z',
            },
          ],
        },
      });
    });

    await waitFor(() => {
      expect(result.current.processes.map((process) => process.id)).toEqual(['p-gap']);
    });

    act(() => {
      firstWs.emit({
        type: 'gap_detected',
        requested_since_seq: 99,
        max_available_sequence_id: 7,
      });
    });

    await waitFor(() => {
      expect(result.current.error).toContain('Stream gap detected');
      expect(MockWebSocket.instances.length).toBeGreaterThan(initialCount);
    });

    const reconnectWs = MockWebSocket.instances[MockWebSocket.instances.length - 1];
    expect(reconnectWs.url).not.toContain('since_seq=');
  });

  it('accepts reconnect snapshot with higher persisted sequence and advances cursor', async () => {
    const { result } = renderHook(() => useExecutionProcessesStream('attempt-3'), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(MockWebSocket.instances.length).toBeGreaterThan(0);
    });

    const firstWs = MockWebSocket.instances[MockWebSocket.instances.length - 1];

    act(() => {
      firstWs.emitOpen();
      firstWs.emit({
        sequence_id: 1,
        message: {
          type: 'snapshot',
          processes: [
            {
              id: 'p-initial',
              attempt_id: 'attempt-3',
              process_id: 1,
              worktree_path: '/tmp/p-initial',
              branch_name: 'b-initial',
              created_at: '2026-02-26T10:00:00.000Z',
            },
          ],
        },
      });
    });

    await waitFor(() => {
      expect(result.current.processes.map((process) => process.id)).toEqual(['p-initial']);
    });

    act(() => {
      result.current.reconnect();
    });

    await waitFor(() => {
      expect(MockWebSocket.instances.length).toBeGreaterThan(1);
    });

    const reconnectWs = MockWebSocket.instances[MockWebSocket.instances.length - 1];
    expect(reconnectWs.url).toContain('since_seq=1');

    act(() => {
      reconnectWs.emitOpen();
      reconnectWs.emit({
        sequence_id: 10,
        message: {
          type: 'snapshot',
          processes: [
            {
              id: 'p-rebased',
              attempt_id: 'attempt-3',
              process_id: 2,
              worktree_path: '/tmp/p-rebased',
              branch_name: 'b-rebased',
              created_at: '2026-02-26T10:01:00.000Z',
            },
          ],
        },
      });
      reconnectWs.emit({
        sequence_id: 11,
        message: {
          type: 'upsert',
          process: {
            id: 'p-live',
            attempt_id: 'attempt-3',
            process_id: 3,
            worktree_path: '/tmp/p-live',
            branch_name: 'b-live',
            created_at: '2026-02-26T10:02:00.000Z',
          },
        },
      });
    });

    await waitFor(() => {
      expect(result.current.processes.map((process) => process.id)).toEqual(['p-rebased', 'p-live']);
    });

    act(() => {
      result.current.reconnect();
    });

    await waitFor(() => {
      expect(MockWebSocket.instances.length).toBeGreaterThan(2);
    });

    const thirdWs = MockWebSocket.instances[MockWebSocket.instances.length - 1];
    expect(thirdWs.url).toContain('since_seq=11');
  });
});
