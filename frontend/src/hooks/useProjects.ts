import { useState, useMemo, useEffect } from 'react';
import { useQuery } from '@tanstack/react-query';
import { getProjects, type ProjectsQueryParams } from '../api/projects';
import type { ProjectListItem } from '../types/project';
import type { ProjectWithRepositoryContext } from '../types/repository';
import { resolveTechStack } from '../utils/resolveTechStack';

interface UseProjectsResult {
    projects: ProjectListItem[];
    apiProjects: ProjectWithRepositoryContext[];
    loading: boolean;
    error: string | null;
    searchQuery: string;
    setSearchQuery: (query: string) => void;
    filters: ProjectFilters;
    setFilters: (filters: ProjectFilters) => void;
    filteredProjects: ProjectListItem[];
    refetch: () => void;
    page: number;
    setPage: (page: number) => void;
    totalPages: number;
    totalCount: number;
    hasMore: boolean;
}

export interface ProjectFilters {
    status: string[];
    techStack: string[];
    hasAgent: boolean | null;
}

const defaultFilters: ProjectFilters = {
    status: [],
    techStack: [],
    hasAgent: null,
};

// ... (keep the same mapping utilities)

function asObject(value: unknown): Record<string, unknown> {
    if (typeof value === 'string') {
        try {
            const parsed = JSON.parse(value);
            if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
                return parsed as Record<string, unknown>;
            }
        } catch {
            return {};
        }
    }

    return value && typeof value === 'object' && !Array.isArray(value)
        ? (value as Record<string, unknown>)
        : {};
}

function formatRelativeTime(isoDate: string | undefined): string {
    if (!isoDate) return 'Just now';
    const date = new Date(isoDate);
    if (Number.isNaN(date.getTime())) return 'Just now';
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / (1000 * 60));
    const diffHours = Math.floor(diffMins / 60);
    const diffDays = Math.floor(diffHours / 24);
    if (diffMins < 1) return 'Just now';
    if (diffMins < 60) return `${diffMins}m ago`;
    if (diffHours < 24) return `${diffHours}h ago`;
    if (diffDays < 7) return `${diffDays}d ago`;
    return date.toLocaleDateString();
}

function mapProjectDtoToListItem(dto: ProjectWithRepositoryContext): ProjectListItem {
    const metadata = asObject(dto.metadata);

    return {
        id: dto.id,
        name: dto.name,
        description: dto.description || '',
        icon: typeof metadata.icon === 'string' ? metadata.icon : 'folder',
        iconColor:
            metadata.iconColor === 'orange' ||
                metadata.iconColor === 'blue' ||
                metadata.iconColor === 'emerald' ||
                metadata.iconColor === 'purple' ||
                metadata.iconColor === 'primary'
                ? metadata.iconColor
                : 'blue',
        techStack: resolveTechStack(dto as any),
        status:
            metadata.status === 'agent_reviewing' ||
                metadata.status === 'active_coding' ||
                metadata.status === 'deploying' ||
                metadata.status === 'completed' ||
                metadata.status === 'paused'
                ? metadata.status
                : 'active_coding',
        statusLabel: typeof metadata.statusLabel === 'string' ? metadata.statusLabel : 'Active',
        statusColor:
            metadata.statusColor === 'yellow' ||
                metadata.statusColor === 'blue' ||
                metadata.statusColor === 'emerald' ||
                metadata.statusColor === 'green' ||
                metadata.statusColor === 'slate'
                ? metadata.statusColor
                : 'blue',
        progress: typeof metadata.progress === 'number' ? metadata.progress : 0,
        agentIcon: 'smart_toy',
        lastActivity: formatRelativeTime(dto.updated_at),
        agentCount: typeof metadata.agentCount === 'number' ? metadata.agentCount : 0,
    };
}

export function useProjects(options?: { limit?: number }): UseProjectsResult {
    const defaultLimit = options?.limit || 9;

    const [searchQuery, setSearchQuery] = useState('');
    const [debouncedSearch, setDebouncedSearch] = useState('');
    const [filters, setFilters] = useState<ProjectFilters>(defaultFilters);
    const [page, setPage] = useState(1);

    // Debounce search query
    useEffect(() => {
        const timer = setTimeout(() => {
            setDebouncedSearch(searchQuery);
            setPage(1); // Reset page on search change
        }, 300);
        return () => clearTimeout(timer);
    }, [searchQuery]);

    // Build query params
    const queryParams: ProjectsQueryParams = {
        page,
        limit: defaultLimit,
    };
    if (debouncedSearch.trim()) {
        queryParams.search = debouncedSearch.trim();
    }

    // Manual fetching with react-query
    const {
        data: response,
        isLoading,
        error: queryError,
        refetch,
    } = useQuery({
        queryKey: ['/api/v1/projects', page, defaultLimit, debouncedSearch],
        queryFn: () => getProjects(queryParams),
        staleTime: 5 * 60 * 1000,
    });

    const body = (response as any) ?? {};
    const apiProjects: ProjectWithRepositoryContext[] = Array.isArray(body?.data)
        ? body.data
        : Array.isArray(response)
            ? response
            : [];
    const metadata = (body?.metadata ?? {}) as Record<string, unknown>;

    const totalPages: number = Number(metadata.total_pages) || 1;
    const totalCount: number = Number(metadata.total_count) || 0;
    const hasMore = !!metadata.has_more;

    // Map backend projects to UI format
    const projects = useMemo(() => {
        return apiProjects.map(mapProjectDtoToListItem);
    }, [apiProjects]);

    // Client-side filtering for status/tech stack
    const filteredProjects = useMemo(() => {
        let result = [...projects];

        // Status filter
        if (filters.status.length > 0) {
            result = result.filter(p => filters.status.includes(p.status));
        }

        // Tech stack filter
        if (filters.techStack.length > 0) {
            result = result.filter(p =>
                p.techStack.some((tech: string) => filters.techStack.includes(tech))
            );
        }

        return result;
    }, [projects, filters]);

    // Reset page if filtering empty results
    useEffect(() => {
        if (filteredProjects.length === 0 && page > 1 && !isLoading) {
            setPage(1);
        }
    }, [filteredProjects.length, page, isLoading]);

    return {
        projects,
        apiProjects,
        loading: isLoading,
        error: queryError ? (queryError as Error).message : null,
        searchQuery,
        setSearchQuery,
        filters,
        setFilters,
        filteredProjects,
        refetch,
        page,
        setPage,
        totalPages,
        totalCount,
        hasMore,
    };
}
