/**
 * useTemplates Hook - React Query integration for project templates
 *
 * Provides:
 * - List all templates with optional filtering by project type
 * - Get single template details
 * - Caching and background refetching
 */

import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  listTemplates,
  getTemplate,
  type ProjectType,
  type ProjectTemplate,
} from '../api/templates';

// Query keys for cache management
export const templateKeys = {
  all: ['templates'] as const,
  lists: () => [...templateKeys.all, 'list'] as const,
  list: (projectType?: ProjectType) => [...templateKeys.lists(), { projectType }] as const,
  details: () => [...templateKeys.all, 'detail'] as const,
  detail: (id: string) => [...templateKeys.details(), id] as const,
};

interface UseTemplatesOptions {
  projectType?: ProjectType;
  enabled?: boolean;
}

interface UseTemplatesResult {
  templates: ProjectTemplate[];
  isLoading: boolean;
  error: Error | null;
  refetch: () => void;
}

/**
 * Hook to fetch and manage project templates list
 */
export function useTemplates(options: UseTemplatesOptions = {}): UseTemplatesResult {
  const { projectType, enabled = true } = options;

  const {
    data: templates = [],
    isLoading,
    error,
    refetch,
  } = useQuery({
    queryKey: templateKeys.list(projectType),
    queryFn: () => listTemplates(projectType),
    enabled,
    staleTime: 5 * 60 * 1000, // 5 minutes
    gcTime: 30 * 60 * 1000, // 30 minutes (formerly cacheTime)
  });

  return {
    templates,
    isLoading,
    error: error as Error | null,
    refetch,
  };
}

interface UseTemplateOptions {
  enabled?: boolean;
}

interface UseTemplateResult {
  template: ProjectTemplate | null;
  isLoading: boolean;
  error: Error | null;
}

/**
 * Hook to fetch a single template by ID
 */
export function useTemplate(id: string | null, options: UseTemplateOptions = {}): UseTemplateResult {
  const { enabled = true } = options;

  const {
    data: template = null,
    isLoading,
    error,
  } = useQuery({
    queryKey: templateKeys.detail(id || ''),
    queryFn: () => getTemplate(id!),
    enabled: enabled && !!id,
    staleTime: 5 * 60 * 1000,
  });

  return {
    template,
    isLoading,
    error: error as Error | null,
  };
}

/**
 * Hook to prefetch templates for faster navigation
 */
export function usePrefetchTemplates() {
  const queryClient = useQueryClient();

  const prefetchTemplates = (projectType?: ProjectType) => {
    queryClient.prefetchQuery({
      queryKey: templateKeys.list(projectType),
      queryFn: () => listTemplates(projectType),
      staleTime: 5 * 60 * 1000,
    });
  };

  return { prefetchTemplates };
}

/**
 * Hook to get templates grouped by project type
 */
export function useTemplatesByType(): {
  templatesByType: Record<ProjectType, ProjectTemplate[]>;
  isLoading: boolean;
  error: Error | null;
} {
  const { templates, isLoading, error } = useTemplates();

  const templatesByType = templates.reduce((acc, template) => {
    const type = template.project_type;
    if (!acc[type]) {
      acc[type] = [];
    }
    acc[type].push(template);
    return acc;
  }, {} as Record<ProjectType, ProjectTemplate[]>);

  return {
    templatesByType,
    isLoading,
    error,
  };
}
