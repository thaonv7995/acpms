import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  authenticatedFetch,
  clearTokens,
  getAccessToken,
  getRefreshToken,
  setAccessToken,
  setRefreshToken,
} from '@/api/client';

function toBase64Url(value: string): string {
  return btoa(value).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '');
}

function makeJwtWithExp(exp: number): string {
  return [
    toBase64Url(JSON.stringify({ alg: 'HS256', typ: 'JWT' })),
    toBase64Url(JSON.stringify({ exp })),
    'signature',
  ].join('.');
}

describe('authenticatedFetch token refresh flow', () => {
  beforeEach(() => {
    clearTokens();
    vi.restoreAllMocks();
  });

  it('sends bearer token and returns response when request succeeds', async () => {
    setAccessToken('token-1');

    const fetchMock = vi.fn().mockResolvedValue(new Response('{}', { status: 200 }));
    vi.stubGlobal('fetch', fetchMock);

    const response = await authenticatedFetch('/api/v1/projects');

    expect(response.status).toBe(200);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [, options] = fetchMock.mock.calls[0];
    expect(options.headers.Authorization).toBe('Bearer token-1');
  });

  it('refreshes access token on 401 and retries request once', async () => {
    setAccessToken('expired-token');
    setRefreshToken('refresh-token');

    const fetchMock = vi
      .fn()
      // Original request -> 401
      .mockResolvedValueOnce(new Response('{}', { status: 401 }))
      // Refresh token request -> 200
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            success: true,
            code: '2000',
            message: 'Token refreshed successfully',
            data: {
              access_token: 'new-token',
              expires_in: 1800,
            },
          }),
          {
            status: 200,
            headers: { 'Content-Type': 'application/json' },
          }
        )
      )
      // Retried request -> 200
      .mockResolvedValueOnce(new Response('{}', { status: 200 }));

    vi.stubGlobal('fetch', fetchMock);

    const response = await authenticatedFetch('/api/v1/projects');

    expect(response.status).toBe(200);
    expect(fetchMock).toHaveBeenCalledTimes(3);

    const [refreshUrl] = fetchMock.mock.calls[1];
    expect(refreshUrl).toContain('/api/v1/auth/refresh');

    const [, retryOptions] = fetchMock.mock.calls[2];
    expect(retryOptions.headers.Authorization).toBe('Bearer new-token');
    expect(getAccessToken()).toBe('new-token');
  });

  it('uses a single refresh request for concurrent 401 responses', async () => {
    setAccessToken('expired-token');
    setRefreshToken('refresh-token');

    const fetchMock = vi
      .fn()
      // two parallel protected requests -> both 401
      .mockResolvedValueOnce(new Response('{}', { status: 401 }))
      .mockResolvedValueOnce(new Response('{}', { status: 401 }))
      // single refresh request
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            success: true,
            code: '2000',
            message: 'Token refreshed successfully',
            data: {
              access_token: 'new-token',
              expires_in: 1800,
            },
          }),
          {
            status: 200,
            headers: { 'Content-Type': 'application/json' },
          }
        )
      )
      // retries for both requests
      .mockResolvedValueOnce(new Response('{}', { status: 200 }))
      .mockResolvedValueOnce(new Response('{}', { status: 200 }));

    vi.stubGlobal('fetch', fetchMock);

    const [res1, res2] = await Promise.all([
      authenticatedFetch('/api/v1/projects'),
      authenticatedFetch('/api/v1/tasks'),
    ]);

    expect(res1.status).toBe(200);
    expect(res2.status).toBe(200);
    expect(fetchMock).toHaveBeenCalledTimes(5);

    const refreshCalls = fetchMock.mock.calls.filter(([url]) =>
      String(url).includes('/api/v1/auth/refresh')
    );
    expect(refreshCalls).toHaveLength(1);
  });

  it('refreshes access token proactively before it expires', async () => {
    const expiringSoonToken = makeJwtWithExp(Math.floor(Date.now() / 1000) + 30);
    setAccessToken(expiringSoonToken);
    setRefreshToken('refresh-token');

    const fetchMock = vi
      .fn()
      // proactive refresh
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            success: true,
            code: '2000',
            message: 'Token refreshed successfully',
            data: {
              access_token: 'new-token',
              expires_in: 1800,
            },
          }),
          {
            status: 200,
            headers: { 'Content-Type': 'application/json' },
          }
        )
      )
      // original request with fresh token
      .mockResolvedValueOnce(new Response('{}', { status: 200 }));

    vi.stubGlobal('fetch', fetchMock);

    const response = await authenticatedFetch('/api/v1/projects');

    expect(response.status).toBe(200);
    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(String(fetchMock.mock.calls[0][0])).toContain('/api/v1/auth/refresh');

    const [, requestOptions] = fetchMock.mock.calls[1];
    expect(requestOptions.headers.Authorization).toBe('Bearer new-token');
    expect(getAccessToken()).toBe('new-token');
  });

  it('does not clear tokens when refresh fails transiently', async () => {
    setAccessToken('expired-token');
    setRefreshToken('refresh-token');

    const fetchMock = vi
      .fn()
      // protected request -> 401
      .mockResolvedValueOnce(new Response('{}', { status: 401 }))
      // refresh request -> temporary server failure
      .mockResolvedValueOnce(new Response('{}', { status: 500 }));

    vi.stubGlobal('fetch', fetchMock);

    await expect(authenticatedFetch('/api/v1/projects')).rejects.toMatchObject({
      status: 401,
      code: 'AUTH_REFRESH_FAILED',
    });

    expect(getAccessToken()).toBe('expired-token');
    expect(getRefreshToken()).toBe('refresh-token');
  });
});
