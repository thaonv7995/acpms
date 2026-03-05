// Generated TypeScript types from Rust models
// This file is auto-generated - do not edit manually

export type ProjectRole = 'owner' | 'admin' | 'developer' | 'viewer';
export type ProjectType = 'web' | 'mobile' | 'desktop' | 'extension' | 'api' | 'microservice';

export type TaskType =
  | 'feature'
  | 'bug'
  | 'refactor'
  | 'docs'
  | 'test'
  | 'init'
  | 'hotfix'
  | 'chore'
  | 'spike'
  | 'small_task'
  | 'deploy';

export type TaskStatus = 'backlog' | 'todo' | 'in_progress' | 'in_review' | 'blocked' | 'done' | 'archived';

export type AttemptStatus = 'queued' | 'running' | 'success' | 'failed' | 'cancelled';

/**
 * @deprecated Use UserDto from @/api/generated/models
 *
 * Note: UserDto has slightly different structure:
 * - avatar_url (string | null) vs avatar_url? (string | undefined)
 * - created_at/updated_at are guaranteed present
 */
export interface User {
  id: string;
  email: string;
  name: string;
  avatar_url?: string;
  gitlab_id?: number;
  gitlab_username?: string;
  created_at: string;
  updated_at: string;
}

/**
 * @deprecated Will be removed once manual API layer is migrated
 * Frontend-specific metadata structure (backend stores as JSON string)
 */
export interface ProjectMetadata {
  icon?: string;
  iconColor?: 'orange' | 'blue' | 'emerald' | 'purple' | 'primary';
  techStack?: string[];
  status?: 'agent_reviewing' | 'active_coding' | 'deploying' | 'completed' | 'paused';
  statusLabel?: string;
  statusColor?: 'yellow' | 'blue' | 'emerald' | 'green' | 'slate';
  progress?: number;
  agentCount?: number;
}

export interface ProjectSettings {
  require_review: boolean;
  auto_deploy: boolean;
  preview_enabled: boolean;
  auto_execute: boolean;
}

/**
 * @deprecated Use ProjectDto from @/api/generated/models
 *
 * Note: ProjectDto.metadata is string (JSON), not ProjectMetadata object
 */
export interface Project {
  id: string;
  name: string;
  description?: string;
  repository_url?: string;
  metadata?: ProjectMetadata;
  require_review?: boolean;
  settings?: ProjectSettings;
  project_type?: ProjectType;
  created_by: string;
  created_at: string;
  updated_at: string;
}

/**
 * @deprecated Use CreateProjectRequestDoc from @/api/generated/models
 *
 * Note: CreateProjectRequestDoc.metadata is string (JSON), not ProjectMetadata
 */
export interface CreateProjectRequest {
  name: string;
  description?: string;
  repository_url?: string;
  metadata?: ProjectMetadata;
  require_review?: boolean;
  create_from_scratch?: boolean;
  visibility?: 'private' | 'public' | 'internal';
  tech_stack?: string;
  stack_selections?: Array<{
    layer: 'frontend' | 'backend' | 'database' | 'auth' | 'cache' | 'queue';
    stack: string;
  }>;
  auto_create_init_task?: boolean;
  project_type?: ProjectType;
  template_id?: string;
  preview_enabled?: boolean;
  reference_keys?: string[];
}

/**
 * @deprecated Use UpdateProjectRequestDoc from @/api/generated/models
 */
export interface UpdateProjectRequest {
  name?: string;
  description?: string;
  repository_url?: string;
  metadata?: ProjectMetadata;
}

/**
 * @deprecated Use TaskDto from @/api/generated/models
 */
export interface Task {
  id: string;
  project_id: string;
  title: string;
  description?: string;
  task_type: TaskType;
  status: TaskStatus;
  assigned_to?: string;
  parent_task_id?: string;
  requirement_id?: string;
  sprint_id?: string;
  gitlab_issue_id?: number;
  metadata: Record<string, any>;
  created_by: string;
  created_at: string;
  updated_at: string;
}

/**
 * @deprecated Use CreateTaskRequestDoc from @/api/generated/models
 */
export interface CreateTaskRequest {
  project_id: string;
  requirement_id?: string;
  sprint_id?: string;
  title: string;
  description?: string;
  task_type: TaskType;
  assigned_to?: string;
  metadata?: Record<string, unknown>;
  parent_task_id?: string;
}

/**
 * @deprecated Use UpdateTaskRequestDoc from @/api/generated/models
 */
export interface UpdateTaskRequest {
  title?: string;
  description?: string;
  status?: TaskStatus;
  assigned_to?: string;
}

/**
 * @deprecated Use TaskAttemptDto from @/api/generated/models
 */
export interface TaskAttempt {
  id: string;
  task_id: string;
  status: AttemptStatus;
  started_at?: string;
  completed_at?: string;
  error_message?: string;
  metadata: Record<string, any>;
  created_at: string;
}

/**
 * @deprecated Use AgentLogDto from @/api/generated/models (if available)
 */
export interface AgentLog {
  id: string;
  attempt_id: string;
  log_type: string;
  content: string;
  created_at: string;
}

/**
 * @deprecated Use GitLabConfigurationDto from @/api/generated/models (if available)
 */
export interface GitLabConfiguration {
  id: string;
  project_id: string;
  gitlab_project_id: number;
  base_url: string;
  created_at: string;
  updated_at: string;
}

/**
 * Request to link a project to GitLab.
 * Provide either repository_url (paste URL) or gitlab_project_id.
 * GitLab URL and PAT are configured globally in System Settings.
 */
export interface LinkGitLabProjectRequest {
  /** GitLab project ID (numeric). Optional if repository_url is provided. */
  gitlab_project_id?: number;
  /** Repository URL (e.g. https://gitlab.com/group/repo). Resolved to project_id via API. */
  repository_url?: string;
}

/**
 * @deprecated Use MergeRequestDto from @/api/generated/models
 */
export interface MergeRequestDb {
  id: string;
  task_id: string;
  gitlab_mr_iid: number;
  web_url: string;
  status: string;
  created_at: string;
  updated_at: string;
}

/**
 * @deprecated Use SprintDto from @/api/generated/models
 */
export interface Sprint {
  id: string;
  project_id: string;
  name: string;
  description: string | null;
  status: 'planned' | 'active' | 'closed' | 'archived';
  start_date: string | null;
  end_date: string | null;
  created_at: string;
  updated_at: string;
}

/**
 * @deprecated Use CreateSprintRequestDoc from @/api/generated/models
 */
export interface CreateSprintRequest {
  project_id: string;
  name: string;
  description?: string;
  start_date?: string;
  end_date?: string;
}

/**
 * @deprecated Use GenerateSprintsRequestDoc from @/api/generated/models
 */
export interface GenerateSprintsRequest {
  project_id: string;
  start_date: string;
  duration_weeks: number;
  count: number;
}
