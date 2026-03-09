import { useState, useMemo, useEffect, type Dispatch, type SetStateAction } from 'react';
import { useQuery } from '@tanstack/react-query';
import { getProjects, type ProjectsQueryParams } from '../api/projects';
import type { ProjectListItem } from '../types/project';
import type { ProjectWithRepositoryContext } from '../types/repository';
import { getProjectStatusPresentation } from '../utils/projectSummary';
import { resolveTechStack } from '../utils/resolveTechStack';

interface UseProjectsResult {
    projects: ProjectListItem[];
    apiProjects: ProjectWithRepositoryContext[];
    loading: boolean;
    error: string | null;
    searchQuery: string;
    setSearchQuery: (query: string) => void;
    filters: ProjectFilters;
    setFilters: Dispatch<SetStateAction<ProjectFilters>>;
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
    const status = getProjectStatusPresentation(dto.summary);

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
        status: status.status,
        statusLabel: status.statusLabel,
        statusColor: status.statusColor,
        progress: status.progress,
        agentIcon: 'smart_toy',
        lastActivity: formatRelativeTime(dto.updated_at),
        agentCount: status.agentCount,
    };
}

function normalizeFilterToken(value: string): string {
    return value.toLowerCase().replace(/[^a-z0-9]/g, '');
}

function techStackMatchesFilter(projectTech: string, selectedTechFilters: string[]): boolean {
    const projectToken = normalizeFilterToken(projectTech);
    if (!projectToken) return false;

    return selectedTechFilters.some((selected) => {
        const selectedToken = normalizeFilterToken(selected);
        if (!selectedToken) return false;
        if (projectToken === selectedToken) return true;

        // Avoid noisy matches for very short tokens (e.g. "go" matching "mongodb").
        if (projectToken.length < 3 || selectedToken.length < 3) {
            return false;
        }

        return projectToken.includes(selectedToken) || selectedToken.includes(projectToken);
    });
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

    // Reset page on filter change
    useEffect(() => {
        setPage(1);
    }, [filters.status.join('|'), filters.techStack.join('|')]);

    const hasActiveClientFilters = filters.status.length > 0 || filters.techStack.length > 0;
    const effectiveQueryPage = hasActiveClientFilters ? 1 : page;
    const effectiveQueryLimit = hasActiveClientFilters ? Math.max(defaultLimit, 500) : defaultLimit;

    // Build query params
    const queryParams: ProjectsQueryParams = {
        page: effectiveQueryPage,
        limit: effectiveQueryLimit,
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
        queryKey: [
            '/api/v1/projects',
            effectiveQueryPage,
            effectiveQueryLimit,
            debouncedSearch,
            hasActiveClientFilters,
        ],
        queryFn: () => getProjects(queryParams),
        staleTime: 30 * 1000,
        refetchInterval: 60 * 1000,
    });

    const body = (response as any) ?? {};
    const apiProjects: ProjectWithRepositoryContext[] = Array.isArray(body?.data)
        ? body.data
        : Array.isArray(response)
            ? response
            : [];
    const metadata = (body?.metadata ?? {}) as Record<string, unknown>;

    const serverTotalPages: number = Number(metadata.total_pages) || 1;
    const serverTotalCount: number = Number(metadata.total_count) || apiProjects.length;
    const serverHasMore = !!metadata.has_more;

    // Map backend projects to UI format
    const projects = useMemo(() => {
        return apiProjects.map(mapProjectDtoToListItem);
    }, [apiProjects]);

    // Client-side filtering for status/tech stack
    const filteredProjectPool = useMemo(() => {
        let result = [...projects];

        // Status filter
        if (filters.status.length > 0) {
            const selectedStatuses = new Set(filters.status.map((status) => status.toLowerCase()));
            result = result.filter((project) => selectedStatuses.has(project.status.toLowerCase()));
        }

        // Tech stack filter
        if (filters.techStack.length > 0) {
            result = result.filter((project) =>
                project.techStack.some((tech) => techStackMatchesFilter(tech, filters.techStack))
            );
        }

        return result;
    }, [projects, filters]);

    const totalCount: number = hasActiveClientFilters ? filteredProjectPool.length : serverTotalCount;
    const totalPages: number = hasActiveClientFilters
        ? Math.max(1, Math.ceil(totalCount / defaultLimit))
        : serverTotalPages;
    const hasMore: boolean = hasActiveClientFilters ? page < totalPages : serverHasMore;

    const filteredProjects = useMemo(() => {
        if (!hasActiveClientFilters) {
            return filteredProjectPool;
        }

        const start = (page - 1) * defaultLimit;
        const end = start + defaultLimit;
        return filteredProjectPool.slice(start, end);
    }, [filteredProjectPool, hasActiveClientFilters, page, defaultLimit]);

    // Keep page in valid range after filter changes
    useEffect(() => {
        if (page > totalPages) {
            setPage(totalPages);
        }
    }, [page, totalPages]);

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
