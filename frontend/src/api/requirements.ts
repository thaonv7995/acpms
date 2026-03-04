// Requirements API client
import { apiGet, apiPost, apiPut, apiDelete, API_PREFIX } from './client';

export type RequirementStatus = 'todo' | 'in_progress' | 'done';
export type RequirementPriority = 'low' | 'medium' | 'high' | 'critical';

export interface Requirement {
    id: string;
    project_id: string;
    title: string;
    content: string;
    status: RequirementStatus;
    priority: RequirementPriority;
    due_date?: string | null; // YYYY-MM-DD
    metadata: Record<string, unknown>;
    created_by: string;
    created_at: string;
    updated_at: string;
}

export interface CreateRequirementRequest {
    title: string;
    content: string;
    priority?: RequirementPriority;
    due_date?: string | null; // YYYY-MM-DD
    metadata?: Record<string, unknown>;
}

export interface UpdateRequirementRequest {
    title?: string;
    content?: string;
    status?: RequirementStatus;
    priority?: RequirementPriority;
    due_date?: string | null; // YYYY-MM-DD
    metadata?: Record<string, unknown>;
}

export async function getRequirements(projectId: string): Promise<Requirement[]> {
    return apiGet<Requirement[]>(`${API_PREFIX}/projects/${projectId}/requirements`);
}

export async function getRequirement(projectId: string, id: string): Promise<Requirement> {
    return apiGet<Requirement>(`${API_PREFIX}/projects/${projectId}/requirements/${id}`);
}

export async function createRequirement(projectId: string, data: CreateRequirementRequest): Promise<Requirement> {
    const body = { ...data, project_id: projectId, sprint_id: null };
    return apiPost<Requirement>(`${API_PREFIX}/projects/${projectId}/requirements`, body);
}

export async function updateRequirement(projectId: string, id: string, data: UpdateRequirementRequest): Promise<Requirement> {
    return apiPut<Requirement>(`${API_PREFIX}/projects/${projectId}/requirements/${id}`, data);
}

export async function deleteRequirement(projectId: string, id: string): Promise<void> {
    return apiDelete(`${API_PREFIX}/projects/${projectId}/requirements/${id}`);
}

export interface RequirementAttachmentUploadUrlRequest {
    filename: string;
    content_type: string;
}

export interface RequirementAttachmentUploadUrlResponse {
    upload_url: string;
    key: string;
}

export async function getRequirementAttachmentUploadUrl(
    projectId: string,
    data: RequirementAttachmentUploadUrlRequest
): Promise<RequirementAttachmentUploadUrlResponse> {
    const res = await apiPost<RequirementAttachmentUploadUrlResponse>(
        `${API_PREFIX}/projects/${projectId}/requirements/attachments/upload-url`,
        data
    );
    return res;
}

export interface RequirementAttachmentDownloadUrlRequest {
    key: string;
}

export interface RequirementAttachmentDownloadUrlResponse {
    download_url: string;
}

export async function getRequirementAttachmentDownloadUrl(
    projectId: string,
    data: RequirementAttachmentDownloadUrlRequest
): Promise<RequirementAttachmentDownloadUrlResponse> {
    return apiPost<RequirementAttachmentDownloadUrlResponse>(
        `${API_PREFIX}/projects/${projectId}/requirements/attachments/download-url`,
        data
    );
}
