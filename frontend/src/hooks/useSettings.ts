/**
 * useSettings Hook - React Query Migration
 *
 * Migrated from useState + useEffect to useQuery/useMutation for:
 * - Automatic caching of settings
 * - Optimistic updates on save
 * - Automatic cache invalidation
 */

import { useState, useCallback } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getSettings, updateSettings, testClaudeConnection, testGitLabConnection } from '../api/settings';
import type { Settings } from '../api/settings';

interface UseSettingsResult {
    settings: Settings | null;
    loading: boolean;
    saving: boolean;
    testing: { claude: boolean; gitlab: boolean };
    error: string | null;
    refetch: () => void;
    save: (settings: Settings) => Promise<void>;
    testClaude: () => Promise<{ success: boolean; message: string }>;
    testGitLab: () => Promise<{ success: boolean; message: string }>;
}

export function useSettings(): UseSettingsResult {
    const queryClient = useQueryClient();
    const [testing, setTesting] = useState({ claude: false, gitlab: false });

    // React Query: Fetch settings with caching
    const {
        data: settings = null,
        isLoading,
        error: queryError,
        refetch,
    } = useQuery<Settings, Error>({
        queryKey: ['settings'],
        queryFn: getSettings,
        staleTime: 10 * 60 * 1000, // Fresh for 10 minutes (settings change infrequently)
    });

    // Mutation: Save settings with cache update
    const saveMutation = useMutation({
        mutationFn: updateSettings,
        onSuccess: (updatedSettings) => {
            // Update cache with new settings
            queryClient.setQueryData(['settings'], updatedSettings);
        },
    });

    const save = useCallback(async (newSettings: Settings) => {
        try {
            await saveMutation.mutateAsync(newSettings);
        } catch (err) {
            throw err;
        }
    }, [saveMutation]);

    const testClaude = useCallback(async () => {
        setTesting(prev => ({ ...prev, claude: true }));
        try {
            return await testClaudeConnection();
        } finally {
            setTesting(prev => ({ ...prev, claude: false }));
        }
    }, []);

    const testGitLab = useCallback(async () => {
        setTesting(prev => ({ ...prev, gitlab: true }));
        try {
            return await testGitLabConnection();
        } finally {
            setTesting(prev => ({ ...prev, gitlab: false }));
        }
    }, []);

    return {
        settings,
        loading: isLoading,
        saving: saveMutation.isPending,
        testing,
        error: queryError ? queryError.message : null,
        refetch: () => { refetch(); },
        save,
        testClaude,
        testGitLab,
    };
}
