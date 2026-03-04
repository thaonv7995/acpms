import type {
  RepositoryAccessMode,
  RepositoryContext,
  RepositoryProvider,
  RepositoryVerificationStatus,
} from '../types/repository';

const DEFAULT_CONTEXT: RepositoryContext = {
  provider: 'unknown',
  access_mode: 'unknown',
  verification_status: 'unknown',
  can_clone: false,
  can_push: false,
  can_open_change_request: false,
  can_merge: false,
  can_manage_webhooks: false,
  can_fork: false,
};

export function normalizeRepositoryContext(
  context?: RepositoryContext | null
): RepositoryContext {
  return {
    ...DEFAULT_CONTEXT,
    ...context,
  };
}

export function isRepositoryReadOnly(context?: RepositoryContext | null): boolean {
  const normalized = normalizeRepositoryContext(context);
  return normalized.access_mode === 'analysis_only' || normalized.access_mode === 'unknown';
}

export function supportsRepositoryGitOps(context?: RepositoryContext | null): boolean {
  return !isRepositoryReadOnly(context);
}

export function getRepositoryProviderLabel(provider?: RepositoryProvider): string {
  switch (provider) {
    case 'github':
      return 'GitHub';
    case 'gitlab':
      return 'GitLab';
    default:
      return 'Repository';
  }
}

export function getRepositoryModeLabel(mode?: RepositoryAccessMode): string {
  switch (mode) {
    case 'direct_gitops':
      return 'Direct GitOps';
    case 'fork_gitops':
      return 'Fork-based GitOps';
    case 'branch_push_only':
      return 'Branch Push Only';
    case 'analysis_only':
      return 'Analysis Only';
    default:
      return 'Access Unknown';
  }
}

export function getRepositoryVerificationLabel(
  status?: RepositoryVerificationStatus
): string {
  switch (status) {
    case 'verified':
      return 'Verified';
    case 'unauthenticated':
      return 'Not authenticated';
    case 'failed':
      return 'Verification failed';
    default:
      return 'Not checked';
  }
}

export function getRepositoryAccessTone(
  context?: RepositoryContext | null
): 'success' | 'warning' | 'neutral' {
  const normalized = normalizeRepositoryContext(context);

  if (
    normalized.access_mode === 'direct_gitops' ||
    normalized.access_mode === 'fork_gitops'
  ) {
    return 'success';
  }

  if (
    normalized.access_mode === 'analysis_only' ||
    normalized.access_mode === 'branch_push_only'
  ) {
    return 'warning';
  }

  return 'neutral';
}

export function getRepositoryAccessSummary(context?: RepositoryContext | null): {
  title: string;
  description: string;
  action: string;
} {
  const normalized = normalizeRepositoryContext(context);
  const provider = getRepositoryProviderLabel(normalized.provider);

  switch (normalized.access_mode) {
    case 'direct_gitops':
      return {
        title: `${provider} write access verified`,
        description:
          'Agent can push changes to this repository and create merge requests or pull requests.',
        action: 'Full GitOps is available for this project.',
      };
    case 'fork_gitops':
      return {
        title: `${provider} fork workflow ready`,
        description:
          'Agent can push to a writable fork and open a change request back to the upstream repository.',
        action: 'Use this mode when the upstream repository is not directly writable.',
      };
    case 'branch_push_only':
      return {
        title: `${provider} branch push only`,
        description:
          'Agent can push branches, but automated merge request or pull request creation is not available yet.',
        action: 'Expect manual PR or MR creation after the agent pushes a branch.',
      };
    case 'analysis_only':
      return {
        title: `${provider} access is read-only`,
        description:
          'This repository can be cloned and analyzed, but agent execution that changes code is blocked.',
        action:
          'Link a writable fork or import a repository you can push to before starting coding attempts.',
      };
    default:
      return {
        title: `${provider} access could not be verified`,
        description:
          'The repository may be cloneable, but ACPMS could not confirm write access for GitOps actions.',
        action:
          'Re-check repository access or configure a PAT with the scopes required for push and PR or MR creation.',
      };
  }
}

export function getRepositoryHref(url?: string | null): string | undefined {
  if (!url) return undefined;
  if (url.startsWith('http://') || url.startsWith('https://')) {
    return url;
  }
  return `https://${url}`;
}
