import { renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useDevServer } from '@/hooks/useDevServer';
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
