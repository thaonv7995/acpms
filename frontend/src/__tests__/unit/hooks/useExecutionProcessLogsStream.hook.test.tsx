import React from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  useExecutionProcessLogsStream,
} from '../../../hooks/useExecutionProcessLogsStream';
import {
  getExecutionProcessNormalizedLogs,
  getExecutionProcessRawLogs,
} from '@/api/executionProcesses';
import { getAccessToken } from '@/api/client';

vi.mock('@/api/executionProcesses', () => ({
  getExecutionProcessNormalizedLogs: vi.fn(),
  getExecutionProcessRawLogs: vi.fn(),
}));

vi.mock('@/api/client', () => ({
  API_PREFIX: '/api/v1',
  getAccessToken: vi.fn(),
}));

type StreamMessage =
  | {
      type: 'event';
      sequence_id: number;
      event: {
        type: 'Log' | 'Status' | 'ApprovalRequest' | 'UserMessage';
        attempt_id: string;
        log_type?: string;
        content?: string;
        timestamp: string;
        created_at?: string;
        id?: string;
        status?: string;
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

describe('useExecutionProcessLogsStream hook', () => {
  beforeEach(() => {
    MockWebSocket.instances = [];
    vi.stubGlobal('WebSocket', MockWebSocket as unknown as typeof WebSocket);
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.clearAllMocks();
  });

  it('merges sequenced WS log events with initial normalized snapshot', async () => {
    vi.mocked(getAccessToken).mockReturnValue('token-123');
    vi.mocked(getExecutionProcessNormalizedLogs).mockResolvedValue([
      {
        id: 'log-base',
        attempt_id: 'attempt-1',
        log_type: 'normalized',
        content: 'base',
        created_at: '2026-02-26T10:00:00.000Z',
      },
    ] as any);

    const { result } = renderHook(
      () => useExecutionProcessLogsStream('process-1', 'normalized'),
      {
        wrapper: createWrapper(),
      }
    );

    await waitFor(() => {
      expect(result.current.logs.map((log) => log.id)).toEqual(['log-base']);
    });

    const latestWs = MockWebSocket.instances[MockWebSocket.instances.length - 1];
    expect(latestWs.protocols).toEqual(['acpms-bearer', 'token-123']);

    act(() => {
      latestWs.emitOpen();
      latestWs.emit({
        type: 'event',
        sequence_id: 2,
        event: {
          type: 'Log',
          attempt_id: 'attempt-1',
          id: 'log-live',
          log_type: 'normalized',
          content: 'live',
          timestamp: '2026-02-26T10:00:01.000Z',
          created_at: '2026-02-26T10:00:01.000Z',
        },
      });
    });

    await waitFor(() => {
      expect(result.current.logs.map((log) => log.id)).toEqual(['log-base', 'log-live']);
      expect(result.current.lastSequenceId).toBe(2);
      expect(result.current.isStreaming).toBe(true);
    });
  });

  it('resyncs from snapshot and reconnects when receiving gap_detected', async () => {
    vi.mocked(getAccessToken).mockReturnValue(null);
    vi.mocked(getExecutionProcessRawLogs).mockResolvedValue([] as any);

    const { result } = renderHook(() => useExecutionProcessLogsStream('process-2', 'raw'), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(MockWebSocket.instances.length).toBeGreaterThan(0);
    });

    const firstWs = MockWebSocket.instances[MockWebSocket.instances.length - 1];

    act(() => {
      firstWs.emitOpen();
      firstWs.emit({
        type: 'event',
        sequence_id: 1,
        event: {
          type: 'Log',
          attempt_id: 'attempt-2',
          id: 'raw-live',
          log_type: 'stdout',
          content: 'line 1',
          timestamp: '2026-02-26T10:00:00.000Z',
          created_at: '2026-02-26T10:00:00.000Z',
        },
      });
    });

    await waitFor(() => {
      expect(result.current.logs.some((log) => log.id === 'raw-live')).toBe(true);
      expect(result.current.lastSequenceId).toBe(1);
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
      expect(MockWebSocket.instances.length).toBeGreaterThan(1);
      expect(result.current.lastSequenceId).toBe(0);
    });

    expect(vi.mocked(getExecutionProcessRawLogs).mock.calls.length).toBeGreaterThanOrEqual(2);
  });

  it('reconnects with since_seq cursor after manual reconnect request', async () => {
    vi.mocked(getAccessToken).mockReturnValue(null);
    vi.mocked(getExecutionProcessRawLogs).mockResolvedValue([] as any);

    const { result } = renderHook(() => useExecutionProcessLogsStream('process-3', 'raw'), {
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
        type: 'event',
        sequence_id: 1,
        event: {
          type: 'Log',
          attempt_id: 'attempt-3',
          id: 'raw-log-1',
          log_type: 'stdout',
          content: 'hello',
          timestamp: '2026-02-26T10:00:00.000Z',
          created_at: '2026-02-26T10:00:00.000Z',
        },
      });
    });

    await waitFor(() => {
      expect(result.current.lastSequenceId).toBe(1);
    });

    act(() => {
      result.current.reconnect();
    });

    await waitFor(() => {
      expect(MockWebSocket.instances.length).toBeGreaterThan(initialCount);
    });

    const reconnectWs = MockWebSocket.instances[MockWebSocket.instances.length - 1];
    expect(reconnectWs.url).toContain('since_seq=1');
  });

  it('ignores stale and duplicate sequenced events', async () => {
    vi.mocked(getAccessToken).mockReturnValue('token-dup');
    vi.mocked(getExecutionProcessNormalizedLogs).mockResolvedValue([
      {
        id: 'log-base',
        attempt_id: 'attempt-4',
        log_type: 'normalized',
        content: 'base',
        created_at: '2026-02-26T10:00:00.000Z',
      },
    ] as any);

    const { result } = renderHook(
      () => useExecutionProcessLogsStream('process-4', 'normalized'),
      {
        wrapper: createWrapper(),
      }
    );

    await waitFor(() => {
      expect(result.current.logs.map((log) => log.id)).toEqual(['log-base']);
    });

    const ws = MockWebSocket.instances[MockWebSocket.instances.length - 1];

    act(() => {
      ws.emitOpen();
      ws.emit({
        type: 'event',
        sequence_id: 2,
        event: {
          type: 'Log',
          attempt_id: 'attempt-4',
          id: 'log-live-2',
          log_type: 'normalized',
          content: 'live-2',
          timestamp: '2026-02-26T10:00:01.000Z',
          created_at: '2026-02-26T10:00:01.000Z',
        },
      });
      ws.emit({
        type: 'event',
        sequence_id: 1,
        event: {
          type: 'Log',
          attempt_id: 'attempt-4',
          id: 'log-stale-1',
          log_type: 'normalized',
          content: 'stale',
          timestamp: '2026-02-26T10:00:00.500Z',
          created_at: '2026-02-26T10:00:00.500Z',
        },
      });
      ws.emit({
        type: 'event',
        sequence_id: 2,
        event: {
          type: 'Log',
          attempt_id: 'attempt-4',
          id: 'log-dup-2',
          log_type: 'normalized',
          content: 'duplicate',
          timestamp: '2026-02-26T10:00:01.500Z',
          created_at: '2026-02-26T10:00:01.500Z',
        },
      });
    });

    await waitFor(() => {
      expect(result.current.logs.map((log) => log.id)).toEqual(['log-base', 'log-live-2']);
      expect(result.current.lastSequenceId).toBe(2);
    });
  });

  it('applies terminal status event sequencing and ignores stale status updates', async () => {
    vi.mocked(getAccessToken).mockReturnValue('token-status');
    vi.mocked(getExecutionProcessRawLogs).mockResolvedValue([
      {
        id: 'raw-base',
        attempt_id: 'attempt-5',
        log_type: 'stdout',
        content: 'base',
        created_at: '2026-02-26T10:00:00.000Z',
      },
    ] as any);

    const { result } = renderHook(() => useExecutionProcessLogsStream('process-5', 'raw'), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(result.current.logs.map((log) => log.id)).toEqual(['raw-base']);
    });

    const ws = MockWebSocket.instances[MockWebSocket.instances.length - 1];

    act(() => {
      ws.emitOpen();
      ws.emit({
        type: 'event',
        sequence_id: 2,
        event: {
          type: 'Status',
          attempt_id: 'attempt-5',
          status: 'Success',
          timestamp: '2026-02-26T10:00:02.000Z',
        },
      });
      ws.emit({
        type: 'event',
        sequence_id: 1,
        event: {
          type: 'Status',
          attempt_id: 'attempt-5',
          status: 'Failed',
          timestamp: '2026-02-26T10:00:03.000Z',
        },
      });
    });

    await waitFor(() => {
      expect(result.current.attemptStatus).toBe('success');
      expect(result.current.lastSequenceId).toBe(2);
      expect(result.current.logs.map((log) => log.id)).toEqual(['raw-base']);
    });
  });
});
