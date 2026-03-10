/**
 * useUsers Hook - Server-side pagination for user management
 */

import { useState, useCallback, useEffect, useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { getCurrentUser } from '../api/auth';
import {
    mapBackendUser,
    type BackendUser,
} from '../mappers/userMapper';
import type { User, UserStats, UserStatus, SystemRole } from '../types/user';
import { filterHiddenServiceAccounts } from '@/lib/hiddenServiceAccounts';
import { getUsersPage, type UsersPageMetadata } from '../api/users';

export const USER_MANAGEMENT_PAGE_SIZE = 10;

interface UseUsersResult {
    users: User[];
    stats: UserStats | null;
    loading: boolean;
    error: string | null;
    refreshUsers: () => Promise<void>;
    filterByRole: (role: SystemRole | null) => void;
    filterByStatus: (status: UserStatus | null) => void;
    search: (query: string) => void;
    page: number;
    setPage: (page: number) => void;
    totalPages: number;
    totalCount: number;
}

export function useUsers(): UseUsersResult {
    const [roleFilter, setRoleFilter] = useState<SystemRole | null>(null);
    const [statusFilter, setStatusFilter] = useState<UserStatus | null>(null);
    const [searchQuery, setSearchQuery] = useState('');
    const [page, setPageState] = useState(1);
    const pageSize = USER_MANAGEMENT_PAGE_SIZE;

    const {
        data: response,
        isLoading,
        error: queryError,
        refetch,
    } = useQuery({
        queryKey: ['/api/v1/users', page, pageSize, searchQuery, roleFilter, statusFilter],
        queryFn: () =>
            getUsersPage({
                page,
                limit: pageSize,
                search: searchQuery || undefined,
                role: roleFilter || undefined,
                status: statusFilter || undefined,
            }),
        staleTime: 30 * 1000,
    });

    const allUsers = useMemo(() => {
        const userData = response?.data;
        if (!userData) return [];
        const currentUserId = getCurrentUser()?.id;
        return filterHiddenServiceAccounts(userData).map((dto): User => {
            const backendUser: BackendUser = {
                id: dto.id,
                email: dto.email,
                name: dto.name,
                avatar_url: dto.avatar_url ?? null,
                gitlab_id: dto.gitlab_id ?? null,
                gitlab_username: dto.gitlab_username ?? null,
                global_roles: dto.global_roles?.length ? dto.global_roles : ['viewer'],
                created_at: dto.created_at,
                updated_at: dto.updated_at || dto.created_at,
            };
            const mapped = mapBackendUser(backendUser);
            if (currentUserId && mapped.id === currentUserId) {
                return {
                    ...mapped,
                    status: 'active' as const,
                    lastActive: 'Just now',
                };
            }
            return mapped;
        });
    }, [response]);

    const metadata = useMemo(
        () => ((response?.metadata ?? {}) as UsersPageMetadata),
        [response]
    );

    const stats = useMemo(() => {
        const statsMetadata = metadata?.stats;
        if (statsMetadata) {
            return {
                total: statsMetadata.total,
                active: statsMetadata.active,
                agentsPaired: statsMetadata.agents_paired,
                pending: statsMetadata.pending,
            };
        }
        return null;
    }, [metadata]);

    const filterByRole = useCallback((role: SystemRole | null) => {
        setRoleFilter(role);
        setPageState(1);
    }, []);

    const filterByStatus = useCallback((status: UserStatus | null) => {
        setStatusFilter(status);
        setPageState(1);
    }, []);

    const search = useCallback((query: string) => {
        setSearchQuery(query);
        setPageState(1);
    }, []);

    const refreshUsers = useCallback(async () => {
        await refetch();
    }, [refetch]);

    const setPage = useCallback((nextPage: number) => {
        setPageState(Math.max(1, nextPage));
    }, []);

    useEffect(() => {
        const totalPages = Math.max(1, metadata.total_pages || 1);
        if (page > totalPages) {
            setPageState(totalPages);
        }
    }, [metadata.total_pages, page]);

    return {
        users: allUsers,
        stats,
        loading: isLoading,
        error: queryError ? (queryError as Error).message : null,
        refreshUsers,
        filterByRole,
        filterByStatus,
        search,
        page,
        setPage,
        totalPages: Math.max(1, metadata.total_pages || 1),
        totalCount: Number.isFinite(metadata.total_count) ? metadata.total_count : allUsers.length,
    };
}
