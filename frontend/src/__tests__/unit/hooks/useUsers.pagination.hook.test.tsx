import React from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useUsers } from '@/hooks/useUsers';
import { getUsersPage } from '@/api/users';
import { getCurrentUser } from '@/api/auth';

vi.mock('@/api/users', () => ({
  getUsersPage: vi.fn(),
}));

vi.mock('@/api/auth', () => ({
  getCurrentUser: vi.fn(() => null),
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

describe('useUsers pagination defaults', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(getCurrentUser).mockReturnValue(null);
    vi.mocked(getUsersPage).mockResolvedValue({
      success: true,
      code: '0000',
      message: 'ok',
      data: [],
      metadata: {
        page: 1,
        page_size: 10,
        total_count: 0,
        total_pages: 1,
        has_more: false,
        stats: {
          total: 0,
          active: 0,
          agents_paired: 0,
          pending: 0,
        },
      },
    });
  });

  it('requests the first users page with a default limit of ten', async () => {
    renderHook(() => useUsers(), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(getUsersPage).toHaveBeenCalledWith({
        page: 1,
        limit: 10,
        search: undefined,
        role: undefined,
        status: undefined,
      });
    });
  });
});
