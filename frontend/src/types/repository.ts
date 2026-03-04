export type RepositoryProvider = 'github' | 'gitlab' | 'unknown';

export type RepositoryAccessMode =
  | 'analysis_only'
  | 'direct_gitops'
  | 'branch_push_only'
  | 'fork_gitops'
  | 'unknown';

export type RepositoryVerificationStatus =
  | 'verified'
  | 'unauthenticated'
  | 'failed'
  | 'unknown';

export interface RepositoryContext {
  provider: RepositoryProvider;
  access_mode: RepositoryAccessMode;
  verification_status: RepositoryVerificationStatus;
  verification_error?: string | null;
  upstream_repository_url?: string | null;
  writable_repository_url?: string | null;
  effective_clone_url?: string | null;
  upstream_project_id?: string | number | null;
  writable_project_id?: string | number | null;
  default_branch?: string | null;
  can_clone?: boolean;
  can_push?: boolean;
  can_open_change_request?: boolean;
  can_merge?: boolean;
  can_manage_webhooks?: boolean;
  can_fork?: boolean;
  verified_at?: string | null;
}

export interface ImportProjectPreflightResponse {
  recommended_action?: string | null;
  warnings: string[];
  repository_context: RepositoryContext;
}

export interface ImportProjectCreateForkResponse {
  upstream_repository_url: string;
  fork_repository_url: string;
  repository_context: RepositoryContext;
  recommended_action?: string | null;
  warnings: string[];
}

export interface RecheckRepositoryAccessResponse {
  project: ProjectWithRepositoryContext;
  recommended_action?: string | null;
  warnings: string[];
}

export interface LinkExistingForkResponse {
  project: ProjectWithRepositoryContext;
  recommended_action?: string | null;
  warnings: string[];
}

export interface CreateForkResponse {
  project: ProjectWithRepositoryContext;
  created_repository_url: string;
  recommended_action?: string | null;
  warnings: string[];
}

export interface ProjectWithRepositoryContext {
  id: string;
  name: string;
  description?: string;
  repository_url?: string;
  metadata?: Record<string, unknown>;
  require_review?: boolean;
  settings?: {
    require_review: boolean;
    auto_deploy: boolean;
    preview_enabled: boolean;
    auto_execute: boolean;
    [key: string]: unknown;
  };
  project_type?: string;
  created_by: string;
  created_at: string;
  updated_at: string;
  repository_context?: RepositoryContext;
}
