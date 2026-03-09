import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useProjectAssistant } from '@/hooks/useProjectAssistant';
import {
  createSession,
  getAssistantLogsWsUrl,
  getSession,
  getSessionStatus,
  listSessions,
  postInput,
  postMessage,
  startSession,
  endSession,
} from '@/api/projectAssistant';

vi.mock('@/api/projectAssistant', () => ({
  createSession: vi.fn(),
  listSessions: vi.fn(),
  getSession: vi.fn(),
  postMessage: vi.fn(),
  postInput: vi.fn(),
  endSession: vi.fn(),
  startSession: vi.fn(),
  getSessionStatus: vi.fn(),
  getAssistantLogsWsUrl: vi.fn(() => 'ws://assistant.test'),
}));

vi.mock('@/api/client', () => ({
  getAccessToken: vi.fn(() => null),
}));

class MockWebSocket {
  static OPEN = 1;

  readyState = MockWebSocket.OPEN;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onclose: (() => void) | null = null;

  constructor(_url: string, _protocols?: string | string[]) {}

  close() {
    this.readyState = 3;
    this.onclose?.();
  }
}

const baseSession = {
  id: 'session-1',
  project_id: 'project-1',
  user_id: 'user-1',
  status: 'active',
  s3_log_key: null,
  created_at: '2026-03-10T00:00:00.000Z',
  ended_at: null,
};

describe('useProjectAssistant', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.stubGlobal('WebSocket', MockWebSocket);

    vi.mocked(createSession).mockResolvedValue(baseSession);
    vi.mocked(listSessions).mockResolvedValue([]);
    vi.mocked(getSession).mockResolvedValue({ session: baseSession, messages: [] });
    vi.mocked(getSessionStatus).mockResolvedValue({ active: false });
    vi.mocked(startSession).mockResolvedValue();
    vi.mocked(endSession).mockResolvedValue(baseSession);
    vi.mocked(getAssistantLogsWsUrl).mockReturnValue('ws://assistant.test');
  });

  it('does not fall back to postInput when attachments would be dropped on a 409 conflict', async () => {
    vi.mocked(postMessage).mockRejectedValue({ status: 409 });
    vi.mocked(postInput).mockResolvedValue();

    const { result } = renderHook(() => useProjectAssistant('project-1'));

    await act(async () => {
      await result.current.createSession(false);
    });

    let ok = false;
    await act(async () => {
      ok = await result.current.sendMessage('Please review this file', [
        { key: 'projects/project-1/assistant-attachments/test.txt', filename: 'test.txt' },
      ]);
    });

    expect(ok).toBe(false);
    expect(postInput).not.toHaveBeenCalled();
    await waitFor(() => {
      expect(result.current.error).toContain('Attachments can only be sent');
    });
  });

  it('still falls back to postInput for text-only follow-up messages', async () => {
    vi.mocked(postMessage).mockRejectedValue({ status: 409 });
    vi.mocked(postInput).mockResolvedValue();

    const { result } = renderHook(() => useProjectAssistant('project-1'));

    await act(async () => {
      await result.current.createSession(false);
    });

    let ok = false;
    await act(async () => {
      ok = await result.current.sendMessage('Follow up without attachments');
    });

    expect(ok).toBe(true);
    expect(postInput).toHaveBeenCalledWith(
      'project-1',
      'session-1',
      'Follow up without attachments'
    );
  });
});
