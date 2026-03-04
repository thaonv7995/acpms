import { renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useAgentAuthSessionStream } from '../../../hooks/useAgentAuthSessionStream';
import {
  isWsCollectionStreamEnabled,
  useWsCollectionStream,
} from '../../../hooks/useWsCollectionStream';

vi.mock('../../../hooks/useWsCollectionStream', () => ({
  isWsCollectionStreamEnabled: vi.fn(),
  useWsCollectionStream: vi.fn(),
}));

const session = {
  session_id: 'session-1',
  provider: 'openai-codex',
  flow_type: 'device_flow' as const,
  status: 'waiting_user_action' as const,
  created_at: '2026-02-27T10:00:00.000Z',
  updated_at: '2026-02-27T10:00:00.000Z',
  expires_at: '2026-02-27T10:05:00.000Z',
  process_pid: 1234,
  allowed_loopback_port: null,
  last_seq: 1,
  last_error: null,
  result: null,
  action_url: 'https://github.com/login/device',
  action_code: 'ABCD-1234',
  action_hint: 'Open URL and paste code',
};

describe('useAgentAuthSessionStream', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(isWsCollectionStreamEnabled).mockReturnValue(true);
    vi.mocked(useWsCollectionStream).mockReturnValue({
      items: [session],
      isStreaming: true,
      error: null,
      reconnect: vi.fn(),
      lastSequenceId: 3,
    } as any);
  });

  it('returns first session item from ws collection stream', () => {
    const { result } = renderHook(() => useAgentAuthSessionStream('session-1'));
    expect(result.current.session?.session_id).toBe('session-1');
    expect(result.current.isStreaming).toBe(true);
    expect(result.current.error).toBeNull();
  });

  it('disables stream when session id is missing', () => {
    renderHook(() => useAgentAuthSessionStream(undefined));

    expect(useWsCollectionStream).toHaveBeenCalledWith(
      expect.objectContaining({
        enabled: false,
        url: null,
      })
    );
  });

  it('builds auth ws endpoint using session id', () => {
    renderHook(() => useAgentAuthSessionStream('session-abc'));

    expect(useWsCollectionStream).toHaveBeenCalledWith(
      expect.objectContaining({
        enabled: true,
        url: expect.stringContaining('/api/v1/agent/auth/sessions/session-abc/ws'),
      })
    );
  });
});
