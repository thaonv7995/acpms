import { useCallback, useMemo, useState, useEffect, useRef } from 'react';
import { useParams, useSearchParams, useLocation, useNavigate } from 'react-router-dom';
import { useMediaQuery } from '@mui/material';
import { AppShell } from '../components/layout/AppShell';
import { TasksLayout, type LayoutMode } from '../components/layout/TasksLayout';
import { NewCard } from '../components/ui/new-card';
import { KanbanBoard } from '../components/kanban/KanbanBoard';
import { TaskPanel } from '../components/panels/TaskPanel';
import { TaskAttemptPanel } from '../components/panels/TaskAttemptPanel';
import { TodoPanel } from '../components/tasks/TodoPanel';
import { GitErrorBanner } from '../components/panels/GitErrorBanner';
import { GitOperationsProvider } from '../contexts/GitOperationsContext';
import { DiffViewer, invalidateDiffCache } from '../components/diff-viewer';
import {
  CreateTaskModal,
  EditTaskModal,
  TaskDetailModal,
  ConfigureAgentModal,
  CreateAttemptDialog,
  ConfirmModal,
} from '../components/modals';
import { useKanban } from '../hooks/useKanban';
import { useProjects } from '../hooks/useProjects';
import { useToast } from '../hooks/useToast';
import { Toast } from '../components/shared/Toast';
import type { CreatedDateFilter } from '../mappers/taskMapper';
import type { KanbanTask } from '../types/project';
import { RetryUiProvider } from '../contexts/RetryUiContext';
import { useProjectTasksNavigation } from './project-tasks/use-project-tasks-navigation';
import { useAttemptData } from './project-tasks/use-attempt-data';
import { useKeyboardShortcuts } from './project-tasks/use-keyboard-shortcuts';
import { ProjectTasksHeader } from './project-tasks/project-tasks-header';
import { PreviewPanelWrapper } from './project-tasks/preview-panel-wrapper';
import { usePreviewReadiness } from '../hooks/usePreviewReadiness';
import { createTaskAttempt, cancelAttempt, getTaskAttempts, getAttempt } from '../api/taskAttempts';
import { deleteTask } from '../api/tasks';

type PreviewDeliveryKind = 'live_preview' | 'artifact_download' | 'unsupported';

interface ArtifactDownloadEntry {
  label: string;
  url: string;
}

function normalizeProjectType(projectType?: string): string {
  return typeof projectType === 'string' ? projectType.trim().toLowerCase() : '';
}

function getPreviewDeliveryKind(projectType?: string): PreviewDeliveryKind {
  switch (normalizeProjectType(projectType)) {
    case 'web':
    case 'api':
    case 'microservice':
      return 'live_preview';
    case 'desktop':
    case 'mobile':
    case 'extension':
      return 'artifact_download';
    default:
      return 'unsupported';
  }
}

function getPrimaryArtifactDownload(
  metadata?: Record<string, unknown>
): ArtifactDownloadEntry | null {
  if (!metadata) return null;

  const appDownloads = Array.isArray(metadata.app_downloads)
    ? metadata.app_downloads
    : [];

  for (const item of appDownloads) {
    if (!item || typeof item !== 'object') continue;
    const entry = item as Record<string, unknown>;
    const url =
      typeof entry.presigned_url === 'string'
        ? entry.presigned_url
        : typeof entry.url === 'string'
          ? entry.url
          : undefined;
    if (!url) continue;
    return {
      label: typeof entry.label === 'string' ? entry.label : 'Download',
      url,
    };
  }

  if (typeof metadata.app_download_url === 'string' && metadata.app_download_url) {
    return {
      label: 'Download',
      url: metadata.app_download_url,
    };
  }

  return null;
}

function triggerArtifactDownload(url: string): void {
  const link = document.createElement('a');
  link.href = url;
  link.target = '_blank';
  link.rel = 'noopener noreferrer';
  link.setAttribute('download', '');
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
}

/**
 * ProjectTasksPage - Unified task management page following vibe-kanban pattern.
 *
 * URL Structure:
 * - /projects/:projectId/tasks - Kanban view only
 * - /projects/:projectId/tasks/:taskId - Task detail panel
 * - /projects/:projectId/tasks/:taskId/attempts/:attemptId - Attempt detail with logs
 * - ?view=diffs - Show diffs panel in split view
 * - ?view=preview - Show preview panel in split view
 *
 * Panel Modes:
 * - mode=null: Kanban + Attempt logs (default)
 * - mode='preview': Attempt logs + Preview panel
 * - mode='diffs': Attempt logs + Diffs panel
 *
 * Key Features:
 * - VirtualizedLogList integration for performance
 * - AttemptSwitcher for navigating between attempts
 * - Full provider integration (RetryUi, Review)
 * - Responsive 3-panel layout (Kanban | Attempt | Aux)
 * - Keyboard shortcuts support
 */
export function ProjectTasksPage() {
  const { projectId, taskId, attemptId } = useParams();
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const location = useLocation();
  const isMobile = useMediaQuery('(max-width: 768px)');

  // Determine projectId from URL structure:
  // - /tasks -> redirect to /tasks/projects
  // - /tasks/projects -> 'all' (show all tasks)
  // - /tasks/projects/:projectId -> projectId from params
  // - /projects/:projectId/tasks (legacy) -> projectId from params
  const isTasksProjectsRoute = location.pathname.startsWith('/tasks/projects/');
  const isTasksProjectsRoot = location.pathname === '/tasks/projects';

  const effectiveProjectIdFromUrl = isTasksProjectsRoot
    ? 'all'  // /tasks/projects -> show all
    : isTasksProjectsRoute
      ? projectId  // projectId comes from /tasks/projects/:projectId
      : (location.pathname === '/tasks' || location.pathname.startsWith('/tasks?'))
        ? 'all'  // /tasks -> show all (will redirect)
        : projectId;  // Legacy /projects/:projectId/tasks

  // Modal states
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [editTask, setEditTask] = useState<{ task: KanbanTask; projectId: string } | null>(null);
  const [viewDetailsTask, setViewDetailsTask] = useState<{ task: KanbanTask; projectId: string } | null>(null);
  const [showAgentConfig, setShowAgentConfig] = useState(false);
  const [showCreateAttempt, setShowCreateAttempt] = useState(false);
  const [pendingDeleteTask, setPendingDeleteTask] = useState<{ id: string; title: string } | null>(null);
  const [isDeletingTask, setIsDeletingTask] = useState(false);
  const [pendingCloseTask, setPendingCloseTask] = useState<{ id: string; title: string } | null>(null);
  const [isClosingTask, setIsClosingTask] = useState(false);
  const lastStreamAttemptStatusRef = useRef<string | null>(null);
  const lastKanbanRefetchAtRef = useRef<number>(0);
  const KANBAN_REFETCH_THROTTLE_MS = 2000;
  const { toasts, showToast, hideToast } = useToast();

  // Local state for project filter (overrides URL projectId when "All Projects" is selected)
  // Track if we came from "All Projects" mode to keep filter when navigating to task detail
  const wasAllProjectsRef = useRef(isTasksProjectsRoot);

  useEffect(() => {
    // Track if we were on "All Projects" before navigation
    if (isTasksProjectsRoot) {
      wasAllProjectsRef.current = true;
    }
  }, [isTasksProjectsRoot]);

  // When navigating from "All Projects" to a task detail, keep filter as "all"
  // This prevents the filter from changing when clicking a task from /tasks/projects
  const [filterProjectId, setFilterProjectId] = useState<string | undefined>(() => {
    // Initialize: if we're on /tasks/projects, set filter to 'all'
    return isTasksProjectsRoot ? 'all' : undefined;
  });

  // Update filter when navigating from "All Projects" to task detail
  useEffect(() => {
    if (wasAllProjectsRef.current && taskId && !isTasksProjectsRoot) {
      // We came from "All Projects" and now viewing a task - keep filter as "all"
      setFilterProjectId('all');
    } else if (isTasksProjectsRoot) {
      // We're back on "All Projects" - clear filter to use URL
      setFilterProjectId('all');
    }
  }, [taskId, isTasksProjectsRoot]);

  // Layout mode from URL
  const rawMode = searchParams.get('view') as LayoutMode;
  const mode: LayoutMode =
    rawMode === 'preview' || rawMode === 'diffs' ? rawMode : null;

  // Fetch projects for dropdown (high limit to show full list, not just first page)
  const { projects, apiProjects } = useProjects({ limit: 500 });

  // Redirect /tasks to /tasks/projects
  useEffect(() => {
    if (location.pathname === '/tasks' && !location.pathname.includes('/projects')) {
      navigate('/tasks/projects', { replace: true });
    }
  }, [location.pathname, navigate]);

  // Determine which projectId to use for kanban filtering
  // Priority: filterProjectId (from dropdown) > effectiveProjectIdFromUrl (from URL or /tasks route)
  const effectiveProjectId = filterProjectId !== undefined ? filterProjectId : effectiveProjectIdFromUrl;
  const isAllProjects = !effectiveProjectId || effectiveProjectId === 'all';
  const kanbanProjectId = isAllProjects ? 'all' : effectiveProjectId;

  // Fetch kanban data
  const {
    columns,
    loading,
    refetch,
    setFilters,
    columnConfig,
    setColumnConfig,
    rawTasks,
    moveTaskToColumn,
    closeTask,
  } = useKanban(kanbanProjectId);

  // Handle project change - update filter and navigate if needed
  const handleProjectChange = useCallback(
    (newProjectId: string) => {
      if (newProjectId === 'all') {
        // Navigate to /tasks/projects to show all projects
        setFilterProjectId(undefined); // Clear filter to use URL
        if (location.pathname !== '/tasks/projects') {
          navigate('/tasks/projects', { replace: true });
        }
      } else {
        // Navigate to /tasks/projects/:projectId
        setFilterProjectId(undefined); // Clear filter to use URL
        if (effectiveProjectIdFromUrl !== newProjectId) {
          const basePath = `/tasks/projects/${newProjectId}`;
          if (taskId && attemptId) {
            navigate(`${basePath}/${taskId}/attempts/${attemptId}`, { replace: true });
          } else if (taskId) {
            navigate(`${basePath}/${taskId}`, { replace: true });
          } else {
            navigate(basePath, { replace: true });
          }
        }
      }
    },
    [navigate, effectiveProjectIdFromUrl, projectId, taskId, attemptId, location.pathname]
  );

  // Find selected task from columns
  const selectedTask = useMemo(() => {
    if (!taskId) return null;
    for (const column of columns) {
      const task = column.tasks.find((t) => t.id === taskId);
      if (task) return task;
    }
    return null;
  }, [columns, taskId]);

  // Fetch and manage attempt data
  // Use effectiveProjectIdFromUrl for attempt data (only fetch if not 'all')
  const attemptProjectId = effectiveProjectIdFromUrl === 'all' ? undefined : effectiveProjectIdFromUrl;
  const { selectedAttempt } = useAttemptData(
    taskId,
    attemptId,
    attemptProjectId
  );
  const selectedProject = useMemo(() => {
    const selectedProjectId = selectedTask?.projectId ?? projectId;
    return selectedProjectId
      ? apiProjects.find((project) => project.id === selectedProjectId)
      : undefined;
  }, [apiProjects, projectId, selectedTask?.projectId]);
  const previewDeliveryKind = useMemo(
    () => getPreviewDeliveryKind(selectedProject?.project_type),
    [selectedProject?.project_type]
  );
  const supportsLivePreview = previewDeliveryKind === 'live_preview';
  const usesArtifactDownloads = previewDeliveryKind === 'artifact_download';
  const displayMode: LayoutMode =
    supportsLivePreview || mode !== 'preview' ? mode : null;
  const { readiness: previewReadiness } = usePreviewReadiness(
    supportsLivePreview ? selectedAttempt?.id : undefined
  );
  const projectPreviewEnabled = Boolean(
    selectedProject?.settings?.auto_deploy || selectedProject?.settings?.preview_enabled
  );
  const primaryArtifactDownload = useMemo(
    () => getPrimaryArtifactDownload(selectedTask?.metadata),
    [selectedTask?.metadata]
  );
  const isLatestAttemptSelected = Boolean(
    selectedAttempt?.id &&
    selectedTask?.latestAttemptId &&
    selectedAttempt.id === selectedTask.latestAttemptId
  );
  const artifactDownloadDisabledReason = useMemo(() => {
    if (!usesArtifactDownloads) return undefined;
    if (!projectPreviewEnabled) {
      return 'Enable Preview/Auto-Deploy in project settings to generate downloadable preview artifacts after task completion.';
    }
    if (!selectedAttempt) {
      return 'Open an attempt to access its preview artifact.';
    }
    if (!isLatestAttemptSelected) {
      return 'Download is available only for the latest attempt because artifact links are stored at task level.';
    }
    if (!primaryArtifactDownload) {
      return 'Artifact will be available after this attempt completes and packaging succeeds.';
    }
    return undefined;
  }, [
    usesArtifactDownloads,
    projectPreviewEnabled,
    selectedAttempt,
    isLatestAttemptSelected,
    primaryArtifactDownload,
  ]);
  const artifactDownloadUrl =
    usesArtifactDownloads && !artifactDownloadDisabledReason
      ? primaryArtifactDownload?.url
      : undefined;

  // Navigation handlers
  // Use effectiveProjectIdFromUrl for navigation (only navigate if not 'all')
  const navigationProjectId = effectiveProjectIdFromUrl === 'all' ? undefined : effectiveProjectIdFromUrl;
  const {
    handleTaskClick,
    handleClosePanel,
    handleModeChange: handleModeChangeBase,
    handleAttemptSelect,
    handleBackToTask,
    handleCreateAttemptSuccess,
  } = useProjectTasksNavigation(navigationProjectId);

  const handleModeChange = useCallback(
    (newMode: LayoutMode) => {
      if (newMode === 'preview' && !supportsLivePreview) {
        return;
      }
      if (newMode === 'diffs' && selectedAttempt) {
        invalidateDiffCache(selectedAttempt.id);
      }
      handleModeChangeBase(newMode);
    },
    [handleModeChangeBase, selectedAttempt, supportsLivePreview]
  );

  useEffect(() => {
    if (mode === 'preview' && previewDeliveryKind === 'artifact_download') {
      handleModeChangeBase(null);
    }
  }, [handleModeChangeBase, mode, previewDeliveryKind]);

  const isTaskView = !!taskId && !attemptId;
  const isPanelOpen = Boolean(taskId && selectedTask);
  const selectedProjectRepositoryContext = useMemo(() => {
    if (!projectId) return undefined;
    return apiProjects.find((project) => project.id === projectId)?.repository_context;
  }, [apiProjects, projectId]);

  // Handlers with task context
  const handleAttemptSelectWithTask = useCallback(
    (attemptId: string) => {
      if (taskId) {
        handleAttemptSelect(taskId, attemptId);
      }
    },
    [taskId, handleAttemptSelect]
  );

  const handleBackToTaskWithContext = useCallback(() => {
    if (taskId) {
      handleBackToTask(taskId);
    }
  }, [taskId, handleBackToTask]);

  const handleCreateAttemptSuccessWithTask = useCallback(
    (newAttemptId: string) => {
      setShowCreateAttempt(false);
      if (taskId) {
        handleCreateAttemptSuccess(taskId, newAttemptId);
      }
    },
    [taskId, handleCreateAttemptSuccess]
  );

  const handleFiltersChange = useCallback(
    (filters: { agentOnly?: boolean; search?: string; createdDate?: CreatedDateFilter }) => {
      setFilters(filters);
    },
    [setFilters]
  );

  const handleTaskMoveFromKanban = useCallback(
    async (targetTaskId: string, targetColumnId: string) => {
      const sourceColumn = columns.find((column) =>
        column.tasks.some((candidate) => candidate.id === targetTaskId)
      );

      if (sourceColumn && sourceColumn.id === targetColumnId) {
        return;
      }

      try {
        await moveTaskToColumn(targetTaskId, targetColumnId, 0);
      } catch (error) {
        const message =
          error instanceof Error ? error.message : 'Failed to move task.';
        showToast(message, 'error');
        throw error;
      }
    },
    [columns, moveTaskToColumn, showToast]
  );

  const refreshKanbanAfterExecutionAction = useCallback(async () => {
    lastKanbanRefetchAtRef.current = Date.now();
    await refetch();
  }, [refetch]);

  const handleTaskCreated = useCallback(() => {
    void refreshKanbanAfterExecutionAction();
  }, [refreshKanbanAfterExecutionAction]);

  useEffect(() => {
    lastStreamAttemptStatusRef.current = null;
  }, [attemptId]);

  const handleAttemptStatusFromStream = useCallback(
    (status: string | null) => {
      if (!attemptId || !status) return;

      const normalized = status.toLowerCase();
      if (lastStreamAttemptStatusRef.current === normalized) return;
      lastStreamAttemptStatusRef.current = normalized;

      // Throttle: useKanban already polls every 5s when active. Avoid burst with user actions.
      const now = Date.now();
      if (now - lastKanbanRefetchAtRef.current < KANBAN_REFETCH_THROTTLE_MS) return;
      lastKanbanRefetchAtRef.current = now;

      // Keep task columns synced with execution lifecycle via SSE status updates.
      if (
        normalized === 'running' ||
        normalized === 'queued' ||
        normalized === 'success' ||
        normalized === 'failed' ||
        normalized === 'cancelled'
      ) {
        void refetch();
      }
    },
    [attemptId, refetch]
  );

  const findTaskById = useCallback(
    (targetTaskId: string) => {
      for (const column of columns) {
        const task = column.tasks.find((candidate) => candidate.id === targetTaskId);
        if (task) return task;
      }
      return null;
    },
    [columns]
  );

  const handleStartTaskFromKanban = useCallback(
    async (targetTaskId: string) => {
      await createTaskAttempt(targetTaskId);
      await refreshKanbanAfterExecutionAction();
    },
    [refreshKanbanAfterExecutionAction]
  );

  const resolveControllableAttemptId = useCallback(
    async (targetTaskId: string): Promise<string | null> => {
      const task = findTaskById(targetTaskId);
      if (task?.latestAttemptId) {
        try {
          const latestAttempt = await getAttempt(task.latestAttemptId);
          const status = latestAttempt.status.toLowerCase();
          if (status === 'running' || status === 'queued') {
            return latestAttempt.id;
          }
        } catch {
          // Fallback to listing attempts below
        }
      }

      const attempts = await getTaskAttempts(targetTaskId);
      if (!attempts.length) {
        return null;
      }

      const sortedAttempts = [...attempts].sort((a, b) => {
        const dateA = new Date(a.created_at).getTime();
        const dateB = new Date(b.created_at).getTime();
        return dateB - dateA;
      });

      const runningAttempt = sortedAttempts.find((attempt) => {
        const status = attempt.status.toLowerCase();
        return status === 'running' || status === 'queued';
      });

      return runningAttempt?.id || null;
    },
    [findTaskById]
  );

  const handleCancelExecutionFromKanban = useCallback(
    async (targetTaskId: string) => {
      const attemptId = await resolveControllableAttemptId(targetTaskId);
      if (!attemptId) {
        showToast('No active run to cancel.', 'info');
        await refreshKanbanAfterExecutionAction();
        return;
      }

      try {
        await cancelAttempt(attemptId);
        showToast('Cancellation requested.', 'info');
      } catch (error) {
        const message =
          error instanceof Error ? error.message : 'Failed to cancel current run.';
        showToast(message, 'error');
      } finally {
        await refreshKanbanAfterExecutionAction();
      }
    },
    [resolveControllableAttemptId, refreshKanbanAfterExecutionAction, showToast]
  );

  const handleViewDetailsFromKanban = useCallback(
    (targetTaskId: string) => {
      const task = findTaskById(targetTaskId);
      if (!task) return;
      const projId = task.projectId ?? (effectiveProjectIdFromUrl === 'all' ? undefined : effectiveProjectIdFromUrl);
      if (projId) {
        setViewDetailsTask({ task, projectId: projId });
      }
    },
    [findTaskById, effectiveProjectIdFromUrl]
  );

  const handleEditFromKanban = useCallback(
    (targetTaskId: string) => {
      const task = findTaskById(targetTaskId);
      if (!task) return;
      const projId = task.projectId ?? (effectiveProjectIdFromUrl === 'all' ? undefined : effectiveProjectIdFromUrl);
      if (projId) {
        setEditTask({ task, projectId: projId });
      }
    },
    [findTaskById, effectiveProjectIdFromUrl]
  );

  const handleNewAttemptFromKanban = useCallback(
    (targetTaskId: string) => {
      const task = findTaskById(targetTaskId);
      if (task) handleTaskClick(task);
    },
    [findTaskById, handleTaskClick]
  );

  const handleRetryFromKanban = useCallback(
    async (targetTaskId: string) => {
      await createTaskAttempt(targetTaskId);
      await refreshKanbanAfterExecutionAction();
    },
    [refreshKanbanAfterExecutionAction]
  );

  const handleDeleteTaskFromKanban = useCallback(
    (targetTaskId: string) => {
      const task = findTaskById(targetTaskId);
      setPendingDeleteTask({
        id: targetTaskId,
        title: task?.title || 'this task',
      });
    },
    [findTaskById]
  );

  const handleCloseTaskFromKanban = useCallback(
    (targetTaskId: string) => {
      const task = findTaskById(targetTaskId);
      setPendingCloseTask({
        id: targetTaskId,
        title: task?.title || 'this task',
      });
    },
    [findTaskById]
  );

  const handleConfirmCloseTask = useCallback(async () => {
    const taskToClose = pendingCloseTask;
    if (!taskToClose || isClosingTask) return;

    setIsClosingTask(true);
    try {
      await closeTask(taskToClose.id);
      if (!columnConfig.showClosed && taskToClose.id === taskId) {
        handleClosePanel();
      }
      setPendingCloseTask(null);
    } finally {
      setIsClosingTask(false);
    }
  }, [pendingCloseTask, isClosingTask, closeTask, columnConfig.showClosed, taskId, handleClosePanel]);

  const handleCloseTaskConfirmModal = useCallback(() => {
    if (isClosingTask) return;
    setPendingCloseTask(null);
  }, [isClosingTask]);

  const handleConfirmDeleteTask = useCallback(async () => {
    const taskToDelete = pendingDeleteTask;
    if (!taskToDelete || isDeletingTask) return;

    setIsDeletingTask(true);
    try {
      await deleteTask(taskToDelete.id);
      await refreshKanbanAfterExecutionAction();

      if (taskToDelete.id === taskId) {
        handleClosePanel();
      }

      setPendingDeleteTask(null);
    } finally {
      setIsDeletingTask(false);
    }
  }, [pendingDeleteTask, isDeletingTask, refreshKanbanAfterExecutionAction, taskId, handleClosePanel]);

  const handleCloseDeleteModal = useCallback(() => {
    if (isDeletingTask) return;
    setPendingDeleteTask(null);
  }, [isDeletingTask]);

  const handleCreateAttemptFromHeader = useCallback(() => {
    if (!selectedTask) return;
    setShowCreateAttempt(true);
  }, [selectedTask]);

  const handleOpenGitActionsFromHeader = useCallback(() => {
    if (!selectedAttempt) return;
    handleModeChange('diffs');
  }, [selectedAttempt, handleModeChange]);

  const handleDeleteTaskFromHeader = useCallback(() => {
    if (!selectedTask) return;
    setPendingDeleteTask({
      id: selectedTask.id,
      title: selectedTask.title || 'this task',
    });
  }, [selectedTask]);

  const handleDownloadArtifactFromHeader = useCallback(() => {
    if (!artifactDownloadUrl) return;
    triggerArtifactDownload(artifactDownloadUrl);
  }, [artifactDownloadUrl]);

  // Cycle mode handler for keyboard shortcut
  const handleCycleMode = useCallback(() => {
    const order: LayoutMode[] = supportsLivePreview ? [null, 'preview', 'diffs'] : [null, 'diffs'];
    const idx = order.indexOf(displayMode);
    const next = order[(idx + 1) % order.length];
    handleModeChange(next);
  }, [displayMode, handleModeChange, supportsLivePreview]);

  // Keyboard shortcuts
  useKeyboardShortcuts({
    isPanelOpen,
    mode: displayMode,
    onCreateTask: () => setShowCreateModal(true),
    onClosePanel: handleClosePanel,
    onCycleMode: handleCycleMode,
  });

  // Render kanban content
  const kanbanContent = (
    <KanbanBoard
      columns={columns}
      loading={loading}
      onTaskClick={handleTaskClick}
      onCreateTask={() => setShowCreateModal(true)}
      onFiltersChange={handleFiltersChange}
      onTaskMove={handleTaskMoveFromKanban}
      onTaskStart={handleStartTaskFromKanban}
      onTaskCancelExecution={handleCancelExecutionFromKanban}
      onTaskDelete={handleDeleteTaskFromKanban}
      onTaskViewDetails={handleViewDetailsFromKanban}
      onTaskEdit={handleEditFromKanban}
      onTaskNewAttempt={handleNewAttemptFromKanban}
      onTaskRetry={handleRetryFromKanban}
      onTaskClose={handleCloseTaskFromKanban}
      columnConfig={columnConfig}
      onColumnConfigChange={setColumnConfig}
      projects={projects.map(p => ({ id: p.id, name: p.name }))}
      selectedProjectId={filterProjectId !== undefined ? filterProjectId : (isAllProjects ? 'all' : effectiveProjectIdFromUrl)}
      onProjectChange={handleProjectChange}
      rawTasks={rawTasks}
    />
  );

  // Render attempt/task content with providers
  const attemptContent = selectedTask ? (
    <RetryUiProvider>
      <NewCard className="h-full min-h-0 flex flex-col bg-diagonal-lines bg-muted border-0">
        {isTaskView ? (
          <TaskPanel
            task={selectedTask}
            projectId={projectId!}
            onAttemptSelect={handleAttemptSelectWithTask}
            onCreateAttempt={() => setShowCreateAttempt(true)}
          />
        ) : (
          <>
            {/* Attempt Panel with logs */}
            <TaskAttemptPanel
              task={selectedTask}
              attempt={selectedAttempt}
              onAttemptStatusChange={handleAttemptStatusFromStream}
            >
              {({ logs, followUp, isRunning }) => (
                <>
                  <GitErrorBanner />
                  <div className="flex-1 min-h-0 flex flex-col">
                    <div className="flex-1 min-h-0 flex flex-col">
                      <div className="mx-auto w-full max-w-[72rem] h-full min-h-0">
                        {logs}
                      </div>
                    </div>

                    <div className="shrink-0 border-t border-border">
                      <div className="mx-auto w-full max-w-[50rem]">
                        <TodoPanel />
                      </div>
                    </div>

                    {!isRunning && (
                      <div className="min-h-0 max-h-[50%] border-t border-border overflow-hidden bg-background">
                        <div className="mx-auto w-full max-w-[50rem] h-full min-h-0">
                          {followUp}
                        </div>
                      </div>
                    )}
                  </div>
                </>
              )}
            </TaskAttemptPanel>
          </>
        )}
      </NewCard>
    </RetryUiProvider>
  ) : null;

  // Render aux content (diffs/preview)
  const auxContent =
    selectedAttempt && displayMode === 'diffs' ? (
      <DiffViewer
        attemptId={selectedAttempt.id}
        taskTitle={selectedTask?.title}
        onApproveSuccess={(message) => showToast(message, 'success')}
        onActionComplete={refreshKanbanAfterExecutionAction}
      />
    ) : displayMode === 'preview' && selectedTask && selectedAttempt ? (
      <PreviewPanelWrapper
        taskId={selectedTask.id}
        attemptId={selectedAttempt.id}
      />
    ) : null;

  // Render right header with breadcrumbs and actions
  const rightHeader = selectedTask ? (
    <ProjectTasksHeader
      selectedTask={selectedTask}
      selectedAttempt={selectedAttempt}
      mode={displayMode}
      isTaskView={isTaskView}
      onModeChange={handleModeChange}
      onBackToTask={handleBackToTaskWithContext}
      onClose={handleClosePanel}
      previewModeDisabled={Boolean(selectedAttempt && previewReadiness && !previewReadiness.ready)}
      previewModeDisabledReason={previewReadiness?.reason || undefined}
      downloadArtifactUrl={artifactDownloadUrl}
      downloadArtifactLabel={primaryArtifactDownload?.label}
      downloadDisabled={Boolean(usesArtifactDownloads && artifactDownloadDisabledReason)}
      downloadDisabledReason={artifactDownloadDisabledReason}
      onDownloadArtifact={handleDownloadArtifactFromHeader}
      onCreateAttempt={handleCreateAttemptFromHeader}
      onOpenGitActions={handleOpenGitActionsFromHeader}
      onDeleteTask={handleDeleteTaskFromHeader}
    />
  ) : null;

  return (
    <AppShell>
      <GitOperationsProvider attemptId={selectedAttempt?.id}>
        <TasksLayout
          kanban={kanbanContent}
          attempt={attemptContent}
          aux={auxContent}
          isPanelOpen={isPanelOpen}
          mode={displayMode}
          isMobile={isMobile}
          rightHeader={rightHeader}
        />
      </GitOperationsProvider>

      {/* View Details Modal (read-only, same style as Edit) */}
      {viewDetailsTask && (
        <TaskDetailModal
          isOpen={!!viewDetailsTask}
          onClose={() => setViewDetailsTask(null)}
          task={viewDetailsTask.task}
          projectId={viewDetailsTask.projectId}
          onEdit={() => {
            setViewDetailsTask(null);
            setEditTask({ task: viewDetailsTask.task, projectId: viewDetailsTask.projectId });
          }}
        />
      )}

      {/* Edit Task Modal */}
      {editTask && (
        <EditTaskModal
          isOpen={!!editTask}
          onClose={() => setEditTask(null)}
          task={editTask.task}
          projectId={editTask.projectId}
          onSuccess={(newAttemptId) => {
            void refreshKanbanAfterExecutionAction();
            if (newAttemptId) {
              handleCreateAttemptSuccess(editTask.task.id, newAttemptId, editTask.projectId);
            }
          }}
        />
      )}

      {/* Create Task Modal */}
      {showCreateModal && (
        <CreateTaskModal
          isOpen={showCreateModal}
          onClose={() => setShowCreateModal(false)}
          navigateToProjectOnCreate={false}
          repositoryContext={selectedProjectRepositoryContext}
          onCreate={handleTaskCreated}
        />
      )}

      {/* Configure Agent Modal */}
      {selectedTask && (
        <ConfigureAgentModal
          isOpen={showAgentConfig}
          onClose={() => setShowAgentConfig(false)}
          taskId={selectedTask.id}
          taskTitle={selectedTask.title}
        />
      )}

      {/* Create Attempt Dialog */}
      {selectedTask && taskId && projectId && (
        <CreateAttemptDialog
          isOpen={showCreateAttempt}
          onClose={() => setShowCreateAttempt(false)}
          taskId={taskId}
          projectId={projectId}
          taskTitle={selectedTask.title}
          repositoryContext={selectedProjectRepositoryContext}
          onSuccess={handleCreateAttemptSuccessWithTask}
        />
      )}

      <ConfirmModal
        isOpen={!!pendingDeleteTask}
        onClose={handleCloseDeleteModal}
        onConfirm={handleConfirmDeleteTask}
        title="Delete Task"
        message={`Delete "${pendingDeleteTask?.title ?? ''}"? This action cannot be undone.`}
        confirmText="Delete Task"
        confirmVariant="danger"
        isLoading={isDeletingTask}
      />

      <ConfirmModal
        isOpen={!!pendingCloseTask}
        onClose={handleCloseTaskConfirmModal}
        onConfirm={handleConfirmCloseTask}
        title="Close Task"
        message={`Move "${pendingCloseTask?.title ?? ''}" to Closed?`}
        confirmText="Close Task"
        confirmVariant="primary"
        isLoading={isClosingTask}
      />

      {/* Toast notifications */}
      {toasts.map((toast) => (
        <Toast
          key={toast.id}
          message={toast.message}
          type={toast.type}
          onClose={() => hideToast(toast.id)}
        />
      ))}
    </AppShell>
  );
}
