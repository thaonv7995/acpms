import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { CreateTaskModal } from '../../../components/modals/CreateTaskModal';
import { useProjects } from '../../../hooks/useProjects';
import { useSprints } from '../../../hooks/useSprints';
import { useProjectMembers } from '../../../hooks/useProjectMembers';
import { useProjectSettings } from '../../../hooks/useProjectSettings';
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

vi.mock('../../../api/tasks', () => ({
  createTask: vi.fn(),
  getTaskAttachmentUploadUrl: vi.fn(),
}));

vi.mock('../../../api/taskAttempts', () => ({
  createTaskAttempt: vi.fn(),
}));

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

    render(
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
});
