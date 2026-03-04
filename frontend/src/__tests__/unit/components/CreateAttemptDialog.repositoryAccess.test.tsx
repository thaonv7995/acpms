import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { CreateAttemptDialog } from '../../../components/modals/CreateAttemptDialog';
import { useCreateTaskAttempt } from '../../../api/generated/task-attempts/task-attempts';

vi.mock('../../../api/generated/task-attempts/task-attempts', () => ({
  useCreateTaskAttempt: vi.fn(),
}));

describe('CreateAttemptDialog repository access guard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(useCreateTaskAttempt).mockReturnValue({
      mutateAsync: vi.fn(),
      isPending: false,
    } as any);
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
});
