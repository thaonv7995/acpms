/**
 * useDeleteUser Mutation Hook - Orval-Generated Integration
 *
 * Wraps Orval-generated useDeleteUser with cache invalidation logic.
 * Invalidates users list and dashboard stats after successful deletion.
 */

import { useQueryClient } from '@tanstack/react-query';
import { useDeleteUser as useDeleteUserGenerated } from '../../api/generated/users/users';

export function useDeleteUser() {
    const queryClient = useQueryClient();

    return useDeleteUserGenerated({
        mutation: {
            // On successful deletion, invalidate caches
            onSuccess: (_response, variables) => {
                // Invalidate the users list to refetch
                queryClient.invalidateQueries({ queryKey: ['/api/v1/users'] });

                // Invalidate dashboard stats (user count changed)
                queryClient.invalidateQueries({ queryKey: ['/api/v1/dashboard'] });

                // Remove the specific user from cache
                queryClient.removeQueries({ queryKey: [`/api/v1/users/${variables.id}`] });
            },
        },
    });
}
