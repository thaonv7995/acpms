import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ReactElement } from 'react';
import { CreateTaskModal } from '../../../components/modals/CreateTaskModal';
import { useProjects } from '../../../hooks/useProjects';
import { useSprints } from '../../../hooks/useSprints';
import { useProjectMembers } from '../../../hooks/useProjectMembers';
import { useProjectSettings } from '../../../hooks/useProjectSettings';
import { useSettings } from '../../../hooks/useSettings';
import { createTask } from '../../../api/tasks';
import { createTaskAttempt } from '../../../api/taskAttempts';
import { DEFAULT_PROJECT_SETTINGS } from '../../../api/projectSettings';

vi.mock('react-router-dom', async () => {
  const actual = await vi.importActual<typeof import('react-router-dom')>('react-router-dom');
  return {
    ...actual,
    useNavigate: () => vi.fn(),
  };
});

vi.mock('../../../components/modals/create-task/ProjectSelector', () => ({
  ProjectSelector: () => <div data-testid="project-selector">Project Selector</div>,
}));

vi.mock('../../../components/modals/create-task/TaskMetadataGrid', () => ({
  TaskMetadataGrid: () => <div data-testid="task-metadata-grid">Task Metadata</div>,
}));

vi.mock('../../../components/modals/create-task/AIDescriptionField', () => ({
  AIDescriptionField: ({
    value,
    onChange,
  }: {
    value: string;
    onChange: (value: string) => void;
  }) => (
    <textarea
      aria-label="Task description"
      value={value}
      onChange={(event) => onChange(event.target.value)}
    />
  ),
}));

vi.mock('../../../hooks/useProjects', () => ({
  useProjects: vi.fn(),
}));

vi.mock('../../../hooks/useSprints', () => ({
  useSprints: vi.fn(),
}));

vi.mock('../../../hooks/useProjectMembers', () => ({
  useProjectMembers: vi.fn(),
}));

vi.mock('../../../hooks/useProjectSettings', () => ({
  useProjectSettings: vi.fn(),
}));

vi.mock('../../../hooks/useSettings', () => ({
  useSettings: vi.fn(),
}));

vi.mock('../../../api/tasks', () => ({
  createTask: vi.fn(),
  updateTaskMetadata: vi.fn().mockResolvedValue(undefined),
  deleteTask: vi.fn().mockResolvedValue(undefined),
  getTaskAttachmentUploadUrl: vi.fn(),
}));

vi.mock('../../../api/taskAttempts', () => ({
  createTaskAttempt: vi.fn(),
}));

function renderCreateTaskModal(ui: ReactElement) {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
      },
    },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>{ui}</MemoryRouter>
    </QueryClientProvider>
  );
}

describe('CreateTaskModal repository access guard', () => {
  beforeEach(() => {
    vi.clearAllMocks();

    vi.mocked(useProjects).mockReturnValue({
      projects: [],
      apiProjects: [],
      loading: false,
      error: null,
      searchQuery: '',
      setSearchQuery: vi.fn(),
      filters: { status: [], techStack: [], hasAgent: null },
      setFilters: vi.fn(),
      filteredProjects: [],
      refetch: vi.fn(),
      page: 1,
      setPage: vi.fn(),
      totalPages: 1,
      totalCount: 0,
      hasMore: false,
    });

    vi.mocked(useSprints).mockReturnValue({
      sprints: [],
      loading: false,
      error: null,
      refreshSprints: vi.fn(),
      generateSprints: vi.fn(),
    });

    vi.mocked(useProjectMembers).mockReturnValue({
      members: [
        {
          id: 'member-1',
          name: 'Owner User',
          email: 'owner@example.com',
          roles: ['owner'],
        },
      ],
      setMembers: vi.fn(),
      loading: false,
      error: null,
      refetch: vi.fn(),
    });

    vi.mocked(useProjectSettings).mockReturnValue({
      settings: {
        ...DEFAULT_PROJECT_SETTINGS,
        require_review: true,
        auto_deploy: true,
        auto_execute: true,
      },
      defaults: {
        ...DEFAULT_PROJECT_SETTINGS,
      },
      loading: false,
      saving: false,
      error: null,
      isDirty: false,
      refetch: vi.fn(),
      updateSettings: vi.fn(),
      updateSetting: vi.fn(),
      resetToDefaults: vi.fn(),
    });

    vi.mocked(useSettings).mockReturnValue({
      settings: {
        gitlab: {
          url: 'https://gitlab.com',
          token: '••••••••••••••••••••',
          autoSync: true,
          configured: true,
        },
        agent: {
          provider: 'openai-codex',
        },
        openclaw: {
          gatewayEnabled: true,
        },
        cloudflare: {
          accountId: '',
          token: '',
          zoneId: '',
          baseDomain: '',
          configured: false,
        },
        notifications: {
          email: false,
          slack: false,
          slackWebhookUrl: '',
        },
        worktreesPath: './worktrees',
        preferredAgentLanguage: 'en',
      },
      loading: false,
      saving: false,
      testing: { claude: false, gitlab: false },
      error: null,
      refetch: vi.fn(),
      save: vi.fn(),
      testClaude: vi.fn(),
      testGitLab: vi.fn(),
    });

    vi.mocked(createTask).mockResolvedValue({
      id: 'task-1',
      project_id: 'project-1',
      title: 'Read-only task',
      description: '',
      task_type: 'feature',
      status: 'Todo',
      created_by: 'user-1',
      metadata: {},
      created_at: '2026-03-04T00:00:00Z',
      updated_at: '2026-03-04T00:00:00Z',
    } as any);
  });

  it('forces auto-start off and skips attempt creation for read-only repositories', async () => {
    const onClose = vi.fn();
    const onCreate = vi.fn();

    renderCreateTaskModal(
      <CreateTaskModal
        isOpen
        onClose={onClose}
        projectId="project-1"
        projectName="ACPMS"
        repositoryContext={{
          provider: 'github',
          access_mode: 'analysis_only',
          verification_status: 'verified',
          can_clone: true,
          can_push: false,
          can_open_change_request: false,
          can_merge: false,
          can_manage_webhooks: false,
          can_fork: true,
          upstream_repository_url: 'https://github.com/acme/app',
          effective_clone_url: 'https://github.com/acme/app',
          default_branch: 'main',
        }}
        navigateToProjectOnCreate={false}
        onCreate={onCreate}
      />
    );

    expect(screen.getByText('GitHub access is read-only')).toBeTruthy();
    expect(
      screen.getByText(
        'Link a writable fork or import a repository you can push to before starting coding attempts.'
      )
    ).toBeTruthy();

    const autoStartSwitch = screen.getByRole('switch', { name: /Auto start/i }) as HTMLButtonElement;
    await waitFor(() => {
      expect(autoStartSwitch.getAttribute('aria-checked')).toBe('false');
    });
    expect(autoStartSwitch.disabled).toBe(true);
    expect(
      screen.getByText('Disabled because this project is read-only for coding attempts.')
    ).toBeTruthy();

    fireEvent.change(
      screen.getByPlaceholderText('e.g. Implement refresh token rotation'),
      { target: { value: 'Ship read-only coverage' } }
    );

    fireEvent.click(screen.getByRole('button', { name: /Create Task/i }));

    await waitFor(() => {
      expect(createTask).toHaveBeenCalledTimes(1);
    });

    expect(createTaskAttempt).not.toHaveBeenCalled();
    expect(onCreate).toHaveBeenCalledWith(
      expect.objectContaining({
        projectId: 'project-1',
        taskId: 'task-1',
        autoStarted: false,
      })
    );
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('defaults review-first to the project require_review setting', async () => {
    renderCreateTaskModal(
      <CreateTaskModal
        isOpen
        onClose={vi.fn()}
        projectId="project-1"
        projectName="ACPMS"
        navigateToProjectOnCreate={false}
      />
    );

    const reviewFirstSwitch = screen.getByRole('switch', { name: /Review first/i });
    await waitFor(() => {
      expect(reviewFirstSwitch.getAttribute('aria-checked')).toBe('true');
    });
  });

  it('keeps task preview off when project settings do not enable it', async () => {
    vi.mocked(useProjectSettings).mockReturnValue({
      settings: {
        ...DEFAULT_PROJECT_SETTINGS,
        auto_deploy: false,
        preview_enabled: true,
      },
      defaults: {
        ...DEFAULT_PROJECT_SETTINGS,
      },
      loading: false,
      saving: false,
      error: null,
      isDirty: false,
      refetch: vi.fn(),
      updateSettings: vi.fn(),
      updateSetting: vi.fn(),
      resetToDefaults: vi.fn(),
    });

    renderCreateTaskModal(
      <CreateTaskModal
        isOpen
        onClose={vi.fn()}
        projectId="project-1"
        projectName="ACPMS"
        navigateToProjectOnCreate={false}
      />
    );

    const taskPreviewSwitch = screen.getByRole('switch', { name: /Task preview/i }) as HTMLButtonElement;

    await waitFor(() => {
      expect(taskPreviewSwitch.getAttribute('aria-checked')).toBe('false');
    });
    expect(taskPreviewSwitch.disabled).toBe(true);
    expect(
      screen.getByText('Disabled because Task Preview is off in Project Settings.')
    ).toBeTruthy();

    fireEvent.change(
      screen.getByPlaceholderText('e.g. Implement refresh token rotation'),
      { target: { value: 'Create task with preview disabled' } }
    );

    fireEvent.click(screen.getByRole('button', { name: /Create Task/i }));

    await waitFor(() => {
      expect(createTask).toHaveBeenCalledTimes(1);
    });

    expect(createTask).toHaveBeenCalledWith(
      expect.objectContaining({
        metadata: expect.objectContaining({
          execution: expect.objectContaining({
            auto_deploy: false,
          }),
        }),
      })
    );
  });

  it('shows setup-required dialog and blocks task creation when source control is not configured', async () => {
    vi.mocked(useSettings).mockReturnValue({
      settings: {
        gitlab: {
          url: 'https://gitlab.com',
          token: '',
          autoSync: true,
          configured: false,
        },
        agent: {
          provider: 'openai-codex',
        },
        openclaw: {
          gatewayEnabled: true,
        },
        cloudflare: {
          accountId: '',
          token: '',
          zoneId: '',
          baseDomain: '',
          configured: false,
        },
        notifications: {
          email: false,
          slack: false,
          slackWebhookUrl: '',
        },
        worktreesPath: './worktrees',
        preferredAgentLanguage: 'en',
      },
      loading: false,
      saving: false,
      testing: { claude: false, gitlab: false },
      error: null,
      refetch: vi.fn(),
      save: vi.fn(),
      testClaude: vi.fn(),
      testGitLab: vi.fn(),
    });

    renderCreateTaskModal(
      <CreateTaskModal
        isOpen
        onClose={vi.fn()}
        projectId="project-1"
        projectName="ACPMS"
        navigateToProjectOnCreate={false}
      />
    );

    fireEvent.change(
      screen.getByPlaceholderText('e.g. Implement refresh token rotation'),
      { target: { value: 'Ship guarded task creation' } }
    );

    fireEvent.click(screen.getByRole('button', { name: /Create Task/i }));

    expect(await screen.findByText('Source control setup required')).toBeTruthy();
    expect(createTask).not.toHaveBeenCalled();
    expect(createTaskAttempt).not.toHaveBeenCalled();
  });
});
