/**
 * useUpdateUser Mutation Hook - Orval-Generated Integration
 *
 * Wraps Orval-generated useUpdateUser with cache invalidation logic.
 * Uses type-safe UpdateUserRequest from OpenAPI spec.
 */

import { useQueryClient } from '@tanstack/react-query';
import { useUpdateUser as useUpdateUserGenerated } from '../../api/generated/users/users';
import type { UpdateUserRequest } from '../../api/generated/models';

export function useUpdateUser() {
    const queryClient = useQueryClient();

    return useUpdateUserGenerated({
        mutation: {
            // On successful update, invalidate caches
            onSuccess: (_response, variables) => {
                // Invalidate the users list to refetch
                queryClient.invalidateQueries({ queryKey: ['/api/v1/users'] });

                // Invalidate the specific user detail (if cached)
                queryClient.invalidateQueries({ queryKey: [`/api/v1/users/${variables.id}`] });
            },
        },
    });
}

// Re-export type for convenience
export type { UpdateUserRequest };
