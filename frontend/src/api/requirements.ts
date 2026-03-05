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

export type RequirementBreakdownStatus =
    | 'queued'
    | 'running'
    | 'review'
    | 'confirmed'
    | 'failed'
    | 'cancelled';

export type RequirementBreakdownSprintAssignmentMode = 'active' | 'selected' | 'backlog';

export interface RequirementBreakdownSession {
    id: string;
    project_id: string;
    requirement_id: string;
    created_by: string;
    status: RequirementBreakdownStatus;
    analysis?: Record<string, unknown> | null;
    impact?: Array<Record<string, unknown>> | null;
    plan?: Record<string, unknown> | null;
    proposed_tasks?: Array<Record<string, unknown>> | null;
    suggested_sprint_id?: string | null;
    error_message?: string | null;
    created_at: string;
    updated_at: string;
    started_at?: string | null;
    completed_at?: string | null;
    confirmed_at?: string | null;
    cancelled_at?: string | null;
}

export interface ConfirmRequirementBreakdownRequest {
    assignment_mode: RequirementBreakdownSprintAssignmentMode;
    sprint_id?: string | null;
}

export interface ConfirmRequirementBreakdownResponse {
    session: RequirementBreakdownSession;
    tasks: Array<{
        id: string;
        title: string;
        status: string;
        task_type: string;
        requirement_id?: string | null;
        sprint_id?: string | null;
    }>;
}

export interface ManualBreakdownTaskDraft {
    title: string;
    description?: string;
    task_type: string;
    priority?: 'low' | 'medium' | 'high' | 'critical';
    assigned_to?: string | null;
    kind?: string | null;
    metadata?: Record<string, unknown>;
}

export interface ConfirmRequirementBreakdownManualRequest {
    assignment_mode: RequirementBreakdownSprintAssignmentMode;
    sprint_id?: string | null;
    tasks: ManualBreakdownTaskDraft[];
}

export interface ConfirmRequirementBreakdownManualResponse {
    tasks: Array<{
        id: string;
        title: string;
        status: string;
        task_type: string;
        requirement_id?: string | null;
        sprint_id?: string | null;
    }>;
}

export interface StartRequirementTaskSequenceRequest {
    continue_on_failure?: boolean;
}

export interface StartRequirementTaskSequenceResponse {
    run_id: string;
    task_ids: string[];
    total_tasks: number;
    continue_on_failure: boolean;
}

export async function startRequirementBreakdown(
    projectId: string,
    requirementId: string
): Promise<RequirementBreakdownSession> {
    return apiPost<RequirementBreakdownSession>(
        `${API_PREFIX}/projects/${projectId}/requirements/${requirementId}/breakdown/start`,
        {}
    );
}

export async function getRequirementBreakdownSession(
    projectId: string,
    requirementId: string,
    sessionId: string
): Promise<RequirementBreakdownSession> {
    return apiGet<RequirementBreakdownSession>(
        `${API_PREFIX}/projects/${projectId}/requirements/${requirementId}/breakdown/${sessionId}`
    );
}

export async function confirmRequirementBreakdown(
    projectId: string,
    requirementId: string,
    sessionId: string,
    payload: ConfirmRequirementBreakdownRequest
): Promise<ConfirmRequirementBreakdownResponse> {
    return apiPost<ConfirmRequirementBreakdownResponse>(
        `${API_PREFIX}/projects/${projectId}/requirements/${requirementId}/breakdown/${sessionId}/confirm`,
        payload
    );
}

export async function cancelRequirementBreakdown(
    projectId: string,
    requirementId: string,
    sessionId: string
): Promise<RequirementBreakdownSession> {
    return apiPost<RequirementBreakdownSession>(
        `${API_PREFIX}/projects/${projectId}/requirements/${requirementId}/breakdown/${sessionId}/cancel`,
        {}
    );
}

export async function confirmRequirementBreakdownManual(
    projectId: string,
    requirementId: string,
    payload: ConfirmRequirementBreakdownManualRequest
): Promise<ConfirmRequirementBreakdownManualResponse> {
    return apiPost<ConfirmRequirementBreakdownManualResponse>(
        `${API_PREFIX}/projects/${projectId}/requirements/${requirementId}/breakdown/manual/confirm`,
        payload
    );
}

export async function startRequirementTaskSequence(
    projectId: string,
    requirementId: string,
    payload: StartRequirementTaskSequenceRequest = {}
): Promise<StartRequirementTaskSequenceResponse> {
    return apiPost<StartRequirementTaskSequenceResponse>(
        `${API_PREFIX}/projects/${projectId}/requirements/${requirementId}/tasks/start-sequential`,
        payload
    );
}
