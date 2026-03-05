import { apiGet, apiPost, apiPut, apiDelete, API_PREFIX, apiGetFull, type ApiResponse } from './client';
import type { CreateProjectRequest, UpdateProjectRequest } from '../shared/types';
import type { ProjectType } from './templates';
import type {
  CreateForkResponse,
  ImportProjectCreateForkResponse,
  ImportProjectPreflightResponse,
  LinkExistingForkResponse,
  ProjectWithRepositoryContext,
  RecheckRepositoryAccessResponse,
} from '../types/repository';

export interface ProjectMember {
  id: string;
  name: string;
  email: string;
  avatar_url?: string | null;
  roles: string[];
}

export interface ProjectsQueryParams {
  page?: number;
  limit?: number;
  before?: string;       // ISO 8601 / RFC 3339 timestamp
  before_id?: string;    // UUID tie-breaker
  search?: string;
}

export async function getProjects(
  params?: ProjectsQueryParams
): Promise<ApiResponse<ProjectWithRepositoryContext[]>> {
  const query = params
    ? '?' + new URLSearchParams(
      Object.entries(params)
        .filter(([, v]) => v != null)
        .map(([k, v]) => [k, String(v)])
    ).toString()
    : '';
  return apiGetFull<ProjectWithRepositoryContext[]>(`${API_PREFIX}/projects${query}`);
}

export async function getProject(id: string): Promise<ProjectWithRepositoryContext> {
  return apiGet<ProjectWithRepositoryContext>(`${API_PREFIX}/projects/${id}`);
}

export async function getProjectMembers(projectId: string): Promise<ProjectMember[]> {
  return apiGet<ProjectMember[]>(`${API_PREFIX}/projects/${projectId}/members`);
}

export interface InviteableUser {
  id: string;
  name: string;
  email: string;
  avatar_url?: string | null;
}

export async function getInviteableUsers(projectId: string): Promise<InviteableUser[]> {
  return apiGet<InviteableUser[]>(`${API_PREFIX}/projects/${projectId}/inviteable-users`);
}

export async function addProjectMember(
  projectId: string,
  data: { user_id: string; roles: string[] }
): Promise<ProjectMember> {
  return apiPost<ProjectMember>(`${API_PREFIX}/projects/${projectId}/members`, data);
}

export async function updateProjectMember(
  projectId: string,
  userId: string,
  data: { roles: string[] }
): Promise<ProjectMember> {
  return apiPut<ProjectMember>(`${API_PREFIX}/projects/${projectId}/members/${userId}`, data);
}

export async function removeProjectMember(
  projectId: string,
  userId: string
): Promise<void> {
  return apiDelete(`${API_PREFIX}/projects/${projectId}/members/${userId}`);
}

export async function syncProjectRepository(projectId: string): Promise<{
  last_sync_at: string;
  branches_synced: number;
  merge_requests_synced: number;
  pipelines_synced: number;
}> {
  return apiPost(`${API_PREFIX}/projects/${projectId}/sync`, {});
}

export async function createProject(data: CreateProjectRequest): Promise<ProjectWithRepositoryContext> {
  return apiPost<ProjectWithRepositoryContext>(`${API_PREFIX}/projects`, data);
}

export async function getInitRefUploadUrl(data: {
  filename: string;
  content_type: string;
}): Promise<{ upload_url: string; key: string }> {
  const res = await apiPost<{ upload_url: string; key: string }>(
    `${API_PREFIX}/projects/init-refs/upload-url`,
    data
  );
  return res;
}

export interface ImportProjectResponse {
  project: ProjectWithRepositoryContext;
  init_task_id?: string | null;
}

export async function importProjectPreflight(data: {
  repository_url: string;
  upstream_repository_url?: string;
}): Promise<ImportProjectPreflightResponse> {
  return apiPost<ImportProjectPreflightResponse>(`${API_PREFIX}/projects/import/preflight`, data);
}

export async function importProjectCreateFork(data: {
  repository_url: string;
}): Promise<ImportProjectCreateForkResponse> {
  return apiPost<ImportProjectCreateForkResponse>(
    `${API_PREFIX}/projects/import/create-fork`,
    data
  );
}

export async function recheckProjectRepositoryAccess(
  projectId: string
): Promise<RecheckRepositoryAccessResponse> {
  return apiPost<RecheckRepositoryAccessResponse>(
    `${API_PREFIX}/projects/${projectId}/repository-context/recheck`,
    {}
  );
}

export async function linkExistingFork(
  projectId: string,
  data: { repository_url: string }
): Promise<LinkExistingForkResponse> {
  return apiPost<LinkExistingForkResponse>(
    `${API_PREFIX}/projects/${projectId}/repository-context/link-fork`,
    data
  );
}

export async function createProjectFork(
  projectId: string
): Promise<CreateForkResponse> {
  return apiPost<CreateForkResponse>(
    `${API_PREFIX}/projects/${projectId}/repository-context/create-fork`,
    {}
  );
}

export async function importProjectFromGitLab(data: {
  name: string;
  repository_url: string;
  upstream_repository_url?: string;
  description?: string;
  require_review?: boolean;
  project_type?: ProjectType;
  auto_create_init_task?: boolean;
  preview_enabled?: boolean;
}): Promise<ImportProjectResponse> {
  return apiPost<ImportProjectResponse>(`${API_PREFIX}/projects/import`, data);
}

export async function updateProject(
  id: string,
  data: UpdateProjectRequest
): Promise<ProjectWithRepositoryContext> {
  return apiPut<ProjectWithRepositoryContext>(`${API_PREFIX}/projects/${id}`, data);
}

export interface DeleteProjectOptions {
  deleteLocalFolder?: boolean;
  deleteGitRepo?: boolean;
}

export async function deleteProject(id: string, options?: DeleteProjectOptions): Promise<void> {
  const query = new URLSearchParams();

  if (options?.deleteLocalFolder) {
    query.set('delete_local_folder', 'true');
  }

  if (options?.deleteGitRepo) {
    query.set('delete_git_repo', 'true');
  }

  const suffix = query.toString();
  const endpoint = suffix
    ? `${API_PREFIX}/projects/${id}?${suffix}`
    : `${API_PREFIX}/projects/${id}`;

  return apiDelete(endpoint);
}
