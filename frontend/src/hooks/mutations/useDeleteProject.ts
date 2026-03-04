/**
 * useDeleteProject Mutation Hook - Orval-Generated Integration
 *
 * Wraps Orval-generated useDeleteProject with cache invalidation logic.
 * Invalidates projects list, dashboard stats, and removes project from cache.
 */

import { useQueryClient } from '@tanstack/react-query';
import { useDeleteProject as useDeleteProjectGenerated } from '../../api/generated/projects/projects';

export function useDeleteProject() {
    const queryClient = useQueryClient();

    return useDeleteProjectGenerated({
        mutation: {
            // On successful deletion, invalidate caches
            onSuccess: (_response, variables) => {
                // Invalidate projects list to refetch
                queryClient.invalidateQueries({ queryKey: ['/api/v1/projects'] });

                // Invalidate dashboard stats (project count changed)
                queryClient.invalidateQueries({ queryKey: ['/api/v1/dashboard'] });

                // Remove the specific project from cache
                queryClient.removeQueries({ queryKey: [`/api/v1/projects/${variables.id}`] });

                // Invalidate any related queries (tasks for this project, etc.)
                queryClient.invalidateQueries({ queryKey: ['/api/v1/tasks'] });
            },
        },
    });
}
