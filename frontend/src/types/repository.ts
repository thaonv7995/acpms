import type {
  CreateForkResponse as GeneratedCreateForkResponse,
  ImportProjectCreateForkResponse as GeneratedImportProjectCreateForkResponse,
  ImportProjectPreflightResponse as GeneratedImportProjectPreflightResponse,
  LinkExistingForkResponse as GeneratedLinkExistingForkResponse,
  ProjectDto,
  ProjectSummaryDto,
  RecheckRepositoryAccessResponse as GeneratedRecheckRepositoryAccessResponse,
  RepositoryAccessMode as GeneratedRepositoryAccessMode,
  RepositoryContext as GeneratedRepositoryContext,
  RepositoryProvider as GeneratedRepositoryProvider,
  RepositoryVerificationStatus as GeneratedRepositoryVerificationStatus,
} from '../api/generated/models';

export type RepositoryProvider = GeneratedRepositoryProvider;

export type RepositoryAccessMode = GeneratedRepositoryAccessMode;

export type RepositoryVerificationStatus = GeneratedRepositoryVerificationStatus;

export type ProjectLifecycleStatus =
  | 'planning'
  | 'active'
  | 'reviewing'
  | 'blocked'
  | 'completed'
  | 'paused'
  | 'archived';

export type ProjectExecutionStatus =
  | 'idle'
  | 'queued'
  | 'running'
  | 'success'
  | 'failed'
  | 'cancelled';

export interface ProjectSummary
  extends Omit<ProjectSummaryDto, 'lifecycle_status' | 'execution_status'> {
  lifecycle_status: ProjectLifecycleStatus;
  execution_status: ProjectExecutionStatus;
}

export interface RepositoryContext extends GeneratedRepositoryContext {
  provider?: RepositoryProvider;
  access_mode?: RepositoryAccessMode;
  verification_status?: RepositoryVerificationStatus;
}

export interface ProjectWithRepositoryContext
  extends Omit<
    ProjectDto,
    'architecture_config' | 'metadata' | 'project_type' | 'repository_context' | 'settings' | 'summary'
  > {
  architecture_config?: Record<string, unknown>;
  metadata?: Record<string, unknown>;
  project_type?: string;
  settings?: ProjectDto['settings'];
  summary?: ProjectSummary | null;
  repository_context?: RepositoryContext;
}

export type ImportProjectPreflightResponse = Omit<
  GeneratedImportProjectPreflightResponse,
  'repository_context'
> & {
  repository_context: RepositoryContext;
};

export type ImportProjectCreateForkResponse = Omit<
  GeneratedImportProjectCreateForkResponse,
  'repository_context'
> & {
  repository_context: RepositoryContext;
};

export type RecheckRepositoryAccessResponse = Omit<
  GeneratedRecheckRepositoryAccessResponse,
  'project'
> & {
  project: ProjectWithRepositoryContext;
};

export type LinkExistingForkResponse = Omit<
  GeneratedLinkExistingForkResponse,
  'project'
> & {
  project: ProjectWithRepositoryContext;
};

export type CreateForkResponse = Omit<GeneratedCreateForkResponse, 'project'> & {
  project: ProjectWithRepositoryContext;
};
