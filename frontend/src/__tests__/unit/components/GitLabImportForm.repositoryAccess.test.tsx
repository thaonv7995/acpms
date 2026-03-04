import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { GitLabImportForm } from '../../../components/modals/create-project/GitLabImportForm';
import type { ImportProjectPreflightResponse } from '../../../types/repository';

function renderForm(preflight: ImportProjectPreflightResponse, upstreamRepoUrl = '') {
  return render(
    <GitLabImportForm
      projectName="ACPMS"
      repoUrl={
        upstreamRepoUrl
          ? 'https://github.com/acme/app-fork'
          : 'https://github.com/acme/app'
      }
      upstreamRepoUrl={upstreamRepoUrl}
      preflight={preflight}
      preflightLoading={false}
      preflightError={null}
      forkPending={false}
      onProjectNameChange={vi.fn()}
      onRepoUrlChange={vi.fn()}
      onCreateFork={vi.fn()}
    />
  );
}

describe('GitLabImportForm repository access states', () => {
  it('renders read-only preflight with auto-fork CTA and warnings', () => {
    renderForm({
      recommended_action:
        'Import for analysis only, then link or create a writable fork before starting coding tasks.',
      warnings: [
        'Repository is currently read-only for agent workflows.',
        'Current credentials cannot push branches to this repository.',
      ],
      repository_context: {
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
      },
    });

    expect(screen.getByText('GitHub access is read-only')).toBeTruthy();
    expect(screen.getByText('Analysis Only')).toBeTruthy();
    expect(screen.getByText('Verified')).toBeTruthy();
    expect(screen.getByText('Yes Clone')).toBeTruthy();
    expect(screen.getByText('No Push')).toBeTruthy();
    expect(screen.getByText('No PR/MR')).toBeTruthy();
    expect(screen.getByRole('button', { name: /Create fork automatically/i })).toBeTruthy();
    expect(screen.getByText('Repository is currently read-only for agent workflows.')).toBeTruthy();
    expect(screen.getByText('Current credentials cannot push branches to this repository.')).toBeTruthy();
  });

  it('renders fork-based preflight without auto-fork CTA when upstream is already linked', () => {
    renderForm(
      {
        recommended_action:
          'Repository should use fork-based GitOps. Push to the writable fork and open PR/MR back to upstream.',
        warnings: [],
        repository_context: {
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
          writable_repository_url: 'https://github.com/acme/app-fork',
          effective_clone_url: 'https://github.com/acme/app-fork',
          default_branch: 'main',
        },
      },
      'https://github.com/acme/app'
    );

    expect(screen.getByText('GitHub fork workflow ready')).toBeTruthy();
    expect(screen.getByText('Fork-based GitOps')).toBeTruthy();
    expect(screen.getByText('Upstream repository')).toBeTruthy();
    expect(screen.getByText('https://github.com/acme/app')).toBeTruthy();
    expect(screen.queryByRole('button', { name: /Create fork automatically/i })).toBeNull();
    expect(screen.getByText('Yes Push')).toBeTruthy();
    expect(screen.getByText('Yes PR/MR')).toBeTruthy();
  });
});
