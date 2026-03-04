import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useAgentAuthSessionStream } from '../../../hooks/useAgentAuthSessionStream';
import { getAccessToken } from '@/api/client';

vi.mock('@/api/client', () => ({
  API_PREFIX: '/api/v1',
  getAccessToken: vi.fn(),
}));

type SessionStatus =
  | 'initiated'
  | 'waiting_user_action'
  | 'verifying'
  | 'succeeded'
  | 'failed'
  | 'cancelled'
  | 'timed_out';

type StreamMessage =
  | {
      type: 'snapshot';
      sequence_id: number;
      session: Record<string, unknown>;
    }
  | {
      type: 'upsert';
      sequence_id: number;
      session: Record<string, unknown>;
    }
  | {
      type: 'gap_detected';
      requested_since_seq: number;
      max_available_sequence_id: number;
    };

function makeSession(status: SessionStatus, lastSeq: number) {
  return {
    session_id: 'auth-session-1',
    provider: 'openai-codex',
    flow_type: 'device_flow',
    status,
    created_at: '2026-02-27T10:00:00.000Z',
    updated_at: '2026-02-27T10:00:00.000Z',
    expires_at: '2026-02-27T10:05:00.000Z',
    process_pid: 1234,
    allowed_loopback_port: null,
    last_seq: lastSeq,
    last_error: null,
    result: null,
    action_url: 'https://github.com/login/device',
    action_code: 'ABCD-1234',
    action_hint: 'Open URL and paste code',
  };
}

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

describe('useAgentAuthSessionStream with real ws collection stream hook', () => {
  beforeEach(() => {
    MockWebSocket.instances = [];
    vi.stubGlobal('WebSocket', MockWebSocket as unknown as typeof WebSocket);
    vi.clearAllMocks();
    vi.mocked(getAccessToken).mockReturnValue('token-auth');
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.clearAllMocks();
  });

  it('applies snapshot/upsert events and reconnects with since_seq cursor', async () => {
    const { result } = renderHook(() => useAgentAuthSessionStream('auth-session-1'));

    await waitFor(() => {
      expect(MockWebSocket.instances.length).toBeGreaterThan(0);
    });

    const firstWs = MockWebSocket.instances[MockWebSocket.instances.length - 1];
    expect(firstWs.url).toContain('/api/v1/agent/auth/sessions/auth-session-1/ws');
    expect(firstWs.protocols).toEqual(['acpms-bearer', 'token-auth']);

    act(() => {
      firstWs.emitOpen();
      firstWs.emit({
        type: 'snapshot',
        sequence_id: 1,
        session: makeSession('waiting_user_action', 1),
      });
      firstWs.emit({
        type: 'upsert',
        sequence_id: 2,
        session: makeSession('verifying', 2),
      });
    });

    await waitFor(() => {
      expect(result.current.session?.status).toBe('verifying');
      expect(result.current.session?.last_seq).toBe(2);
    });

    act(() => {
      result.current.reconnect();
    });

    await waitFor(() => {
      expect(MockWebSocket.instances.length).toBeGreaterThan(1);
    });

    const reconnectWs = MockWebSocket.instances[MockWebSocket.instances.length - 1];
    expect(reconnectWs.url).toContain('since_seq=2');
  });

  it('resets cursor and reconnects from snapshot on gap_detected', async () => {
    const { result } = renderHook(() => useAgentAuthSessionStream('auth-session-1'));

    await waitFor(() => {
      expect(MockWebSocket.instances.length).toBeGreaterThan(0);
    });

    const firstWs = MockWebSocket.instances[MockWebSocket.instances.length - 1];
    act(() => {
      firstWs.emitOpen();
      firstWs.emit({
        type: 'snapshot',
        sequence_id: 4,
        session: makeSession('waiting_user_action', 4),
      });
    });

    await waitFor(() => {
      expect(result.current.session?.last_seq).toBe(4);
    });

    act(() => {
      firstWs.emit({
        type: 'gap_detected',
        requested_since_seq: 10,
        max_available_sequence_id: 6,
      });
    });

    await waitFor(() => {
      expect(result.current.error).toContain('Stream gap detected');
      expect(MockWebSocket.instances.length).toBeGreaterThan(1);
    });

    const reconnectWs = MockWebSocket.instances[MockWebSocket.instances.length - 1];
    expect(reconnectWs.url).not.toContain('since_seq=');
  });
});
