/**
 * useCreateProject Mutation Hook - Orval-Generated Integration
 *
 * Wraps Orval-generated useCreateProject with cache invalidation logic.
 * Uses type-safe CreateProjectRequestDoc from OpenAPI spec.
 */

import { useQueryClient } from '@tanstack/react-query';
import { useCreateProject as useCreateProjectGenerated } from '../../api/generated/projects/projects';
import type { CreateProjectRequestDoc } from '../../api/generated/models';

export function useCreateProject() {
    const queryClient = useQueryClient();

    return useCreateProjectGenerated({
        mutation: {
            // On successful creation, invalidate caches
            onSuccess: () => {
                // Invalidate projects list to refetch
                queryClient.invalidateQueries({ queryKey: ['/api/v1/projects'] });

                // Invalidate dashboard stats (project count changed)
                queryClient.invalidateQueries({ queryKey: ['/api/v1/dashboard'] });
            },
        },
    });
}

// Re-export type for convenience
export type { CreateProjectRequestDoc };
