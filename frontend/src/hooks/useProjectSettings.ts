/**
 * useProjectSettings Hook - React Query integration for project settings
 *
 * Provides:
 * - Automatic caching of project settings
 * - Optimistic updates on save
 * - Reset to defaults functionality
 * - Individual setting updates via PATCH
 */

import { useCallback } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
    getProjectSettings,
    updateProjectSettings,
    patchProjectSetting,
    DEFAULT_PROJECT_SETTINGS,
} from '../api/projectSettings';
import type {
    ProjectSettings,
    ProjectSettingsResponse,
    UpdateProjectSettingsRequest,
} from '../api/projectSettings';

interface UseProjectSettingsOptions {
    projectId: string;
    enabled?: boolean;
}

interface UseProjectSettingsResult {
    settings: ProjectSettings | null;
    defaults: ProjectSettings;
    loading: boolean;
    saving: boolean;
    error: string | null;
    isDirty: boolean;
    refetch: () => void;
    updateSettings: (settings: UpdateProjectSettingsRequest) => Promise<void>;
    updateSetting: <K extends keyof ProjectSettings>(key: K, value: ProjectSettings[K]) => Promise<void>;
    resetToDefaults: () => Promise<void>;
}

export function useProjectSettings({ projectId, enabled = true }: UseProjectSettingsOptions): UseProjectSettingsResult {
    const queryClient = useQueryClient();
    const queryKey = ['projectSettings', projectId];

    // Fetch project settings
    const {
        data,
        isLoading,
        error: queryError,
        refetch,
    } = useQuery<ProjectSettingsResponse, Error>({
        queryKey,
        queryFn: () => getProjectSettings(projectId),
        enabled: enabled && !!projectId,
        staleTime: 5 * 60 * 1000, // Fresh for 5 minutes
    });

    // Mutation: Update all settings
    const updateMutation = useMutation({
        mutationFn: (settings: UpdateProjectSettingsRequest) =>
            updateProjectSettings(projectId, settings),
        onMutate: async (newSettings) => {
            // Cancel outgoing refetches
            await queryClient.cancelQueries({ queryKey });

            // Snapshot previous value
            const previousData = queryClient.getQueryData<ProjectSettingsResponse>(queryKey);

            // Optimistically update
            if (previousData) {
                queryClient.setQueryData<ProjectSettingsResponse>(queryKey, {
                    ...previousData,
                    settings: { ...previousData.settings, ...newSettings },
                });
            }

            return { previousData };
        },
        onSuccess: (response) => {
            queryClient.setQueryData(queryKey, response);
            // Invalidate project query to reflect settings changes
            queryClient.invalidateQueries({ queryKey: ['/api/v1/projects', projectId] });
        },
        onError: (_err, _variables, context) => {
            // Rollback on error
            if (context?.previousData) {
                queryClient.setQueryData(queryKey, context.previousData);
            }
        },
    });

    // Mutation: Patch single setting
    const patchMutation = useMutation({
        mutationFn: ({ key, value }: { key: keyof ProjectSettings; value: unknown }) =>
            patchProjectSetting(projectId, key, value),
        onMutate: async ({ key, value }) => {
            await queryClient.cancelQueries({ queryKey });

            const previousData = queryClient.getQueryData<ProjectSettingsResponse>(queryKey);

            if (previousData) {
                queryClient.setQueryData<ProjectSettingsResponse>(queryKey, {
                    ...previousData,
                    settings: { ...previousData.settings, [key]: value },
                });
            }

            return { previousData };
        },
        onSuccess: (response) => {
            queryClient.setQueryData(queryKey, response);
            queryClient.invalidateQueries({ queryKey: ['/api/v1/projects', projectId] });
        },
        onError: (_err, _variables, context) => {
            if (context?.previousData) {
                queryClient.setQueryData(queryKey, context.previousData);
            }
        },
    });

    // Update multiple settings at once
    const updateSettings = useCallback(
        async (settings: UpdateProjectSettingsRequest) => {
            await updateMutation.mutateAsync(settings);
        },
        [updateMutation]
    );

    // Update a single setting
    const updateSetting = useCallback(
        async <K extends keyof ProjectSettings>(key: K, value: ProjectSettings[K]) => {
            await patchMutation.mutateAsync({ key, value });
        },
        [patchMutation]
    );

    // Reset all settings to defaults
    const resetToDefaults = useCallback(async () => {
        const defaults = data?.defaults || DEFAULT_PROJECT_SETTINGS;
        await updateMutation.mutateAsync(defaults);
    }, [updateMutation, data?.defaults]);

    // Check if current settings differ from defaults
    const isDirty = data
        ? JSON.stringify(data.settings) !== JSON.stringify(data.defaults || DEFAULT_PROJECT_SETTINGS)
        : false;

    return {
        settings: data?.settings || null,
        defaults: data?.defaults || DEFAULT_PROJECT_SETTINGS,
        loading: isLoading,
        saving: updateMutation.isPending || patchMutation.isPending,
        error: queryError ? queryError.message : null,
        isDirty,
        refetch: () => { refetch(); },
        updateSettings,
        updateSetting,
        resetToDefaults,
    };
}

// Re-export types for convenience
export type { ProjectSettings, UpdateProjectSettingsRequest };
export { DEFAULT_PROJECT_SETTINGS };
