import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ProjectDetailPage } from '../../../pages/ProjectDetailPage';
import { useProjectDetail } from '../../../hooks/useProjectDetail';
import { useProjectMembers } from '../../../hooks/useProjectMembers';
import { useProjectAssistant } from '../../../hooks/useProjectAssistant';
import {
  createProjectFork,
  recheckProjectRepositoryAccess,
} from '../../../api/projects';
import { getCurrentUser, isSystemAdmin } from '../../../api/auth';
import type { ProjectWithRepositoryContext, RepositoryContext } from '../../../types/repository';

const mockNavigate = vi.fn();

vi.mock('react-router-dom', async () => {
  const actual = await vi.importActual<typeof import('react-router-dom')>('react-router-dom');
  return {
    ...actual,
    useParams: () => ({ id: 'project-1' }),
    useNavigate: () => mockNavigate,
  };
});

vi.mock('../../../components/layout/AppShell', () => ({
  AppShell: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}));

vi.mock('../../../components/modals', () => ({
  CreateTaskModal: () => null,
  ViewLogsModal: () => null,
  RequirementFormModal: () => null,
  RequirementDetailModal: () => null,
}));

vi.mock('../../../components/project-assistant', () => ({
  FloatingChatButton: () => null,
  ProjectAssistantPanel: () => null,
}));

vi.mock('../../../components/project-detail', () => ({
  SummaryTab: () => <div data-testid="summary-tab">Summary</div>,
  TaskListTab: () => <div data-testid="task-list-tab">Tasks</div>,
  RequirementsTab: () => <div data-testid="requirements-tab">Requirements</div>,
  ArchitectureTab: () => <div data-testid="architecture-tab">Architecture</div>,
  DeploymentsTab: () => <div data-testid="deployments-tab">Deployments</div>,
  SettingsTab: () => <div data-testid="settings-tab">Settings</div>,
  SprintSelector: () => <div data-testid="sprint-selector">Sprint</div>,
}));

vi.mock('../../../components/common/ErrorBoundary', () => ({
  ErrorBoundary: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock('../../../hooks/useProjectDetail', () => ({
  useProjectDetail: vi.fn(),
}));

vi.mock('../../../hooks/useProjectMembers', () => ({
  useProjectMembers: vi.fn(),
}));

vi.mock('../../../hooks/useProjectAssistant', () => ({
  useProjectAssistant: vi.fn(),
}));

vi.mock('../../../api/projects', () => ({
  createProjectFork: vi.fn(),
  recheckProjectRepositoryAccess: vi.fn(),
}));

vi.mock('../../../api/taskAttempts', () => ({
  getTaskAttempts: vi.fn(),
}));

vi.mock('../../../api/requirements', () => ({
  updateRequirement: vi.fn(),
}));

vi.mock('../../../api/auth', () => ({
  getCurrentUser: vi.fn(),
  isSystemAdmin: vi.fn(),
}));

function makeRawProject(repositoryContext: RepositoryContext): ProjectWithRepositoryContext {
  return {
    id: 'project-1',
    name: 'ACPMS',
    description: 'Repository access test project',
    repository_url: repositoryContext.writable_repository_url
      || repositoryContext.upstream_repository_url
      || 'https://github.com/acme/app',
    metadata: {},
    require_review: true,
    project_type: 'web',
    created_by: 'user-1',
    created_at: '2026-03-04T00:00:00Z',
    updated_at: '2026-03-04T00:00:00Z',
    repository_context: repositoryContext,
  };
}

function seedProjectPage(repositoryContext: RepositoryContext) {
  const rawProject = makeRawProject(repositoryContext);

  vi.mocked(useProjectDetail).mockReturnValue({
    project: {
      id: 'project-1',
      name: 'ACPMS',
      repositoryUrl: rawProject.repository_url || 'https://github.com/acme/app',
      branch: 'main',
      status: 'active',
      lastDeploy: 'Never',
      stats: {
        activeAgents: 0,
        pendingReview: 0,
        criticalBugs: 0,
        buildStatus: 0,
      },
    },
    rawProject,
    kanbanColumns: [],
    tasks: [],
    rawTasks: [],
    requirements: [],
    taskStats: { total: 0, byType: {}, byStatus: {} },
    activeTab: 'summary',
    setActiveTab: vi.fn(),
    loading: false,
    error: null,
    refetch: vi.fn(),
    sprints: [],
    selectedSprintId: null,
    setSelectedSprintId: vi.fn(),
    activeSprint: null,
  });

  vi.mocked(useProjectMembers).mockReturnValue({
    members: [
      {
        id: 'user-1',
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

  vi.mocked(useProjectAssistant).mockReturnValue({
    session: null,
    messages: [],
    sessions: [],
    loading: false,
    error: null,
    agentActive: false,
    starting: false,
    createSession: vi.fn(),
    listSessions: vi.fn(),
    startAgent: vi.fn(),
    sendMessage: vi.fn(),
    refreshSession: vi.fn(),
    loadSession: vi.fn(),
    endSession: vi.fn(),
  });

  vi.mocked(getCurrentUser).mockReturnValue({
    id: 'user-1',
    name: 'Owner User',
    email: 'owner@example.com',
  } as any);
  vi.mocked(isSystemAdmin).mockReturnValue(false);
}

describe('ProjectDetailPage repository access status icon', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders a read-only status icon and exposes access actions in the modal', async () => {
    seedProjectPage({
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
    });

    vi.mocked(recheckProjectRepositoryAccess).mockResolvedValue({
      project: makeRawProject({
        provider: 'github',
        access_mode: 'direct_gitops',
        verification_status: 'verified',
        can_clone: true,
        can_push: true,
        can_open_change_request: true,
        can_merge: true,
        can_manage_webhooks: true,
        can_fork: true,
        writable_repository_url: 'https://github.com/acme/app',
        effective_clone_url: 'https://github.com/acme/app',
      }),
      recommended_action: 'Repository is ready for full GitOps workflow.',
      warnings: [],
    });
    vi.mocked(createProjectFork).mockResolvedValue({
      project: makeRawProject({
        provider: 'github',
        access_mode: 'fork_gitops',
        verification_status: 'verified',
        can_clone: true,
        can_push: true,
        can_open_change_request: true,
        can_merge: true,
        can_manage_webhooks: false,
        can_fork: true,
        upstream_repository_url: 'https://github.com/acme/app',
        writable_repository_url: 'https://github.com/me/app-fork',
        effective_clone_url: 'https://github.com/me/app-fork',
      }),
      created_repository_url: 'https://github.com/me/app-fork',
      recommended_action: 'Writable fork created successfully.',
      warnings: [],
    });

    render(<ProjectDetailPage />);

    const statusButton = screen.getByRole('button', {
      name: /Repository access status: Analysis Only/i,
    });
    expect(statusButton).toBeTruthy();
    expect(statusButton.title).toContain('GitHub access is read-only');
    expect(statusButton.querySelector('.text-red-500')).toBeTruthy();

    fireEvent.click(statusButton);

    expect(screen.getByText('Repository access')).toBeTruthy();
    expect(screen.getByText('Analysis Only')).toBeTruthy();
    expect(screen.getByText('Push blocked')).toBeTruthy();
    expect(screen.getByText('PR/MR blocked')).toBeTruthy();
    expect(screen.getByRole('button', { name: /Re-check access/i })).toBeTruthy();
    expect(screen.getByRole('button', { name: /Create fork automatically/i })).toBeTruthy();
    expect(screen.getByRole('button', { name: /Link writable fork/i })).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: /Re-check access/i }));

    await waitFor(() => {
      expect(recheckProjectRepositoryAccess).toHaveBeenCalledWith('project-1');
    });

    expect(screen.getAllByText('Repository is ready for full GitOps workflow.').length).toBeGreaterThan(0);

    fireEvent.click(screen.getByRole('button', { name: /Create fork automatically/i }));

    await waitFor(() => {
      expect(createProjectFork).toHaveBeenCalledWith('project-1');
    });

    expect(screen.getAllByText('Writable fork created successfully.').length).toBeGreaterThan(0);

    fireEvent.click(screen.getByRole('button', { name: /Link writable fork/i }));

    expect(screen.getByText('Link Existing Fork')).toBeTruthy();
    expect(screen.getAllByText('https://github.com/acme/app').length).toBeGreaterThan(0);
    expect(screen.getByDisplayValue('https://github.com/acme/app')).toBeTruthy();
  });

  it('renders a green status icon for fork-based gitops without upgrade CTAs', () => {
    seedProjectPage({
      provider: 'github',
      access_mode: 'fork_gitops',
      verification_status: 'verified',
      can_clone: true,
      can_push: true,
      can_open_change_request: true,
      can_merge: true,
      can_manage_webhooks: false,
      can_fork: true,
      upstream_repository_url: 'https://github.com/acme/app',
      writable_repository_url: 'https://github.com/me/app-fork',
      effective_clone_url: 'https://github.com/me/app-fork',
      default_branch: 'main',
    });

    render(<ProjectDetailPage />);

    const statusButton = screen.getByRole('button', {
      name: /Repository access status: Fork-based GitOps/i,
    });
    expect(statusButton).toBeTruthy();
    expect(statusButton.title).toContain('GitHub fork workflow ready');
    expect(statusButton.querySelector('.text-emerald-500')).toBeTruthy();

    fireEvent.click(statusButton);

    expect(screen.getByText('Fork-based GitOps')).toBeTruthy();
    expect(screen.getByText('Push enabled')).toBeTruthy();
    expect(screen.getByText('PR/MR enabled')).toBeTruthy();
    expect(screen.getByRole('button', { name: /Re-check access/i })).toBeTruthy();
    expect(screen.queryByRole('button', { name: /Create fork automatically/i })).toBeNull();
    expect(screen.queryByRole('button', { name: /Link writable fork/i })).toBeNull();
  });
});
