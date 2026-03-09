import { apiDelete, apiGet, apiPatch, apiPost, API_PREFIX } from './client';

export interface TaskContextAttachment {
  id: string;
  task_context_id: string;
  storage_key: string;
  filename: string;
  content_type: string;
  size_bytes?: number | null;
  checksum?: string | null;
  created_at: string;
}

export interface TaskContext {
  id: string;
  task_id: string;
  title?: string | null;
  content_type: string;
  raw_content: string;
  source: string;
  sort_order: number;
  attachments: TaskContextAttachment[];
  created_at: string;
  updated_at: string;
}

export interface CreateTaskContextRequest {
  title?: string | null;
  content_type: string;
  raw_content: string;
  source: string;
  sort_order: number;
}

export interface UpdateTaskContextRequest {
  title?: string | null;
  content_type?: string;
  raw_content?: string;
  sort_order?: number;
}

export interface TaskContextAttachmentUploadUrlRequest {
  filename: string;
  content_type: string;
}

export interface TaskContextAttachmentUploadUrlResponse {
  upload_url: string;
  key: string;
}

export interface CreateTaskContextAttachmentRequest {
  storage_key: string;
  filename: string;
  content_type: string;
  size_bytes?: number | null;
  checksum?: string | null;
}

export interface TaskContextAttachmentDownloadUrlResponse {
  download_url: string;
}

export async function getTaskContexts(taskId: string): Promise<TaskContext[]> {
  return apiGet<TaskContext[]>(`${API_PREFIX}/tasks/${taskId}/contexts`);
}

export async function createTaskContext(
  taskId: string,
  data: CreateTaskContextRequest
): Promise<TaskContext> {
  return apiPost<TaskContext>(`${API_PREFIX}/tasks/${taskId}/contexts`, data);
}

export async function updateTaskContext(
  taskId: string,
  contextId: string,
  data: UpdateTaskContextRequest
): Promise<TaskContext> {
  return apiPatch<TaskContext>(`${API_PREFIX}/tasks/${taskId}/contexts/${contextId}`, data);
}

export async function deleteTaskContext(taskId: string, contextId: string): Promise<void> {
  return apiDelete(`${API_PREFIX}/tasks/${taskId}/contexts/${contextId}`);
}

export async function getTaskContextAttachmentUploadUrl(
  taskId: string,
  data: TaskContextAttachmentUploadUrlRequest
): Promise<TaskContextAttachmentUploadUrlResponse> {
  return apiPost<TaskContextAttachmentUploadUrlResponse>(
    `${API_PREFIX}/tasks/${taskId}/context-attachments/upload-url`,
    data
  );
}

export async function createTaskContextAttachment(
  taskId: string,
  contextId: string,
  data: CreateTaskContextAttachmentRequest
): Promise<TaskContextAttachment> {
  return apiPost<TaskContextAttachment>(
    `${API_PREFIX}/tasks/${taskId}/contexts/${contextId}/attachments`,
    data
  );
}

export async function deleteTaskContextAttachment(
  taskId: string,
  contextId: string,
  attachmentId: string
): Promise<void> {
  return apiDelete(
    `${API_PREFIX}/tasks/${taskId}/contexts/${contextId}/attachments/${attachmentId}`
  );
}

export async function getTaskContextAttachmentDownloadUrl(
  taskId: string,
  key: string
): Promise<TaskContextAttachmentDownloadUrlResponse> {
  return apiPost<TaskContextAttachmentDownloadUrlResponse>(
    `${API_PREFIX}/tasks/${taskId}/context-attachments/download-url`,
    { key }
  );
}
