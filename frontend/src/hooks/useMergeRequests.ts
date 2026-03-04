// Custom hook for Merge Requests data fetching
import { useState, useEffect, useCallback, useRef } from 'react';
import { getMergeRequestStats, getMergeRequests, syncGitLabMRs } from '../api/mergeRequests';
import { getProjects } from '../api/projects';
import type { MergeRequest, MRStats, MRStatus } from '../api/mergeRequests';

interface UseMergeRequestsResult {
    stats: MRStats | null;
    mergeRequests: MergeRequest[];
    loading: boolean;
    syncing: boolean;
    error: string | null;
    refetch: () => void;
    filterByStatus: (status: MRStatus | null) => void;
    search: (query: string) => void;
    syncWithGitLab: () => Promise<void>;
}

export function useMergeRequests(): UseMergeRequestsResult {
    const [stats, setStats] = useState<MRStats | null>(null);
    const [mergeRequests, setMergeRequests] = useState<MergeRequest[]>([]);
    const [loading, setLoading] = useState(true);
    const [syncing, setSyncing] = useState(false);
    const [error, setError] = useState<string | null>(null);

    // Filter state
    const [statusFilter, setStatusFilter] = useState<MRStatus | null>(null);
    const [searchQuery, setSearchQuery] = useState<string>('');

    const fetchList = useCallback(async () => {
        setLoading(true);
        setError(null);

        try {
            const mrsData = await getMergeRequests({
                status: statusFilter || undefined,
                search: searchQuery || undefined
            });
            setMergeRequests(mrsData);
        } catch (err) {
            setError(err instanceof Error ? err.message : 'Failed to load merge requests');
        } finally {
            setLoading(false);
        }
    }, [statusFilter, searchQuery]);

    const fetchAll = useCallback(async () => {
        setLoading(true);
        setError(null);

        try {
            const [statsData, mrsData] = await Promise.all([
                getMergeRequestStats(),
                getMergeRequests({
                    status: statusFilter || undefined,
                    search: searchQuery || undefined
                })
            ]);
            setStats(statsData);
            setMergeRequests(mrsData);
        } catch (err) {
            setError(err instanceof Error ? err.message : 'Failed to load merge requests');
        } finally {
            setLoading(false);
        }
    }, [statusFilter, searchQuery]);

    const filtersRef = useRef({ statusFilter, searchQuery });
    const isInitialMount = useRef(true);

    useEffect(() => {
        void fetchAll();
    }, []);

    useEffect(() => {
        if (isInitialMount.current) {
            isInitialMount.current = false;
            filtersRef.current = { statusFilter, searchQuery };
            return;
        }
        if (
            filtersRef.current.statusFilter === statusFilter &&
            filtersRef.current.searchQuery === searchQuery
        ) {
            return;
        }
        filtersRef.current = { statusFilter, searchQuery };
        void fetchList();
    }, [statusFilter, searchQuery, fetchList]);

    const filterByStatus = useCallback((status: MRStatus | null) => {
        setStatusFilter(status);
    }, []);

    const search = useCallback((query: string) => {
        setSearchQuery(query);
    }, []);

    const syncWithGitLab = useCallback(async () => {
        setSyncing(true);
        setError(null);

        try {
            const projectIds =
                mergeRequests.length > 0
                    ? [...new Set(mergeRequests.map((mr) => mr.projectId))]
                    : (await getProjects({ limit: 100 })).data?.map((p) => p.id) ?? [];
            await syncGitLabMRs(projectIds);
            await fetchAll();
        } catch (err) {
            setError(err instanceof Error ? err.message : 'Failed to sync with GitLab');
            throw err;
        } finally {
            setSyncing(false);
        }
    }, [fetchAll, mergeRequests]);

    return {
        stats,
        mergeRequests,
        loading,
        syncing,
        error,
        refetch: fetchAll,
        filterByStatus,
        search,
        syncWithGitLab,
    };
}
