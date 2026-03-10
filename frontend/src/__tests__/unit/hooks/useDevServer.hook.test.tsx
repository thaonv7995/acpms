import { renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  extractPreviewSignalFromAttemptLogs,
  useDevServer,
} from '@/hooks/useDevServer';
import {
  getPreview,
  getPreviewControl,
  getPreviewReadiness,
  getPreviewRuntimeStatus,
} from '@/api/previews';
import { getAttemptLogs } from '@/api/taskAttempts';

vi.mock('@/api/previews', () => ({
  createPreview: vi.fn(),
  getPreview: vi.fn(),
  getPreviewControl: vi.fn(),
  getPreviewReadiness: vi.fn(),
  getPreviewRuntimeStatus: vi.fn(),
  stopPreviewForAttempt: vi.fn(),
}));

vi.mock('@/api/taskAttempts', () => ({
  getAttemptLogs: vi.fn(),
}));

describe('useDevServer preview URL syncing', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.localStorage.clear();

    vi.mocked(getPreviewReadiness).mockResolvedValue({
      attempt_id: 'attempt-1',
      project_type: 'web',
      preview_supported: true,
      preview_enabled: true,
      runtime_enabled: true,
      cloudflare_ready: true,
      ready: true,
      missing_cloudflare_fields: [],
      reason: null,
    });

    vi.mocked(getPreviewControl).mockResolvedValue({
      attempt_id: 'attempt-1',
      preview_available: false,
      controllable: false,
      dismissible: false,
      action: 'none',
      runtime_type: null,
      control_source: null,
      container_name: null,
      compose_project_name: null,
    });

    vi.mocked(getPreviewRuntimeStatus).mockResolvedValue({
      attempt_id: 'attempt-1',
      runtime_enabled: true,
      worktree_path: null,
      compose_file_exists: false,
      docker_project_name: null,
      compose_file_path: null,
      running_services: [],
      runtime_ready: true,
      last_error: null,
      started_at: null,
      stopped_at: null,
      message: null,
    });
  });

  it('replaces a fallback local preview URL with the latest public preview URL from logs', async () => {
    vi.mocked(getPreview).mockResolvedValue(null);
    vi.mocked(getPreviewControl).mockResolvedValue({
      attempt_id: 'attempt-1',
      preview_available: true,
      controllable: false,
      dismissible: true,
      action: 'dismiss',
      runtime_type: null,
      control_source: 'agent_output',
      container_name: null,
      compose_project_name: null,
    });
    vi.mocked(getAttemptLogs).mockResolvedValue([
      {
        id: 'log-1',
        attempt_id: 'attempt-1',
        log_type: 'stdout',
        content:
          'PREVIEW_TARGET: http://127.0.0.1:4174\nPREVIEW_URL: https://alike-demonstration-ace-provides.trycloudflare.com',
        created_at: '2026-03-07T13:45:00Z',
      },
    ]);

    const { result } = renderHook(() =>
      useDevServer(
        'task-1',
        'attempt-1',
        'http://127.0.0.1:4174'
      )
    );

    await waitFor(() => {
      expect(result.current.url).toBe(
        'https://alike-demonstration-ace-provides.trycloudflare.com'
      );
    });

    expect(result.current.externalPreview).toBe(true);
  });

  it('prefers the newest public preview signal even when logs arrive newest-first', () => {
    const signal = extractPreviewSignalFromAttemptLogs([
      {
        id: 'log-new',
        attempt_id: 'attempt-1',
        log_type: 'stdout',
        content: 'PREVIEW_URL: https://preview-9b0679a1.thaonv.online',
        created_at: '2026-03-10T10:25:19Z',
      },
      {
        id: 'log-old',
        attempt_id: 'attempt-1',
        log_type: 'stdout',
        content: 'PREVIEW_TARGET: http://127.0.0.1:8080',
        created_at: '2026-03-10T10:03:32Z',
      },
    ]);

    expect(signal).toEqual({
      url: 'https://preview-9b0679a1.thaonv.online',
      signalKey:
        'log-new:2026-03-10T10:25:19Z:https://preview-9b0679a1.thaonv.online',
    });
  });

  it('prefers metadata fallback over log-derived local preview when the attempt is done', async () => {
    vi.mocked(getPreview).mockResolvedValue(null);
    vi.mocked(getPreviewControl).mockResolvedValue({
      attempt_id: 'attempt-1',
      preview_available: true,
      controllable: false,
      dismissible: true,
      action: 'dismiss',
      runtime_type: null,
      control_source: 'file_contract',
      container_name: null,
      compose_project_name: null,
    });
    vi.mocked(getAttemptLogs).mockResolvedValue([
      {
        id: 'log-local',
        attempt_id: 'attempt-1',
        log_type: 'stdout',
        content: 'PREVIEW_TARGET: http://127.0.0.1:8080',
        created_at: '2026-03-10T10:03:32Z',
      },
    ]);

    const { result } = renderHook(() =>
      useDevServer(
        'task-1',
        'attempt-1',
        'https://preview-0d010f14-bb2b-43bd-9345-4cb0bcacb893.thaonv.online',
        false,
        'SUCCESS'
      )
    );

    await waitFor(() => {
      expect(result.current.url).toBe(
        'https://preview-0d010f14-bb2b-43bd-9345-4cb0bcacb893.thaonv.online'
      );
    });

    expect(getAttemptLogs).not.toHaveBeenCalled();
    expect(result.current.externalPreview).toBe(true);
  });

  it('refreshes the running preview when the backend preview URL changes', async () => {
    vi.mocked(getAttemptLogs).mockResolvedValue([]);
    vi.mocked(getPreview)
      .mockResolvedValueOnce({
        id: 'preview-1',
        attempt_id: 'attempt-1',
        preview_url: 'http://127.0.0.1:4174',
        status: 'active',
        created_at: '2026-03-07T13:45:00Z',
        expires_at: null,
      })
      .mockResolvedValue({
        id: 'preview-1',
        attempt_id: 'attempt-1',
        preview_url: 'https://alike-demonstration-ace-provides.trycloudflare.com',
        status: 'active',
        created_at: '2026-03-07T13:45:00Z',
        expires_at: null,
      });

    const { result } = renderHook(() => useDevServer('task-1', 'attempt-1'));

    await waitFor(() => {
      expect(result.current.url).toBe(
        'https://alike-demonstration-ace-provides.trycloudflare.com'
      );
    });

    expect(result.current.previewRevision).toBeGreaterThan(0);
  });
});
