import React from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { getPendingApprovalsForProcess } from '@/api/approvals';
import { useApprovalsStream } from '../../../hooks/useApprovalsStream';
import { getAccessToken } from '@/api/client';

vi.mock('@/api/approvals', () => ({
  getPendingApprovalsForProcess: vi.fn(),
}));

vi.mock('@/api/client', () => ({
  API_PREFIX: '/api/v1',
  getAccessToken: vi.fn(),
}));

type StreamMessage =
  | {
      type: 'snapshot';
      sequence_id: number;
      data: {
        approvals: Record<
          string,
          {
            id: string;
            attempt_id: string;
            execution_process_id: string | null;
            tool_use_id: string;
            tool_name: string;
            status: 'pending' | 'approved' | 'denied' | 'timed_out';
            created_at: string;
            responded_at: string | null;
          }
        >;
      };
    }
  | {
      type: 'patch';
      sequence_id: number;
      operations: Array<{
        op: 'add' | 'replace' | 'remove';
        path: string;
        value?: {
          id: string;
          attempt_id: string;
          execution_process_id: string | null;
          tool_use_id: string;
          tool_name: string;
          status: 'pending' | 'approved' | 'denied' | 'timed_out';
          created_at: string;
          responded_at: string | null;
        };
      }>;
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

describe('useApprovalsStream with real ws collection stream hook', () => {
  beforeEach(() => {
    MockWebSocket.instances = [];
    vi.stubGlobal('WebSocket', MockWebSocket as unknown as typeof WebSocket);
    vi.clearAllMocks();
    vi.mocked(getAccessToken).mockReturnValue('token-approvals');
    vi.mocked(getPendingApprovalsForProcess).mockResolvedValue([]);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.clearAllMocks();
  });

  it('applies patch snapshot/operations and reconnects with since_seq cursor', async () => {
    const { result } = renderHook(
      () => useApprovalsStream({ executionProcessId: 'process-1' }),
      {
        wrapper: createWrapper(),
      }
    );

    await waitFor(() => {
      expect(MockWebSocket.instances.length).toBeGreaterThan(0);
    });

    const initialCount = MockWebSocket.instances.length;
    const firstWs = MockWebSocket.instances[initialCount - 1];
    expect(firstWs.protocols).toEqual(['acpms-bearer', 'token-approvals']);
    expect(firstWs.url).toContain('/api/v1/approvals/stream/ws?execution_process_id=process-1');
    expect(firstWs.url).toContain('projection=patch');

    act(() => {
      firstWs.emitOpen();
      firstWs.emit({
        type: 'snapshot',
        sequence_id: 1,
        data: {
          approvals: {
            a2: {
              id: 'a2',
              attempt_id: 'attempt-1',
              execution_process_id: 'process-1',
              tool_use_id: 'tool-2',
              tool_name: 'Bash',
              status: 'pending',
              created_at: '2026-02-26T10:01:00.000Z',
              responded_at: null,
            },
            a1: {
              id: 'a1',
              attempt_id: 'attempt-1',
              execution_process_id: 'process-1',
              tool_use_id: 'tool-1',
              tool_name: 'Read',
              status: 'approved',
              created_at: '2026-02-26T10:00:00.000Z',
              responded_at: '2026-02-26T10:00:03.000Z',
            },
          },
        },
      });
      firstWs.emit({
        type: 'patch',
        sequence_id: 2,
        operations: [
          {
            op: 'replace',
            path: '/approvals/a2',
            value: {
              id: 'a2',
              attempt_id: 'attempt-1',
              execution_process_id: 'process-1',
              tool_use_id: 'tool-2',
              tool_name: 'Bash',
              status: 'approved',
              created_at: '2026-02-26T10:01:00.000Z',
              responded_at: '2026-02-26T10:01:30.000Z',
            },
          },
          {
            op: 'add',
            path: '/approvals/a3',
            value: {
              id: 'a3',
              attempt_id: 'attempt-1',
              execution_process_id: 'process-1',
              tool_use_id: 'tool-3',
              tool_name: 'WebFetch',
              status: 'pending',
              created_at: '2026-02-26T10:02:00.000Z',
              responded_at: null,
            },
          },
        ],
      });
    });

    await waitFor(() => {
      expect(result.current.approvals.map((item) => item.id)).toEqual(['a1', 'a2', 'a3']);
      expect(result.current.pendingApprovals.map((item) => item.id)).toEqual(['a3']);
    });

    act(() => {
      result.current.reconnect();
    });

    await waitFor(() => {
      expect(MockWebSocket.instances.length).toBeGreaterThan(initialCount);
    });

    const reconnectWs = MockWebSocket.instances[MockWebSocket.instances.length - 1];
    expect(reconnectWs.url).toContain('since_seq=2');
  });

  it('resets and reconnects when approvals stream receives gap_detected', async () => {
    const { result } = renderHook(
      () => useApprovalsStream({ executionProcessId: 'process-2' }),
      {
        wrapper: createWrapper(),
      }
    );

    await waitFor(() => {
      expect(MockWebSocket.instances.length).toBeGreaterThan(0);
    });

    const initialCount = MockWebSocket.instances.length;
    const firstWs = MockWebSocket.instances[initialCount - 1];

    act(() => {
      firstWs.emitOpen();
      firstWs.emit({
        type: 'snapshot',
        sequence_id: 3,
        data: {
          approvals: {
            a1: {
              id: 'a1',
              attempt_id: 'attempt-1',
              execution_process_id: 'process-2',
              tool_use_id: 'tool-1',
              tool_name: 'Read',
              status: 'pending',
              created_at: '2026-02-26T10:00:00.000Z',
              responded_at: null,
            },
          },
        },
      });
    });

    await waitFor(() => {
      expect(result.current.approvals.map((item) => item.id)).toEqual(['a1']);
    });

    act(() => {
      firstWs.emit({
        type: 'gap_detected',
        requested_since_seq: 99,
        max_available_sequence_id: 4,
      });
    });

    await waitFor(() => {
      expect(result.current.error).toContain('Stream gap detected');
      expect(MockWebSocket.instances.length).toBeGreaterThan(initialCount);
    });

    const reconnectWs = MockWebSocket.instances[MockWebSocket.instances.length - 1];
    expect(reconnectWs.url).not.toContain('since_seq=');
  });

  it('applies remove patch operations and recomputes pending approvals', async () => {
    const { result } = renderHook(
      () => useApprovalsStream({ executionProcessId: 'process-3' }),
      {
        wrapper: createWrapper(),
      }
    );

    await waitFor(() => {
      expect(MockWebSocket.instances.length).toBeGreaterThan(0);
    });

    const ws = MockWebSocket.instances[MockWebSocket.instances.length - 1];

    act(() => {
      ws.emitOpen();
      ws.emit({
        type: 'snapshot',
        sequence_id: 1,
        data: {
          approvals: {
            a1: {
              id: 'a1',
              attempt_id: 'attempt-3',
              execution_process_id: 'process-3',
              tool_use_id: 'tool-1',
              tool_name: 'Read',
              status: 'pending',
              created_at: '2026-02-26T10:00:00.000Z',
              responded_at: null,
            },
            a2: {
              id: 'a2',
              attempt_id: 'attempt-3',
              execution_process_id: 'process-3',
              tool_use_id: 'tool-2',
              tool_name: 'Bash',
              status: 'pending',
              created_at: '2026-02-26T10:01:00.000Z',
              responded_at: null,
            },
          },
        },
      });
      ws.emit({
        type: 'patch',
        sequence_id: 2,
        operations: [
          {
            op: 'remove',
            path: '/approvals/a1',
          },
          {
            op: 'replace',
            path: '/approvals/a2',
            value: {
              id: 'a2',
              attempt_id: 'attempt-3',
              execution_process_id: 'process-3',
              tool_use_id: 'tool-2',
              tool_name: 'Bash',
              status: 'approved',
              created_at: '2026-02-26T10:01:00.000Z',
              responded_at: '2026-02-26T10:01:30.000Z',
            },
          },
        ],
      });
    });

    await waitFor(() => {
      expect(result.current.approvals.map((item) => item.id)).toEqual(['a2']);
      expect(result.current.pendingApprovals).toEqual([]);
    });
  });

  it('ignores stale and duplicate sequenced patch messages', async () => {
    const { result } = renderHook(
      () => useApprovalsStream({ executionProcessId: 'process-4' }),
      {
        wrapper: createWrapper(),
      }
    );

    await waitFor(() => {
      expect(MockWebSocket.instances.length).toBeGreaterThan(0);
    });

    const ws = MockWebSocket.instances[MockWebSocket.instances.length - 1];

    act(() => {
      ws.emitOpen();
      ws.emit({
        type: 'snapshot',
        sequence_id: 5,
        data: {
          approvals: {
            a1: {
              id: 'a1',
              attempt_id: 'attempt-4',
              execution_process_id: 'process-4',
              tool_use_id: 'tool-1',
              tool_name: 'Read',
              status: 'pending',
              created_at: '2026-02-26T10:00:00.000Z',
              responded_at: null,
            },
          },
        },
      });
      // stale sequence (older than last sequence=5) => ignored
      ws.emit({
        type: 'patch',
        sequence_id: 4,
        operations: [
          {
            op: 'add',
            path: '/approvals/a-stale',
            value: {
              id: 'a-stale',
              attempt_id: 'attempt-4',
              execution_process_id: 'process-4',
              tool_use_id: 'tool-stale',
              tool_name: 'Bash',
              status: 'pending',
              created_at: '2026-02-26T10:00:01.000Z',
              responded_at: null,
            },
          },
        ],
      });
      // duplicate sequence (equal to last sequence=5) => ignored
      ws.emit({
        type: 'patch',
        sequence_id: 5,
        operations: [
          {
            op: 'replace',
            path: '/approvals/a1',
            value: {
              id: 'a1',
              attempt_id: 'attempt-4',
              execution_process_id: 'process-4',
              tool_use_id: 'tool-1',
              tool_name: 'Read',
              status: 'approved',
              created_at: '2026-02-26T10:00:00.000Z',
              responded_at: '2026-02-26T10:01:10.000Z',
            },
          },
        ],
      });
      // next valid sequence => applied
      ws.emit({
        type: 'patch',
        sequence_id: 6,
        operations: [
          {
            op: 'add',
            path: '/approvals/a2',
            value: {
              id: 'a2',
              attempt_id: 'attempt-4',
              execution_process_id: 'process-4',
              tool_use_id: 'tool-2',
              tool_name: 'WebFetch',
              status: 'pending',
              created_at: '2026-02-26T10:02:00.000Z',
              responded_at: null,
            },
          },
        ],
      });
    });

    await waitFor(() => {
      expect(result.current.approvals.map((item) => item.id)).toEqual(['a1', 'a2']);
      expect(result.current.pendingApprovals.map((item) => item.id)).toEqual(['a1', 'a2']);
    });
  });

  it('accepts reconnect snapshot with higher persisted sequence and keeps cursor progression', async () => {
    const { result } = renderHook(
      () => useApprovalsStream({ executionProcessId: 'process-5' }),
      {
        wrapper: createWrapper(),
      }
    );

    await waitFor(() => {
      expect(MockWebSocket.instances.length).toBeGreaterThan(0);
    });

    const firstWs = MockWebSocket.instances[MockWebSocket.instances.length - 1];

    act(() => {
      firstWs.emitOpen();
      firstWs.emit({
        type: 'snapshot',
        sequence_id: 2,
        data: {
          approvals: {
            a1: {
              id: 'a1',
              attempt_id: 'attempt-5',
              execution_process_id: 'process-5',
              tool_use_id: 'tool-1',
              tool_name: 'Read',
              status: 'pending',
              created_at: '2026-02-26T10:00:00.000Z',
              responded_at: null,
            },
          },
        },
      });
    });

    await waitFor(() => {
      expect(result.current.approvals.map((item) => item.id)).toEqual(['a1']);
    });

    act(() => {
      result.current.reconnect();
    });

    await waitFor(() => {
      expect(MockWebSocket.instances.length).toBeGreaterThan(1);
    });

    const reconnectWs = MockWebSocket.instances[MockWebSocket.instances.length - 1];
    expect(reconnectWs.url).toContain('since_seq=2');

    act(() => {
      reconnectWs.emitOpen();
      reconnectWs.emit({
        type: 'snapshot',
        sequence_id: 15,
        data: {
          approvals: {
            a2: {
              id: 'a2',
              attempt_id: 'attempt-5',
              execution_process_id: 'process-5',
              tool_use_id: 'tool-2',
              tool_name: 'Bash',
              status: 'pending',
              created_at: '2026-02-26T10:02:00.000Z',
              responded_at: null,
            },
          },
        },
      });
      reconnectWs.emit({
        type: 'patch',
        sequence_id: 16,
        operations: [
          {
            op: 'add',
            path: '/approvals/a3',
            value: {
              id: 'a3',
              attempt_id: 'attempt-5',
              execution_process_id: 'process-5',
              tool_use_id: 'tool-3',
              tool_name: 'WebFetch',
              status: 'pending',
              created_at: '2026-02-26T10:03:00.000Z',
              responded_at: null,
            },
          },
        ],
      });
    });

    await waitFor(() => {
      expect(result.current.approvals.map((item) => item.id)).toEqual(['a2', 'a3']);
      expect(result.current.pendingApprovals.map((item) => item.id)).toEqual(['a2', 'a3']);
    });

    act(() => {
      result.current.reconnect();
    });

    await waitFor(() => {
      expect(MockWebSocket.instances.length).toBeGreaterThan(2);
    });

    const thirdWs = MockWebSocket.instances[MockWebSocket.instances.length - 1];
    expect(thirdWs.url).toContain('since_seq=16');
  });
});
