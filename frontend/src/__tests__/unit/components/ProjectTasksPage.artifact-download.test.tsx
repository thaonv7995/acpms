import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ProjectTasksPage } from '../../../pages/ProjectTasksPage';
import { useKanban } from '../../../hooks/useKanban';
import { useProjects } from '../../../hooks/useProjects';
import { useToast } from '../../../hooks/useToast';
import { usePreviewReadiness } from '../../../hooks/usePreviewReadiness';
import { useAttemptData } from '../../../pages/project-tasks/use-attempt-data';
import { useProjectTasksNavigation } from '../../../pages/project-tasks/use-project-tasks-navigation';
import { getAttemptArtifacts } from '../../../api/taskAttempts';

const mockNavigate = vi.fn();
const mockHandleModeChangeBase = vi.fn();

vi.mock('react-router-dom', async () => {
  const actual = await vi.importActual<typeof import('react-router-dom')>(
    'react-router-dom'
  );
  return {
    ...actual,
    useParams: () => ({
      projectId: 'project-1',
      taskId: 'task-1',
      attemptId: 'attempt-1',
    }),
    useSearchParams: () => [new URLSearchParams('view=preview'), vi.fn()],
    useLocation: () => ({
      pathname: '/tasks/projects/project-1/task-1/attempts/attempt-1',
      search: '?view=preview',
      hash: '',
      state: null,
      key: 'test',
    }),
    useNavigate: () => mockNavigate,
  };
});

vi.mock('@mui/material', async () => {
  const actual = await vi.importActual<typeof import('@mui/material')>(
    '@mui/material'
  );
  return {
    ...actual,
    useMediaQuery: () => false,
  };
});

vi.mock('../../../components/layout/AppShell', () => ({
  AppShell: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}));

vi.mock('../../../components/layout/TasksLayout', () => ({
  TasksLayout: ({ rightHeader }: { rightHeader: React.ReactNode }) => (
    <div data-testid="tasks-layout">{rightHeader}</div>
  ),
}));

vi.mock('../../../components/ui/new-card', () => ({
  NewCard: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  NewCardHeader: ({
    children,
    actions,
  }: {
    children: React.ReactNode;
    actions?: React.ReactNode;
  }) => (
    <div>
      <div>{children}</div>
      <div>{actions}</div>
    </div>
  ),
}));

vi.mock('../../../components/kanban/KanbanBoard', () => ({
  KanbanBoard: () => <div data-testid="kanban-board" />,
}));

vi.mock('../../../components/panels/TaskPanel', () => ({
  TaskPanel: () => <div data-testid="task-panel" />,
}));

vi.mock('../../../components/panels/TaskAttemptPanel', () => ({
  TaskAttemptPanel: ({
    children,
  }: {
    children: (args: {
      logs: React.ReactNode;
      followUp: React.ReactNode;
      isRunning: boolean;
    }) => React.ReactNode;
  }) => (
    <div data-testid="task-attempt-panel">
      {children({
        logs: <div data-testid="logs" />,
        followUp: <div data-testid="follow-up" />,
        isRunning: false,
      })}
    </div>
  ),
}));

vi.mock('../../../components/tasks/TodoPanel', () => ({
  TodoPanel: () => <div data-testid="todo-panel" />,
}));

vi.mock('../../../components/panels/GitErrorBanner', () => ({
  GitErrorBanner: () => null,
}));

vi.mock('../../../contexts/GitOperationsContext', () => ({
  GitOperationsProvider: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
}));

vi.mock('../../../components/diff-viewer', () => ({
  DiffViewer: () => <div data-testid="diff-viewer" />,
  invalidateDiffCache: vi.fn(),
}));

vi.mock('../../../components/modals', () => ({
  CreateTaskModal: () => null,
  EditTaskModal: () => null,
  TaskDetailModal: () => null,
  ConfigureAgentModal: () => null,
  CreateAttemptDialog: () => null,
  ConfirmModal: () => null,
}));

vi.mock('../../../contexts/RetryUiContext', () => ({
  RetryUiProvider: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock('../../../pages/project-tasks/preview-panel-wrapper', () => ({
  PreviewPanelWrapper: () => <div data-testid="preview-panel-wrapper" />,
}));

vi.mock('../../../hooks/useKanban', () => ({
  useKanban: vi.fn(),
}));

vi.mock('../../../hooks/useProjects', () => ({
  useProjects: vi.fn(),
}));

vi.mock('../../../hooks/useToast', () => ({
  useToast: vi.fn(),
}));

vi.mock('../../../hooks/usePreviewReadiness', () => ({
  usePreviewReadiness: vi.fn(),
}));

vi.mock('../../../pages/project-tasks/use-attempt-data', () => ({
  useAttemptData: vi.fn(),
}));

vi.mock('../../../pages/project-tasks/use-keyboard-shortcuts', () => ({
  useKeyboardShortcuts: vi.fn(),
}));

vi.mock('../../../pages/project-tasks/use-project-tasks-navigation', () => ({
  useProjectTasksNavigation: vi.fn(),
}));

vi.mock('../../../components/shared/Toast', () => ({
  Toast: () => null,
}));

vi.mock('../../../api/taskAttempts', () => ({
  createTaskAttempt: vi.fn(),
  cancelAttempt: vi.fn(),
  getTaskAttempts: vi.fn(),
  getAttemptArtifacts: vi.fn(),
}));

vi.mock('../../../api/tasks', () => ({
  deleteTask: vi.fn(),
}));

function setupMocks(projectType: 'desktop' | 'mobile' | 'extension', artifactUrl: string) {
  vi.mocked(useKanban).mockReturnValue({
    columns: [
      {
        id: 'col-in-progress',
        title: 'In Progress',
        status: 'in_progress',
        color: 'blue',
        tasks: [
          {
            id: 'task-1',
            title: 'Task 1',
            type: 'feature',
            status: 'in_progress',
            priority: 'medium',
            createdAt: '2026-03-04T00:00:00Z',
            latestAttemptId: 'attempt-1',
            projectId: 'project-1',
            metadata: {
              app_downloads: [
                {
                  label: `${projectType} artifact`,
                  attempt_id: 'attempt-1',
                  artifact_id: `${projectType}-artifact-1`,
                  artifact_key: `builds/${projectType}-qa.zip`,
                  artifact_type: `${projectType}_artifact`,
                  os: projectType === 'extension' ? 'browser' : projectType,
                },
              ],
            },
          },
        ],
      },
    ],
    loading: false,
    error: null,
    refetch: vi.fn().mockResolvedValue(undefined),
    updateStatus: vi.fn().mockResolvedValue(undefined),
    moveTaskToColumn: vi.fn().mockResolvedValue(undefined),
    closeTask: vi.fn().mockResolvedValue(undefined),
    columnConfig: {
      showClosed: false,
      showBacklog: true,
    },
    setFilters: vi.fn(),
    filters: {},
  } as any);

  vi.mocked(useProjects).mockReturnValue({
    projects: [
      {
        id: 'project-1',
        name: 'Project 1',
      },
    ],
    apiProjects: [
      {
        id: 'project-1',
        name: 'Project 1',
        description: 'Artifact preview delivery project',
        created_by: 'user-1',
        created_at: '2026-03-04T00:00:00Z',
        updated_at: '2026-03-04T00:00:00Z',
        project_type: projectType,
        settings: {
          require_review: true,
          auto_deploy: false,
          preview_enabled: true,
          auto_execute: false,
        },
      },
    ],
    loading: false,
    error: null,
    searchQuery: '',
    setSearchQuery: vi.fn(),
    filters: {
      status: [],
      techStack: [],
      hasAgent: null,
    },
    setFilters: vi.fn(),
    filteredProjects: [],
    refetch: vi.fn(),
    page: 1,
    setPage: vi.fn(),
    totalPages: 1,
    totalCount: 1,
    hasMore: false,
  } as any);

  vi.mocked(useToast).mockReturnValue({
    toasts: [],
    showToast: vi.fn(),
    hideToast: vi.fn(),
    clearToasts: vi.fn(),
  });

  vi.mocked(useAttemptData).mockReturnValue({
    sortedAttempts: [],
    selectedAttempt: {
      id: 'attempt-1',
      task_id: 'task-1',
      branch: 'feat/artifact-preview',
      status: 'completed',
      created_at: '2026-03-04T00:00:00Z',
      updated_at: '2026-03-04T00:00:00Z',
      metadata: {
        app_downloads: [
          {
            attempt_id: 'attempt-1',
            artifact_id: `${projectType}-artifact-1`,
            artifact_key: `builds/${projectType}-qa.zip`,
            artifact_type: `${projectType}_artifact`,
            os: projectType === 'extension' ? 'browser' : projectType,
            label: `${projectType} artifact`,
          },
        ],
      },
    },
    isAttemptsLoading: false,
  } as any);

  vi.mocked(getAttemptArtifacts).mockResolvedValue([
    {
      id: `${projectType}-artifact-1`,
      artifact_key: `builds/${projectType}-qa.zip`,
      artifact_type: `${projectType}_artifact`,
      size_bytes: 1024,
      file_count: 1,
      download_url: artifactUrl,
      created_at: '2026-03-04T00:00:00Z',
    },
  ]);

  vi.mocked(usePreviewReadiness).mockReturnValue({ readiness: null } as any);

  vi.mocked(useProjectTasksNavigation).mockReturnValue({
    handleTaskClick: vi.fn(),
    handleViewTaskDetails: vi.fn(),
    handleClosePanel: vi.fn(),
    handleModeChange: mockHandleModeChangeBase,
    handleAttemptSelect: vi.fn(),
    handleBackToTask: vi.fn(),
    handleCreateAttemptSuccess: vi.fn(),
  });
}

describe('ProjectTasksPage artifact preview delivery', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockHandleModeChangeBase.mockReset();
    mockNavigate.mockReset();
  });

  it.each(['desktop', 'mobile', 'extension'] as const)(
    'switches %s preview UI to artifact download action and uses artifact link',
    async (projectType) => {
      const artifactUrl = `https://cdn.example.com/builds/${projectType}-qa.zip`;
      setupMocks(projectType, artifactUrl);
      const queryClient = new QueryClient();

      render(
        <QueryClientProvider client={queryClient}>
          <ProjectTasksPage />
        </QueryClientProvider>
      );

      await waitFor(() => {
        expect(mockHandleModeChangeBase).toHaveBeenCalledWith(null);
      });

      expect(screen.queryByLabelText('Preview')).toBeNull();

      const downloadButton = screen.getByTitle(`${projectType} artifact`);
      const appendChildSpy = vi.spyOn(document.body, 'appendChild');
      const linkClickSpy = vi
        .spyOn(HTMLAnchorElement.prototype, 'click')
        .mockImplementation(() => {});

      fireEvent.click(downloadButton);

      await waitFor(() => {
        expect(getAttemptArtifacts).toHaveBeenCalledWith('attempt-1');
      });

      let anchorNode: HTMLAnchorElement | undefined;
      await waitFor(() => {
        anchorNode = appendChildSpy.mock.calls
          .map(([node]) => node)
          .find((node) => node instanceof HTMLAnchorElement) as
          | HTMLAnchorElement
          | undefined;
        expect(anchorNode).toBeTruthy();
      });

      expect(anchorNode?.href).toBe(artifactUrl);
      expect(anchorNode?.target).toBe('_blank');
      expect(anchorNode?.rel).toContain('noopener');
      expect(anchorNode?.rel).toContain('noreferrer');
      expect(linkClickSpy).toHaveBeenCalledTimes(1);

      appendChildSpy.mockRestore();
      linkClickSpy.mockRestore();
    }
  );
});
