import React from 'react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, waitFor, fireEvent, act } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ExecutionLogs } from '@/components/agents/ExecutionLogs';
import { getAttemptLogs } from '@/api/taskAttempts';
import { getAccessToken } from '@/api/client';
import { respondToApproval } from '@/api/approvals';

vi.mock('@/api/taskAttempts', () => ({
  getAttemptLogs: vi.fn(),
}));

vi.mock('@/api/client', () => ({
  getAccessToken: vi.fn(),
}));

vi.mock('@/api/approvals', () => ({
  respondToApproval: vi.fn(),
}));

type StreamMessage =
  | {
      type: 'ApprovalRequest';
      attempt_id: string;
      execution_process_id?: string | null;
      tool_use_id: string;
      tool_name: string;
      tool_input: Record<string, unknown>;
      timestamp: string;
    }
  | {
      type: 'Log';
      attempt_id: string;
      log_type: string;
      content: string;
      timestamp: string;
    };

class MockWebSocket {
  static instances: MockWebSocket[] = [];
  static readonly OPEN = 1;

  readonly url: string;
  readonly protocols: string[];

  onopen: ((event: Event) => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;
  onclose: ((event: CloseEvent) => void) | null = null;
  readyState = MockWebSocket.OPEN;

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
      queries: { retry: false },
      mutations: { retry: false },
    },
  });

  return function Wrapper({ children }: { children: React.ReactNode }) {
    return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
  };
}

describe('ExecutionLogs approval integration', () => {
  beforeEach(() => {
    MockWebSocket.instances = [];
    vi.stubGlobal('WebSocket', MockWebSocket as unknown as typeof WebSocket);
    Object.defineProperty(Element.prototype, 'scrollIntoView', {
      configurable: true,
      value: vi.fn(),
    });
    vi.clearAllMocks();
    vi.mocked(getAttemptLogs).mockResolvedValue([]);
    vi.mocked(getAccessToken).mockReturnValue(null);
    vi.mocked(respondToApproval).mockResolvedValue({ success: true } as any);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.clearAllMocks();
  });

  it('opens approval modal from websocket request and submits approve decision', async () => {
    render(<ExecutionLogs attemptId="attempt-1" />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(getAttemptLogs).toHaveBeenCalledWith('attempt-1');
      expect(MockWebSocket.instances.length).toBeGreaterThan(0);
    });

    const ws = MockWebSocket.instances[MockWebSocket.instances.length - 1];
    act(() => {
      ws.emitOpen();
      ws.emit({
        type: 'ApprovalRequest',
        attempt_id: 'attempt-1',
        execution_process_id: 'process-1',
        tool_use_id: 'tool-use-1',
        tool_name: 'Bash',
        tool_input: { command: 'ls -la' },
        timestamp: '2026-02-27T10:58:00.000Z',
      });
    });

    await waitFor(() => {
      expect(screen.getByText('Bash - Pending Approval')).toBeTruthy();
      expect(screen.getByText('Tool Permission Request')).toBeTruthy();
    });

    fireEvent.click(screen.getByRole('button', { name: 'Approve' }));

    await waitFor(() => {
      expect(respondToApproval).toHaveBeenCalledWith('tool-use-1', 'approve', undefined);
    });
  });
});
