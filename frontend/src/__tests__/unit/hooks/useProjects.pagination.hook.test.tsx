import React from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useProjects } from '@/hooks/useProjects';
import { getProjects } from '@/api/projects';

vi.mock('@/api/projects', () => ({
  getProjects: vi.fn(),
}));

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

describe('useProjects pagination', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('does not snap back to page one while the next projects page is still loading', async () => {
    let callCount = 0;
    vi.mocked(getProjects).mockImplementation(async (params) => {
      callCount += 1;

      if (callCount === 1) {
        return {
          success: true,
          code: '0000',
          message: 'ok',
          data: [],
          metadata: {
            page: 1,
            page_size: 9,
            total_count: 18,
            total_pages: 2,
            has_more: true,
          },
        } as any;
      }

      expect(params).toMatchObject({
        page: 2,
        limit: 9,
      });

      return new Promise(() => {});
    });

    const { result } = renderHook(() => useProjects(), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(result.current.page).toBe(1);
    });

    act(() => {
      result.current.setPage(2);
    });

    await waitFor(() => {
      expect(getProjects).toHaveBeenCalledTimes(2);
    });

    await waitFor(() => {
      expect(result.current.page).toBe(2);
    });
  });
});
