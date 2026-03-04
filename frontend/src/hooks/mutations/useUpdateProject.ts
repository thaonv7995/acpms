/**
 * useUpdateProject Mutation Hook - Orval-Generated Integration
 *
 * Wraps Orval-generated useUpdateProject with cache invalidation and
 * optimistic updates for instant UI feedback.
 */

import { useQueryClient } from '@tanstack/react-query';
import { useUpdateProject as useUpdateProjectGenerated } from '../../api/generated/projects/projects';
import type { UpdateProjectRequestDoc, ProjectDto } from '../../api/generated/models';

export function useUpdateProject() {
    const queryClient = useQueryClient();

    return useUpdateProjectGenerated({
        mutation: {
            // Optimistic update: Update UI immediately before API call completes
            onMutate: async ({ id, data }) => {
                // Cancel outgoing refetches
                await queryClient.cancelQueries({ queryKey: ['/api/v1/projects'] });

                // Snapshot previous value
                const previousResponse = queryClient.getQueryData(['/api/v1/projects']);

                // Optimistically update projects list
                queryClient.setQueryData(['/api/v1/projects'], (old: any) => {
                    if (!old?.data) return old;
                    return {
                        ...old,
                        data: old.data.map((project: ProjectDto) =>
                            project.id === id ? { ...project, ...data } : project
                        ),
                    };
                });

                // Return context with previous value for rollback
                return { previousResponse };
            },

            // On success, invalidate to ensure data is in sync
            onSuccess: (_response, variables) => {
                queryClient.invalidateQueries({ queryKey: ['/api/v1/projects'] });
                queryClient.invalidateQueries({ queryKey: [`/api/v1/projects/${variables.id}`] });
                queryClient.invalidateQueries({ queryKey: ['/api/v1/dashboard'] });
            },

            // On error, rollback to previous value
            onError: (_err, _variables, context) => {
                if (context?.previousResponse) {
                    queryClient.setQueryData(['/api/v1/projects'], context.previousResponse);
                }
            },
        },
    });
}

// Re-export type for convenience
export type { UpdateProjectRequestDoc };
