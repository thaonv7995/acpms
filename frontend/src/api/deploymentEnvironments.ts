import {
  API_PREFIX,
  apiDelete,
  apiGet,
  apiPatch,
  apiPost,
} from './client';

export type DeploymentTargetType = 'local' | 'ssh_remote';
export type DeploymentRuntimeType = 'compose' | 'systemd' | 'raw_script';
export type DeploymentArtifactStrategy = 'git_pull' | 'upload_bundle' | 'build_artifact';
export type DeploymentRunStatus =
  | 'queued'
  | 'running'
  | 'success'
  | 'failed'
  | 'cancelled'
  | 'rolling_back'
  | 'rolled_back';
export type DeploymentTriggerType = 'manual' | 'auto' | 'rollback' | 'retry';
export type DeploymentSourceType = 'branch' | 'commit' | 'artifact' | 'release';
export type DeploymentReleaseStatus = 'active' | 'superseded' | 'failed' | 'rolled_back';
export type DeploymentTimelineStep =
  | 'precheck'
  | 'connect'
  | 'prepare'
  | 'deploy'
  | 'domain_config'
  | 'healthcheck'
  | 'finalize'
  | 'rollback';
export type DeploymentTimelineEventType = 'system' | 'agent' | 'command' | 'warning' | 'error';

export interface DeploymentEnvironment {
  id: string;
  project_id: string;
  name: string;
  slug: string;
  description?: string | null;
  target_type: DeploymentTargetType;
  is_enabled: boolean;
  is_default: boolean;
  runtime_type: DeploymentRuntimeType;
  deploy_path: string;
  artifact_strategy: DeploymentArtifactStrategy;
  branch_policy: Record<string, unknown>;
  healthcheck_url?: string | null;
  healthcheck_timeout_secs: number;
  healthcheck_expected_status: number;
  target_config: Record<string, unknown>;
  domain_config: Record<string, unknown>;
  created_by?: string | null;
  created_at: string;
  updated_at: string;
}

export interface DeploymentRun {
  id: string;
  project_id: string;
  environment_id: string;
  status: DeploymentRunStatus;
  trigger_type: DeploymentTriggerType;
  triggered_by?: string | null;
  source_type: DeploymentSourceType;
  source_ref?: string | null;
  attempt_id?: string | null;
  started_at?: string | null;
  completed_at?: string | null;
  error_message?: string | null;
  metadata: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

export interface DeploymentRelease {
  id: string;
  project_id: string;
  environment_id: string;
  run_id: string;
  version_label: string;
  artifact_ref?: string | null;
  git_commit_sha?: string | null;
  status: DeploymentReleaseStatus;
  deployed_at: string;
  metadata: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

export interface DeploymentTimelineEvent {
  id: string;
  run_id: string;
  step: DeploymentTimelineStep;
  event_type: DeploymentTimelineEventType;
  message: string;
  payload: Record<string, unknown>;
  created_at: string;
}

export interface DeploymentEnvironmentSecretInput {
  secret_type: 'ssh_private_key' | 'ssh_password' | 'api_token' | 'known_hosts' | 'env_file';
  value: string;
}

export interface CreateDeploymentEnvironmentRequest {
  name: string;
  description?: string;
  target_type: DeploymentTargetType;
  is_enabled?: boolean;
  is_default?: boolean;
  runtime_type?: DeploymentRuntimeType;
  deploy_path: string;
  artifact_strategy?: DeploymentArtifactStrategy;
  branch_policy?: Record<string, unknown>;
  healthcheck_url?: string;
  healthcheck_timeout_secs?: number;
  healthcheck_expected_status?: number;
  target_config?: Record<string, unknown>;
  domain_config?: Record<string, unknown>;
  secrets?: DeploymentEnvironmentSecretInput[];
}

export interface UpdateDeploymentEnvironmentRequest {
  name?: string;
  description?: string;
  target_type?: DeploymentTargetType;
  is_enabled?: boolean;
  is_default?: boolean;
  runtime_type?: DeploymentRuntimeType;
  deploy_path?: string;
  artifact_strategy?: DeploymentArtifactStrategy;
  branch_policy?: Record<string, unknown>;
  healthcheck_url?: string;
  healthcheck_timeout_secs?: number;
  healthcheck_expected_status?: number;
  target_config?: Record<string, unknown>;
  domain_config?: Record<string, unknown>;
  secrets?: DeploymentEnvironmentSecretInput[];
}

export interface DeploymentCheckResult {
  step: string;
  status: 'pass' | 'fail';
  message: string;
}

export interface DeploymentConnectionTestResponse {
  success: boolean;
  checks: DeploymentCheckResult[];
}

export interface StartDeploymentRunRequest {
  source_type?: DeploymentSourceType;
  source_ref?: string;
  attempt_id?: string;
  metadata?: Record<string, unknown>;
}

export interface RollbackDeploymentRunRequest {
  target_release_id?: string;
  metadata?: Record<string, unknown>;
}

export interface ListDeploymentRunsQuery {
  environment_id?: string;
  status?: DeploymentRunStatus;
  limit?: number;
}

export interface ListDeploymentReleasesQuery {
  status?: DeploymentReleaseStatus;
  limit?: number;
}

function buildQueryString(params: Record<string, string | number | undefined>): string {
  const query = new URLSearchParams();

  Object.entries(params).forEach(([key, value]) => {
    if (value !== undefined && value !== null && value !== '') {
      query.set(key, String(value));
    }
  });

  const queryString = query.toString();
  return queryString ? `?${queryString}` : '';
}

export async function fetchSshKnownHosts(
  projectId: string,
  host: string
): Promise<{ known_hosts: string }> {
  return apiPost<{ known_hosts: string }>(
    `${API_PREFIX}/projects/${projectId}/deployment-environments/ssh-keyscan`,
    { host }
  );
}

export async function listDeploymentEnvironments(projectId: string): Promise<DeploymentEnvironment[]> {
  return apiGet<DeploymentEnvironment[]>(`${API_PREFIX}/projects/${projectId}/deployment-environments`);
}

export async function createDeploymentEnvironment(
  projectId: string,
  data: CreateDeploymentEnvironmentRequest
): Promise<DeploymentEnvironment> {
  return apiPost<DeploymentEnvironment>(
    `${API_PREFIX}/projects/${projectId}/deployment-environments`,
    data
  );
}

export async function getDeploymentEnvironment(
  projectId: string,
  environmentId: string
): Promise<DeploymentEnvironment> {
  return apiGet<DeploymentEnvironment>(
    `${API_PREFIX}/projects/${projectId}/deployment-environments/${environmentId}`
  );
}

export async function updateDeploymentEnvironment(
  projectId: string,
  environmentId: string,
  data: UpdateDeploymentEnvironmentRequest
): Promise<DeploymentEnvironment> {
  return apiPatch<DeploymentEnvironment>(
    `${API_PREFIX}/projects/${projectId}/deployment-environments/${environmentId}`,
    data
  );
}

export async function deleteDeploymentEnvironment(
  projectId: string,
  environmentId: string
): Promise<void> {
  return apiDelete(`${API_PREFIX}/projects/${projectId}/deployment-environments/${environmentId}`);
}

export async function testDeploymentEnvironmentConnection(
  projectId: string,
  environmentId: string
): Promise<DeploymentConnectionTestResponse> {
  return apiPost<DeploymentConnectionTestResponse>(
    `${API_PREFIX}/projects/${projectId}/deployment-environments/${environmentId}/test-connection`,
    {}
  );
}

export async function testDeploymentEnvironmentDomain(
  projectId: string,
  environmentId: string
): Promise<DeploymentConnectionTestResponse> {
  return apiPost<DeploymentConnectionTestResponse>(
    `${API_PREFIX}/projects/${projectId}/deployment-environments/${environmentId}/test-domain`,
    {}
  );
}

export async function startDeploymentRun(
  projectId: string,
  environmentId: string,
  data?: StartDeploymentRunRequest
): Promise<DeploymentRun> {
  return apiPost<DeploymentRun>(
    `${API_PREFIX}/projects/${projectId}/deployment-environments/${environmentId}/deploy`,
    data ?? {}
  );
}

export async function listDeploymentRuns(
  projectId: string,
  query?: ListDeploymentRunsQuery
): Promise<DeploymentRun[]> {
  const queryString = buildQueryString({
    environment_id: query?.environment_id,
    status: query?.status,
    limit: query?.limit,
  });

  return apiGet<DeploymentRun[]>(`${API_PREFIX}/projects/${projectId}/deployment-runs${queryString}`);
}

export async function getDeploymentRun(runId: string): Promise<DeploymentRun> {
  return apiGet<DeploymentRun>(`${API_PREFIX}/deployment-runs/${runId}`);
}

export async function listDeploymentRunLogs(runId: string): Promise<DeploymentTimelineEvent[]> {
  return apiGet<DeploymentTimelineEvent[]>(`${API_PREFIX}/deployment-runs/${runId}/logs`);
}

export async function listDeploymentRunTimeline(runId: string): Promise<DeploymentTimelineEvent[]> {
  return apiGet<DeploymentTimelineEvent[]>(`${API_PREFIX}/deployment-runs/${runId}/timeline`);
}

export async function cancelDeploymentRun(runId: string): Promise<DeploymentRun> {
  return apiPost<DeploymentRun>(`${API_PREFIX}/deployment-runs/${runId}/cancel`, {});
}

export async function retryDeploymentRun(runId: string): Promise<DeploymentRun> {
  return apiPost<DeploymentRun>(`${API_PREFIX}/deployment-runs/${runId}/retry`, {});
}

export async function rollbackDeploymentRun(
  runId: string,
  data?: RollbackDeploymentRunRequest
): Promise<DeploymentRun> {
  return apiPost<DeploymentRun>(`${API_PREFIX}/deployment-runs/${runId}/rollback`, data ?? {});
}

export async function listDeploymentReleases(
  projectId: string,
  environmentId: string,
  query?: ListDeploymentReleasesQuery
): Promise<DeploymentRelease[]> {
  const queryString = buildQueryString({
    status: query?.status,
    limit: query?.limit,
  });

  return apiGet<DeploymentRelease[]>(
    `${API_PREFIX}/projects/${projectId}/deployment-environments/${environmentId}/releases${queryString}`
  );
}

export async function getDeploymentRelease(releaseId: string): Promise<DeploymentRelease> {
  return apiGet<DeploymentRelease>(`${API_PREFIX}/deployment-releases/${releaseId}`);
}

export function getDeploymentRunStreamUrl(runId: string, afterId?: string): string {
  const queryString = buildQueryString({ after_id: afterId });
  return `${API_PREFIX}/deployment-runs/${runId}/stream${queryString}`;
}
