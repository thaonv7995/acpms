/**
 * useKanban Hook - Kanban board data with real API integration
 *
 * Migrated from mock data to Orval-generated type-safe clients
 */

import { useState, useCallback, useMemo } from 'react';
import { useListTasks, useUpdateTaskStatus } from '../api/generated/tasks/tasks';
import type { ListTasksParams } from '../api/generated/models';
import { useProjects } from './useProjects';
import type { KanbanColumn, KanbanTask } from '../types/project';
import {
  mapBackendTasks,
  groupTasksIntoColumns,
  applyTaskFilters,
  type CreatedDateFilter,
  mapFrontendStatusToBackend,
} from '../mappers/taskMapper';
import { logger } from '@/lib/logger';

interface KanbanFilters {
  agentOnly?: boolean;
  search?: string;
  createdDate?: CreatedDateFilter;
}

const KANBAN_REFETCH_INTERVAL_MS = 5000;
const KANBAN_IDLE_REFETCH_INTERVAL_MS = 30000;

function isTaskActiveStatus(status: string | undefined): boolean {
  if (!status) return false;
  const normalized = status.toLowerCase();
  return normalized === 'inprogress' || normalized === 'in_progress' || normalized === 'running' || normalized === 'queued';
}

function hasActiveTasksInResponse(data: unknown): boolean {
  const tasks = (data as { data?: Array<{ status?: string; has_in_progress_attempt?: boolean }> } | undefined)?.data;
  if (!Array.isArray(tasks)) return false;
  return tasks.some(
    (task) =>
      isTaskActiveStatus(task.status) || task.has_in_progress_attempt === true
  );
}

function getKanbanRefetchInterval(query: { state: { data?: unknown } }): number {
  return hasActiveTasksInResponse(query.state.data)
    ? KANBAN_REFETCH_INTERVAL_MS
    : KANBAN_IDLE_REFETCH_INTERVAL_MS;
}

interface UseKanbanResult {
  columns: KanbanColumn[];
  loading: boolean;
  error: string | null;
  refetch: () => Promise<void>;
  updateStatus: (taskId: string, status: KanbanTask['status']) => Promise<void>;
  moveTaskToColumn: (taskId: string, columnId: string, position: number) => Promise<void>;
  setFilters: (filters: KanbanFilters) => void;
  filters: KanbanFilters;
  /** Raw TaskDto[] from API — pass to useKanbanStats to avoid duplicate fetch */
  rawTasks: import('../api/generated/models').TaskDto[];
}

export function useKanban(projectId?: string): UseKanbanResult {
  const [filters, setFiltersState] = useState<KanbanFilters>({});
  const { projects, loading: projectsLoading } = useProjects({ limit: 100 });

  // Determine if we should fetch all tasks (when projectId is 'all' or undefined)
  const isAllProjects = projectId === 'all' || !projectId;

  // When isAllProjects is true, we omit project_id, which triggers the backend to return all tasks for the user.
  const queryParams = isAllProjects ? {} : { project_id: projectId };

  const taskQuery = useListTasks(
    queryParams as ListTasksParams,
    {
      query: {
        staleTime: 30000,
        refetchInterval: getKanbanRefetchInterval,
        refetchIntervalInBackground: false,
      },
    }
  );

  const tasksResponse = taskQuery.data;
  const isLoading = projectsLoading || taskQuery.isLoading;
  const queryError = taskQuery.error;

  const refetch = useCallback(async () => {
    await taskQuery.refetch();
  }, [taskQuery]);

  // Update task status mutation
  const updateStatusMutation = useUpdateTaskStatus();

  // Transform backend tasks to frontend format and group into columns
  // customFetch returns full response body { success, code, data, ... }
  const columns = useMemo(() => {
    const tasks = tasksResponse?.data;
    if (!tasks || !Array.isArray(tasks) || tasks.length === 0) {
      return getEmptyColumns();
    }

    // Create projects map for project name lookup
    const projectsMap = new Map<string, { name: string }>();
    projects.forEach((project) => {
      projectsMap.set(project.id, { name: project.name });
    });

    // Map backend tasks to frontend format
    const kanbanTasks = mapBackendTasks(tasks, undefined, projectsMap);

    // Apply filters
    const filteredTasks = applyTaskFilters(kanbanTasks, filters);

    // Group into columns
    return groupTasksIntoColumns(filteredTasks);
  }, [tasksResponse, filters, projects]);

  // Set filters handler
  const setFilters = useCallback((newFilters: KanbanFilters) => {
    setFiltersState(newFilters);
  }, []);

  // Update task status
  const updateStatus = useCallback(
    async (taskId: string, status: KanbanTask['status']) => {
      try {
        await updateStatusMutation.mutateAsync({
          id: taskId,
          data: { status: mapFrontendStatusToBackend(status) },
        });
        // Refetch to get updated data
        await refetch();
      } catch (err) {
        logger.error('Failed to update task status:', err);
        throw err;
      }
    },
    [updateStatusMutation, refetch]
  );

  // Move task to column (same as update status for now)
  const moveTaskToColumn = useCallback(
    async (taskId: string, columnId: string, _position: number) => {
      // Extract status from column ID
      const statusMap: Record<string, KanbanTask['status']> = {
        'col-backlog': 'todo',
        'col-in-progress': 'in_progress',
        'col-in-review': 'in_review',
        'col-done': 'done',
      };

      const newStatus = statusMap[columnId];
      if (newStatus) {
        await updateStatus(taskId, newStatus);
      }
    },
    [updateStatus]
  );

  return {
    columns,
    loading: isLoading,
    error: queryError ? 'Failed to load kanban board' : null,
    refetch,
    updateStatus,
    moveTaskToColumn,
    setFilters,
    filters,
    rawTasks: (tasksResponse?.data as import('../api/generated/models').TaskDto[] | undefined) || [],
  };
}

/**
 * Get empty columns structure
 */
function getEmptyColumns(): KanbanColumn[] {
  return [
    { id: 'col-backlog', title: 'BACKLOG', status: 'todo', color: 'slate', tasks: [] },
    { id: 'col-in-progress', title: 'AGENT PROCESSING', status: 'in_progress', color: 'blue', tasks: [] },
    { id: 'col-in-review', title: 'IN REVIEW', status: 'in_review', color: 'yellow', tasks: [] },
    { id: 'col-done', title: 'COMPLETED', status: 'done', color: 'green', tasks: [] },
  ];
}
