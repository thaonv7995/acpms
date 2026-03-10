import { beforeEach, describe, expect, it, vi } from 'vitest';
import { clearTokens, setAccessToken } from '@/api/client';
import { getUsersPage } from '@/api/users';

function successResponse<T>(data: T, metadata?: Record<string, unknown>): Response {
  return new Response(
    JSON.stringify({
      success: true,
      code: '0000',
      message: 'ok',
      data,
      metadata,
    }),
    {
      status: 200,
      headers: { 'Content-Type': 'application/json' },
    }
  );
}

describe('users API pagination', () => {
  beforeEach(() => {
    clearTokens();
    setAccessToken('token-1');
    vi.restoreAllMocks();
  });

  it('builds pagination and filter query params for list users', async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      successResponse([], {
        page: 2,
        page_size: 25,
        total_count: 42,
        total_pages: 2,
        has_more: false,
      })
    );
    vi.stubGlobal('fetch', fetchMock);

    const response = await getUsersPage({
      page: 2,
      limit: 25,
      search: 'alice',
      role: 'developer',
      status: 'active',
    });

    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [url, options] = fetchMock.mock.calls[0];
    expect(String(url)).toContain('/api/v1/users?');
    expect(String(url)).toContain('page=2');
    expect(String(url)).toContain('limit=25');
    expect(String(url)).toContain('search=alice');
    expect(String(url)).toContain('role=developer');
    expect(String(url)).toContain('status=active');
    expect(options.method ?? 'GET').toBe('GET');

    expect(response.metadata?.page).toBe(2);
    expect(response.metadata?.page_size).toBe(25);
    expect(response.metadata?.total_count).toBe(42);
    expect(response.metadata?.total_pages).toBe(2);
    expect(response.metadata?.has_more).toBe(false);
  });
});
