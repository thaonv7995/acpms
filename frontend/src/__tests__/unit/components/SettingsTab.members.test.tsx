import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SettingsTab } from '../../../components/project-detail/SettingsTab';
import { useProjectMembers } from '../../../hooks/useProjectMembers';
import { getCurrentUser, isSystemAdmin } from '../../../api/auth';

const membersPanelSpy = vi.fn();

vi.mock('react-router-dom', async () => {
  const actual = await vi.importActual<typeof import('react-router-dom')>('react-router-dom');
  return {
    ...actual,
    useNavigate: () => vi.fn(),
  };
});

vi.mock('@tanstack/react-query', () => ({
  useQueryClient: () => ({
    removeQueries: vi.fn(),
    invalidateQueries: vi.fn(),
  }),
}));

vi.mock('../../../components/projects/GitLabIntegrationSettings', () => ({
  GitLabIntegrationSettings: () => null,
}));

vi.mock('../../../components/project-detail/ProjectMembersPanel', () => ({
  ProjectMembersPanel: (props: { canManageMembers: boolean }) => {
    membersPanelSpy(props);
    return (
      <div data-testid="members-panel">
        {props.canManageMembers ? 'manageable' : 'read-only'}
      </div>
    );
  },
}));

vi.mock('../../../api/generated/projects/projects', () => ({
  useUpdateProject: () => ({
    isPending: false,
    mutate: vi.fn(),
  }),
}));

vi.mock('../../../hooks/useProjectMembers', () => ({
  useProjectMembers: vi.fn(),
}));

vi.mock('../../../api/auth', () => ({
  getCurrentUser: vi.fn(),
  isSystemAdmin: vi.fn(),
}));

vi.mock('../../../api/projects', () => ({
  deleteProject: vi.fn(),
  syncProjectRepository: vi.fn(),
}));

describe('SettingsTab member management access', () => {
  beforeEach(() => {
    vi.clearAllMocks();

    vi.mocked(useProjectMembers).mockReturnValue({
      members: [],
      setMembers: vi.fn(),
      loading: false,
      error: null,
      refetch: vi.fn(),
    });
  });

  it('allows system admins to manage members even when no visible members exist', () => {
    vi.mocked(getCurrentUser).mockReturnValue({
      id: 'admin-1',
      name: 'System Admin',
      email: 'admin@example.com',
      global_roles: ['admin'],
    } as any);
    vi.mocked(isSystemAdmin).mockReturnValue(true);

    render(
      <SettingsTab
        projectId="project-1"
        projectName="OpenClaw Project"
        requireReview={true}
        onRefresh={vi.fn()}
      />
    );

    expect(screen.getByTestId('members-panel').textContent).toContain('manageable');
    expect(membersPanelSpy).toHaveBeenCalledWith(
      expect.objectContaining({ canManageMembers: true })
    );
  });

  it('keeps member management read-only for non-admin users without owner membership', () => {
    vi.mocked(getCurrentUser).mockReturnValue({
      id: 'user-1',
      name: 'Regular User',
      email: 'user@example.com',
      global_roles: [],
    } as any);
    vi.mocked(isSystemAdmin).mockReturnValue(false);

    render(
      <SettingsTab
        projectId="project-1"
        projectName="OpenClaw Project"
        requireReview={true}
        onRefresh={vi.fn()}
      />
    );

    expect(screen.getByTestId('members-panel').textContent).toContain('read-only');
    expect(membersPanelSpy).toHaveBeenCalledWith(
      expect.objectContaining({ canManageMembers: false })
    );
  });
});
