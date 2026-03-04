import { useCallback } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import type { LayoutMode } from '../../components/layout/TasksLayout';
import type { KanbanTask } from '../../types/project';

/**
 * Custom hook for managing navigation in ProjectTasksPage
 */
export function useProjectTasksNavigation(projectId: string | undefined) {
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();

  const navigateWithSearch = useCallback(
    (pathname: string, options?: { replace?: boolean }) => {
      const search = searchParams.toString();
      navigate({ pathname, search: search ? `?${search}` : '' }, options);
    },
    [navigate, searchParams]
  );

  const handleTaskClick = useCallback(
    (task: KanbanTask) => {
      // Navigate to /attempts/latest which will auto-redirect to latest attempt when available
      // This matches vibe-kanban-reference behavior
      // Use task.projectId if projectId is undefined (when filtering all projects)
      const effectiveProjectId = projectId || task.projectId;
      if (effectiveProjectId) {
        navigateWithSearch(
          `/tasks/projects/${effectiveProjectId}/${task.id}/attempts/latest`
        );
      } else {
        // Fallback: navigate to task detail without project context
        navigateWithSearch(`/tasks/${task.id}`);
      }
    },
    [navigateWithSearch, projectId]
  );

  /** Navigate to task detail panel only (TaskPanel with attempts list), NOT attempt logs */
  const handleViewTaskDetails = useCallback(
    (task: KanbanTask) => {
      const effectiveProjectId = projectId || task.projectId;
      if (effectiveProjectId) {
        navigateWithSearch(`/tasks/projects/${effectiveProjectId}/${task.id}`);
      } else {
        navigateWithSearch(`/tasks/${task.id}`);
      }
    },
    [navigateWithSearch, projectId]
  );

  const handleClosePanel = useCallback(() => {
    if (projectId) {
      navigate(`/tasks/projects/${projectId}`);
    } else {
      navigate('/tasks/projects');
    }
  }, [navigate, projectId]);

  const handleModeChange = useCallback(
    (newMode: LayoutMode) => {
      const params = new URLSearchParams(searchParams);
      if (newMode === null) {
        params.delete('view');
      } else {
        params.set('view', newMode);
      }
      setSearchParams(params, { replace: true });
    },
    [searchParams, setSearchParams]
  );

  const handleAttemptSelect = useCallback(
    (taskId: string, selectedAttemptId: string) => {
      if (projectId) {
        navigateWithSearch(
          `/tasks/projects/${projectId}/${taskId}/attempts/${selectedAttemptId}`
        );
      } else {
        navigateWithSearch(`/tasks/${taskId}/attempts/${selectedAttemptId}`);
      }
    },
    [navigateWithSearch, projectId]
  );

  const handleBackToTask = useCallback(
    (taskId: string) => {
      if (projectId) {
        navigateWithSearch(`/tasks/projects/${projectId}/${taskId}`);
      } else {
        navigateWithSearch(`/tasks/${taskId}`);
      }
    },
    [navigateWithSearch, projectId]
  );

  const handleCreateAttemptSuccess = useCallback(
    (taskId: string, newAttemptId: string, projectIdOverride?: string) => {
      const effectiveProjectId = projectIdOverride ?? projectId;
      if (effectiveProjectId) {
        navigateWithSearch(
          `/tasks/projects/${effectiveProjectId}/${taskId}/attempts/${newAttemptId}`
        );
      } else {
        navigateWithSearch(`/tasks/${taskId}/attempts/${newAttemptId}`);
      }
    },
    [navigateWithSearch, projectId]
  );

  return {
    handleTaskClick,
    handleViewTaskDetails,
    handleClosePanel,
    handleModeChange,
    handleAttemptSelect,
    handleBackToTask,
    handleCreateAttemptSuccess,
  };
}
