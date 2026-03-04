import { apiGet, apiPost, apiPut, apiDelete, API_PREFIX } from './client';
import type { Task, CreateTaskRequest, UpdateTaskRequest, TaskStatus } from '../shared/types';

export type { Task, CreateTaskRequest, UpdateTaskRequest, TaskStatus };

export interface TaskAttachmentUploadUrlRequest {
  project_id: string;
  filename: string;
  content_type: string;
}

export interface TaskAttachmentUploadUrlResponse {
  upload_url: string;
  key: string;
}

export async function getTasks(projectId?: string): Promise<Task[]> {
  const params = projectId ? `?project_id=${projectId}` : '';
  return apiGet<Task[]>(`${API_PREFIX}/tasks${params}`);
}

export async function getTask(id: string): Promise<Task> {
  return apiGet<Task>(`${API_PREFIX}/tasks/${id}`);
}

export async function createTask(data: CreateTaskRequest): Promise<Task> {
  return apiPost<Task>(`${API_PREFIX}/tasks`, data);
}

export async function getTaskAttachmentUploadUrl(
  data: TaskAttachmentUploadUrlRequest
): Promise<TaskAttachmentUploadUrlResponse> {
  return apiPost<TaskAttachmentUploadUrlResponse>(`${API_PREFIX}/tasks/attachments/upload-url`, data);
}

export async function updateTask(id: string, data: UpdateTaskRequest): Promise<Task> {
  return apiPut<Task>(`${API_PREFIX}/tasks/${id}`, data);
}

export async function deleteTask(id: string): Promise<void> {
  return apiDelete(`${API_PREFIX}/tasks/${id}`);
}

export async function updateTaskStatus(id: string, status: TaskStatus): Promise<Task> {
  return apiPut<Task>(`${API_PREFIX}/tasks/${id}/status`, { status });
}

export async function getTaskChildren(id: string): Promise<Task[]> {
  return apiGet<Task[]>(`${API_PREFIX}/tasks/${id}/children`);
}

export async function assignTask(id: string, userId: string | null): Promise<Task> {
  return apiPost<Task>(`${API_PREFIX}/tasks/${id}/assign`, { user_id: userId });
}

export async function updateTaskMetadata(id: string, metadata: Record<string, unknown>): Promise<Task> {
  return apiPut<Task>(`${API_PREFIX}/tasks/${id}/metadata`, { metadata });
}

export async function getTaskAttempts(taskId: string) {
  return apiGet<Array<{
    id: string;
    task_id: string;
    status: string;
    started_at?: string;
    completed_at?: string;
    error_message?: string;
    created_at: string;
  }>>(`${API_PREFIX}/tasks/${taskId}/attempts`);
}

export async function getAttemptLogs(attemptId: string) {
  return apiGet<Array<{
    id: string;
    attempt_id: string;
    log_type: string;
    content: string;
    created_at: string;
  }>>(`${API_PREFIX}/attempts/${attemptId}/logs`);
}
