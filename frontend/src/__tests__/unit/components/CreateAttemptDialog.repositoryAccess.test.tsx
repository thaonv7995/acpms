import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { MemoryRouter } from 'react-router-dom';
import { CreateAttemptDialog } from '../../../components/modals/CreateAttemptDialog';
import { useCreateTaskAttempt } from '../../../api/generated/task-attempts/task-attempts';
import { useSettings } from '../../../hooks/useSettings';

vi.mock('../../../api/generated/task-attempts/task-attempts', () => ({
  useCreateTaskAttempt: vi.fn(),
}));

vi.mock('../../../hooks/useSettings', () => ({
  useSettings: vi.fn(),
}));

describe('CreateAttemptDialog repository access guard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(useCreateTaskAttempt).mockReturnValue({
      mutateAsync: vi.fn(),
      isPending: false,
    } as any);
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
  });

  it('blocks starting a new attempt when repository access is read-only', () => {
    const onClose = vi.fn();
    const onSuccess = vi.fn();
    const mutateAsync = vi.fn();

    vi.mocked(useCreateTaskAttempt).mockReturnValue({
      mutateAsync,
      isPending: false,
    } as any);

    render(
      <MemoryRouter>
        <CreateAttemptDialog
          isOpen
          onClose={onClose}
          taskId="task-1"
          projectId="project-1"
          taskTitle="Implement guarded attempt flow"
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
          onSuccess={onSuccess}
        />
      </MemoryRouter>
    );

    expect(screen.getByText('GitHub access is read-only')).toBeTruthy();
    expect(
      screen.getByText(
        'Link a writable fork or import a repository you can push to before starting coding attempts.'
      )
    ).toBeTruthy();

    const blockedButton = screen.getByRole('button', { name: /Start Blocked/i }) as HTMLButtonElement;
    expect(blockedButton.disabled).toBe(true);

    fireEvent.click(blockedButton);

    expect(mutateAsync).not.toHaveBeenCalled();
    expect(onSuccess).not.toHaveBeenCalled();
  });

  it('shows setup-required dialog and blocks attempt creation when source control is not configured', async () => {
    const mutateAsync = vi.fn();

    vi.mocked(useCreateTaskAttempt).mockReturnValue({
      mutateAsync,
      isPending: false,
    } as any);
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

    render(
      <MemoryRouter>
        <CreateAttemptDialog
          isOpen
          onClose={vi.fn()}
          taskId="task-1"
          projectId="project-1"
          taskTitle="Implement guarded attempt flow"
          onSuccess={vi.fn()}
        />
      </MemoryRouter>
    );

    fireEvent.click(screen.getByRole('button', { name: /Setup Required/i }));

    expect(await screen.findByText('Source control setup required')).toBeTruthy();
    expect(mutateAsync).not.toHaveBeenCalled();
  });
});
