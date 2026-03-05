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
  mapBackendStatusToFrontend,
  groupTasksIntoColumns,
  applyTaskFilters,
  type CreatedDateFilter,
  mapFrontendStatusToBackend,
} from '../mappers/taskMapper';
import { logger } from '@/lib/logger';
import { isBreakdownSupportTask } from '../utils/kanbanVisibility';
import {
  createKanbanColumns,
  getDefaultKanbanColumnConfig,
  isKanbanStatusVisible,
  normalizeKanbanColumnConfig,
  type KanbanColumnConfig,
} from '../utils/kanbanColumns';

interface KanbanFilters {
  agentOnly?: boolean;
  search?: string;
  createdDate?: CreatedDateFilter;
}

const KANBAN_REFETCH_INTERVAL_MS = 5000;
const KANBAN_IDLE_REFETCH_INTERVAL_MS = 30000;
const KANBAN_COLUMN_CONFIG_STORAGE_KEY = 'kanban.column-config.v1';

function isTaskActiveStatus(status: string | undefined): boolean {
  if (!status) return false;
  const normalized = status.toLowerCase();
  return normalized === 'inprogress' || normalized === 'in_progress' || normalized === 'running' || normalized === 'queued';
}

function hasActiveTasksInResponse(data: unknown): boolean {
  const tasks = (
    data as {
      data?: Array<{
        status?: string;
        has_in_progress_attempt?: boolean;
        title?: string;
        metadata?: unknown;
      }>;
    } | undefined
  )?.data;
  if (!Array.isArray(tasks)) return false;
  return tasks.some(
    (task) =>
      !isBreakdownSupportTask(task) &&
      (isTaskActiveStatus(task.status) || task.has_in_progress_attempt === true)
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
  /** Archive a single task */
  closeTask: (taskId: string) => Promise<void>;
  /** Archive all done tasks */
  closeAllDone: () => Promise<void>;
  /** Raw TaskDto[] from API — pass to useKanbanStats to avoid duplicate fetch */
  rawTasks: import('../api/generated/models').TaskDto[];
  columnConfig: KanbanColumnConfig;
  setColumnConfig: (next: Partial<KanbanColumnConfig>) => void;
}

export function useKanban(projectId?: string): UseKanbanResult {
  const [filters, setFiltersState] = useState<KanbanFilters>({});
  const [columnConfig, setColumnConfigState] = useState<KanbanColumnConfig>(() => {
    const defaults = getDefaultKanbanColumnConfig();
    if (typeof window === 'undefined') return defaults;
    try {
      const raw = window.localStorage.getItem(KANBAN_COLUMN_CONFIG_STORAGE_KEY);
      if (!raw) return defaults;
      const parsed = JSON.parse(raw) as Partial<KanbanColumnConfig>;
      return normalizeKanbanColumnConfig(parsed);
    } catch {
      return defaults;
    }
  });
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
  const visibleRawTasks = useMemo(() => {
    const tasks = tasksResponse?.data;
    if (!tasks || !Array.isArray(tasks) || tasks.length === 0) {
      return [];
    }

    return tasks.filter((task) => {
      if (isBreakdownSupportTask(task)) return false;
      return isKanbanStatusVisible(mapBackendStatusToFrontend(task.status), columnConfig);
    });
  }, [tasksResponse, columnConfig]);

  const columns = useMemo(() => {
    if (visibleRawTasks.length === 0) {
      return getEmptyColumns(columnConfig);
    }

    // Create projects map for project name lookup
    const projectsMap = new Map<string, { name: string }>();
    projects.forEach((project) => {
      projectsMap.set(project.id, { name: project.name });
    });

    // Map backend tasks to frontend format
    const kanbanTasks = mapBackendTasks(visibleRawTasks, undefined, projectsMap);

    // Apply filters
    const filteredTasks = applyTaskFilters(kanbanTasks, filters);

    // Group into columns
    return groupTasksIntoColumns(filteredTasks, columnConfig);
  }, [visibleRawTasks, filters, projects, columnConfig]);

  // Set filters handler
  const setFilters = useCallback((newFilters: KanbanFilters) => {
    setFiltersState(newFilters);
  }, []);

  const setColumnConfig = useCallback((next: Partial<KanbanColumnConfig>) => {
    setColumnConfigState((prev) => {
      const merged = normalizeKanbanColumnConfig({ ...prev, ...next });
      if (typeof window !== 'undefined') {
        window.localStorage.setItem(KANBAN_COLUMN_CONFIG_STORAGE_KEY, JSON.stringify(merged));
      }
      return merged;
    });
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
        'col-backlog': 'backlog',
        'col-todo': 'todo',
        'col-in-progress': 'in_progress',
        'col-in-review': 'in_review',
        'col-done': 'done',
        'col-closed': 'archived',
      };

      const newStatus = statusMap[columnId];
      if (newStatus) {
        await updateStatus(taskId, newStatus);
      }
    },
    [updateStatus]
  );

  // Archive a single task
  const closeTask = useCallback(
    async (taskId: string) => {
      await updateStatus(taskId, 'archived');
    },
    [updateStatus]
  );

  // Archive all done tasks
  const closeAllDone = useCallback(async () => {
    const doneColumn = columns.find((col) => col.status === 'done');
    if (!doneColumn || doneColumn.tasks.length === 0) return;

    const promises = doneColumn.tasks.map((task) =>
      updateStatusMutation.mutateAsync({
        id: task.id,
        data: { status: mapFrontendStatusToBackend('archived') },
      })
    );
    await Promise.all(promises);
    await refetch();
  }, [columns, updateStatusMutation, refetch]);

  return {
    columns,
    loading: isLoading,
    error: queryError ? 'Failed to load kanban board' : null,
    refetch,
    updateStatus,
    moveTaskToColumn,
    setFilters,
    filters,
    closeTask,
    closeAllDone,
    rawTasks: visibleRawTasks,
    columnConfig,
    setColumnConfig,
  };
}

/**
 * Get empty columns structure
 */
function getEmptyColumns(columnConfig?: Partial<KanbanColumnConfig>): KanbanColumn[] {
  return createKanbanColumns(columnConfig);
}
