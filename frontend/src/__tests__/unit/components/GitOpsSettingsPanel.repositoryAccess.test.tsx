import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { GitOpsSettingsPanel } from '../../../components/project-detail/settings/GitOpsSettingsPanel';
import { DEFAULT_PROJECT_SETTINGS } from '../../../api/projectSettings';

describe('GitOpsSettingsPanel repository access guard', () => {
  it('locks GitOps toggles when repository access is read-only', () => {
    const onUpdateSetting = vi.fn().mockResolvedValue(undefined);

    render(
      <GitOpsSettingsPanel
        settings={{
          ...DEFAULT_PROJECT_SETTINGS,
          gitops_enabled: true,
          auto_merge: false,
        }}
        saving={false}
        onUpdateSetting={onUpdateSetting}
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
        }}
      />
    );

    const gitOpsSwitch = screen.getByRole('switch', { name: 'GitOps Workflow' }) as HTMLButtonElement;
    const autoMergeSwitch = screen.getByRole('switch', { name: 'Auto-Merge Approved MRs' }) as HTMLButtonElement;

    expect(gitOpsSwitch.disabled).toBe(true);
    expect(autoMergeSwitch.disabled).toBe(true);
    expect(screen.getByText('GitOps settings locked by repository access')).toBeTruthy();
    expect(
      screen.getByText(
        'Link a writable fork or import a repository you can push to before starting coding attempts.'
      )
    ).toBeTruthy();

    fireEvent.click(gitOpsSwitch);
    fireEvent.click(autoMergeSwitch);

    expect(onUpdateSetting).not.toHaveBeenCalled();
  });

  it('allows GitOps workflow but disables auto-merge when merge capability is unavailable', () => {
    const onUpdateSetting = vi.fn().mockResolvedValue(undefined);

    render(
      <GitOpsSettingsPanel
        settings={{
          ...DEFAULT_PROJECT_SETTINGS,
          gitops_enabled: true,
          auto_merge: false,
        }}
        saving={false}
        onUpdateSetting={onUpdateSetting}
        repositoryContext={{
          provider: 'gitlab',
          access_mode: 'direct_gitops',
          verification_status: 'verified',
          can_clone: true,
          can_push: true,
          can_open_change_request: true,
          can_merge: false,
          can_manage_webhooks: false,
          can_fork: true,
          writable_repository_url: 'https://gitlab.com/me/app-fork',
          effective_clone_url: 'https://gitlab.com/me/app-fork',
        }}
      />
    );

    const gitOpsSwitch = screen.getByRole('switch', { name: 'GitOps Workflow' }) as HTMLButtonElement;
    const autoMergeSwitch = screen.getByRole('switch', { name: 'Auto-Merge Approved MRs' }) as HTMLButtonElement;

    expect(gitOpsSwitch.disabled).toBe(false);
    expect(autoMergeSwitch.disabled).toBe(true);
    expect(screen.getByText('Auto-merge unavailable')).toBeTruthy();
    expect(
      screen.getByText('Current repository access does not allow automatic merge operations.')
    ).toBeTruthy();

    fireEvent.click(gitOpsSwitch);
    fireEvent.click(autoMergeSwitch);

    expect(onUpdateSetting).toHaveBeenCalledTimes(1);
    expect(onUpdateSetting).toHaveBeenCalledWith('gitops_enabled', false);
  });
});
