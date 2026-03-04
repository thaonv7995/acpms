/**
 * useUsers Hook - Orval-Generated API Client Integration
 *
 * Migrated from manual API calls to Orval-generated type-safe clients
 * - Uses generated useListUsers hook from OpenAPI spec
 * - Type-safe API interactions with UserDto
 * - Client-side filtering and stats calculation
 */

import { useState, useCallback, useMemo } from 'react';
import { useListUsers } from '../api/generated/users/users';
import type { UserDto } from '../api/generated/models';
import { getCurrentUser } from '../api/auth';
import {
    mapBackendUser,
    calculateUserStats,
    applyUserFilters,
    type BackendUser,
} from '../mappers/userMapper';
import type { User, UserStats, UserStatus, SystemRole } from '../types/user';

interface UseUsersResult {
    users: User[];
    stats: UserStats | null;
    loading: boolean;
    error: string | null;
    refreshUsers: () => Promise<void>;
    filterByRole: (role: SystemRole | null) => void;
    filterByStatus: (status: UserStatus | null) => void;
    search: (query: string) => void;
}

/**
 * Map generated UserDto to frontend User using shared mapper logic.
 * Falls back to created_at when updated_at is unexpectedly missing at runtime.
 */
function mapUserDtoToUser(dto: UserDto): User {
    const backendUser: BackendUser = {
        id: dto.id,
        email: dto.email,
        name: dto.name,
        avatar_url: dto.avatar_url ?? null,
        gitlab_id: null,
        gitlab_username: dto.gitlab_username ?? null,
        global_roles: dto.global_roles?.length ? dto.global_roles : ['viewer'],
        created_at: dto.created_at,
        updated_at: dto.updated_at || dto.created_at,
    };

    return mapBackendUser(backendUser);
}

export function useUsers(): UseUsersResult {
    // Filter state (client-side filtering for MVP)
    const [roleFilter, setRoleFilter] = useState<SystemRole | null>(null);
    const [statusFilter, setStatusFilter] = useState<UserStatus | null>(null);
    const [searchQuery, setSearchQuery] = useState('');

    // Orval-generated useListUsers hook with React Query
    const {
        data: userListResponse,
        isLoading,
        error: queryError,
        refetch,
    } = useListUsers({
        query: {
            staleTime: 5 * 60 * 1000, // Fresh for 5 minutes
        },
    });

    // Extract and map users from response
    // customFetch returns full response body { success, code, data, ... }
    const allUsers = useMemo(() => {
        const userData = userListResponse?.data;
        if (!userData) return [];
        const currentUserId = getCurrentUser()?.id;
        return userData.map((dto): User => {
            const mapped = mapUserDtoToUser(dto);
            if (currentUserId && mapped.id === currentUserId) {
                return {
                    ...mapped,
                    status: 'active' as const,
                    lastActive: 'Just now',
                };
            }
            return mapped;
        });
    }, [userListResponse]);

    // Calculate stats from all users (memoized)
    const stats = useMemo(() => {
        if (allUsers.length === 0) return null;
        return calculateUserStats(allUsers);
    }, [allUsers]);

    // Apply filters to users (memoized)
    const users = useMemo(() => {
        return applyUserFilters(allUsers, {
            role: roleFilter || undefined,
            status: statusFilter || undefined,
            search: searchQuery || undefined,
        });
    }, [allUsers, roleFilter, statusFilter, searchQuery]);

    // Filter callbacks
    const filterByRole = useCallback((role: SystemRole | null) => {
        setRoleFilter(role);
    }, []);

    const filterByStatus = useCallback((status: UserStatus | null) => {
        setStatusFilter(status);
    }, []);

    const search = useCallback((query: string) => {
        setSearchQuery(query);
    }, []);

    // Refresh function (triggers refetch)
    const refreshUsers = useCallback(async () => {
        await refetch();
    }, [refetch]);

    return {
        users,
        stats,
        loading: isLoading,
        error: queryError ? (queryError as Error).message : null,
        refreshUsers,
        filterByRole,
        filterByStatus,
        search,
    };
}
