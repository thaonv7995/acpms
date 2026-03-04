/**
 * useDashboard Hook - Orval-Generated API Client Integration
 *
 * Migrated from manual API calls to Orval-generated type-safe client
 * - Uses generated useGetDashboard hook from OpenAPI spec
 * - Type-safe API interactions with DashboardDataDoc
 * - Automatic caching with aggressive stale time (30 seconds)
 * - Background refetching for real-time feel
 */

import { useMemo } from 'react';
import { useGetDashboard } from '../api/generated/dashboard/dashboard';
import type { DashboardDataDoc } from '../api/generated/models';

interface UseDashboardResult extends Partial<DashboardDataDoc> {
    loading: boolean;
    error: string | null;
    refetch: () => void;
}

export function useDashboard(): UseDashboardResult {
    // Orval-generated useGetDashboard hook with React Query
    const {
        data: dashboardResponse,
        isLoading,
        error: queryError,
        refetch,
    } = useGetDashboard({
        query: {
            staleTime: 30 * 1000, // Fresh for 30 seconds (real-time feel)
            refetchInterval: 60 * 1000, // Auto-refetch every 60 seconds (background polling)
        },
    });

    // Extract data from response
    const data = useMemo(() => {
        return dashboardResponse?.data || null;
    }, [dashboardResponse]);

    return {
        stats: data?.stats,
        projects: data?.projects,
        agentLogs: data?.agentLogs,
        humanTasks: data?.humanTasks,
        loading: isLoading,
        error: queryError ? (queryError as Error).message : null,
        refetch: () => { refetch(); },
    };
}
