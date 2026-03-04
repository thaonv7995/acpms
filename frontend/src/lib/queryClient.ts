/**
 * TanStack Query (React Query) Configuration
 *
 * Centralized query client for automatic caching, background refetching,
 * and cache invalidation on mutations.
 */

import { QueryClient } from '@tanstack/react-query';

/**
 * Create QueryClient with global defaults
 *
 * Caching Strategy:
 * - staleTime: How long data is considered fresh (5 minutes)
 * - cacheTime: How long inactive data stays in cache (10 minutes)
 * - refetchOnWindowFocus: Disabled to avoid annoying refetches
 * - retry: Retry failed requests once (avoid overwhelming server)
 */
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // Data is fresh for 5 minutes (no refetch during this time)
      staleTime: 5 * 60 * 1000,

      // Inactive data stays in cache for 10 minutes before garbage collection
      gcTime: 10 * 60 * 1000,

      // Don't refetch when user switches tabs (annoying UX)
      refetchOnWindowFocus: false,

      // Retry failed requests once (network hiccups)
      retry: 1,

      // Exponential backoff: 1000ms delay before retry
      retryDelay: (attemptIndex) => Math.min(1000 * 2 ** attemptIndex, 30000),
    },
    mutations: {
      // Don't retry mutations (avoid duplicate operations like duplicate user creation)
      retry: 0,
    },
  },
});

/**
 * Query Keys Convention
 *
 * Use hierarchical keys for granular cache invalidation:
 *
 * @example
 * ['users'] - All users list
 * ['users', { role: 'developer' }] - Filtered users
 * ['user', userId] - Single user detail
 *
 * ['projects'] - All projects
 * ['projects', { status: 'active' }] - Filtered projects
 * ['project', projectId] - Single project
 *
 * ['tasks'] - All tasks
 * ['tasks', { projectId }] - Tasks for a project
 * ['task', taskId] - Single task
 *
 * ['dashboard'] - Dashboard stats
 * ['kanban'] - Kanban board data
 */
