import { beforeEach, describe, expect, it, vi } from 'vitest';
import { clearTokens, setAccessToken } from '@/api/client';
import {
  cancelDeploymentRun,
  getDeploymentRunStreamUrl,
  listDeploymentReleases,
  listDeploymentRuns,
  retryDeploymentRun,
  rollbackDeploymentRun,
  startDeploymentRun,
} from '@/api/deploymentEnvironments';

function successResponse<T>(data: T): Response {
  return new Response(
    JSON.stringify({
      success: true,
      code: '0000',
      message: 'ok',
      data,
    }),
    {
      status: 200,
      headers: { 'Content-Type': 'application/json' },
    }
  );
}

describe('deployment environments API', () => {
  beforeEach(() => {
    clearTokens();
    setAccessToken('token-1');
    vi.restoreAllMocks();
  });

  it('builds query string when listing runs', async () => {
    const fetchMock = vi.fn().mockResolvedValue(successResponse([]));
    vi.stubGlobal('fetch', fetchMock);

    await listDeploymentRuns('project-1', {
      environment_id: 'env-1',
      status: 'running',
      limit: 25,
    });

    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [url, options] = fetchMock.mock.calls[0];
    expect(String(url)).toContain('/api/v1/projects/project-1/deployment-runs?');
    expect(String(url)).toContain('environment_id=env-1');
    expect(String(url)).toContain('status=running');
    expect(String(url)).toContain('limit=25');
    expect(options.method ?? 'GET').toBe('GET');
  });

  it('calls start/cancel/retry/rollback run endpoints with POST', async () => {
    const runData = { id: 'run-1' };
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(successResponse(runData))
      .mockResolvedValueOnce(successResponse(runData))
      .mockResolvedValueOnce(successResponse(runData))
      .mockResolvedValueOnce(successResponse(runData));
    vi.stubGlobal('fetch', fetchMock);

    await startDeploymentRun('project-1', 'env-1', { source_type: 'branch', source_ref: 'main' });
    await cancelDeploymentRun('run-1');
    await retryDeploymentRun('run-1');
    await rollbackDeploymentRun('run-1', { target_release_id: 'release-1' });

    expect(fetchMock).toHaveBeenCalledTimes(4);
    expect(String(fetchMock.mock.calls[0][0])).toContain(
      '/api/v1/projects/project-1/deployment-environments/env-1/deploy'
    );
    expect(fetchMock.mock.calls[0][1].method).toBe('POST');

    expect(String(fetchMock.mock.calls[1][0])).toContain('/api/v1/deployment-runs/run-1/cancel');
    expect(fetchMock.mock.calls[1][1].method).toBe('POST');

    expect(String(fetchMock.mock.calls[2][0])).toContain('/api/v1/deployment-runs/run-1/retry');
    expect(fetchMock.mock.calls[2][1].method).toBe('POST');

    expect(String(fetchMock.mock.calls[3][0])).toContain('/api/v1/deployment-runs/run-1/rollback');
    expect(fetchMock.mock.calls[3][1].method).toBe('POST');
    expect(fetchMock.mock.calls[3][1].body).toContain('release-1');
  });

  it('builds release query and stream url', async () => {
    const fetchMock = vi.fn().mockResolvedValue(successResponse([]));
    vi.stubGlobal('fetch', fetchMock);

    await listDeploymentReleases('project-1', 'env-1', {
      status: 'active',
      limit: 10,
    });

    const [url] = fetchMock.mock.calls[0];
    expect(String(url)).toContain(
      '/api/v1/projects/project-1/deployment-environments/env-1/releases?'
    );
    expect(String(url)).toContain('status=active');
    expect(String(url)).toContain('limit=10');

    expect(getDeploymentRunStreamUrl('run-1')).toBe('/api/v1/deployment-runs/run-1/stream');
    expect(getDeploymentRunStreamUrl('run-1', 'evt-9')).toBe(
      '/api/v1/deployment-runs/run-1/stream?after_id=evt-9'
    );
  });
});
