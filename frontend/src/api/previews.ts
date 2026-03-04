import {
    API_PREFIX,
    ApiError,
    authenticatedFetch,
    type ApiResponse,
} from './client';

export interface PreviewInfo {
    id: string;
    attempt_id: string;
    preview_url: string;
    status: 'creating' | 'active' | 'failed' | 'deleted';
    created_at: string;
    expires_at: string | null;
}

export interface PreviewReadiness {
    attempt_id: string;
    project_type: string;
    preview_supported: boolean;
    preview_enabled: boolean;
    runtime_enabled: boolean;
    cloudflare_ready: boolean;
    ready: boolean;
    missing_cloudflare_fields: string[];
    reason: string | null;
}

export interface PreviewRuntimeStatus {
    attempt_id: string;
    runtime_enabled: boolean;
    worktree_path: string | null;
    compose_file_exists: boolean;
    docker_project_name: string | null;
    compose_file_path: string | null;
    running_services: string[];
    runtime_ready: boolean;
    last_error: string | null;
    started_at: string | null;
    stopped_at: string | null;
    message: string | null;
}

export interface PreviewRuntimeLogs {
    attempt_id: string;
    runtime_enabled: boolean;
    docker_project_name: string | null;
    compose_file_path: string | null;
    tail: number;
    logs: string;
    message: string | null;
}

async function parsePreviewPayload<T>(response: Response): Promise<T> {
    if (!response.ok) {
        try {
            const errorBody = (await response.json()) as ApiResponse<null>;
            throw new ApiError(
                response.status,
                errorBody.message || response.statusText,
                errorBody.code
            );
        } catch (error) {
            if (error instanceof ApiError) {
                throw error;
            }
            throw new ApiError(response.status, response.statusText);
        }
    }

    const rawText = await response.text();
    if (!rawText) {
        return undefined as T;
    }

    const payload = JSON.parse(rawText) as unknown;
    if (
        payload &&
        typeof payload === 'object' &&
        'success' in payload &&
        'data' in payload
    ) {
        const wrapped = payload as ApiResponse<T>;
        if (!wrapped.success) {
            throw new ApiError(200, wrapped.message, wrapped.code);
        }
        return wrapped.data;
    }

    return payload as T;
}

export async function getPreviews(): Promise<PreviewInfo[]> {
    const response = await authenticatedFetch(`${API_PREFIX}/previews`);
    return parsePreviewPayload<PreviewInfo[]>(response);
}

export async function createPreview(attemptId: string): Promise<PreviewInfo> {
    const response = await authenticatedFetch(
        `${API_PREFIX}/attempts/${attemptId}/preview`,
        {
            method: 'POST',
            body: JSON.stringify({}),
        }
    );
    return parsePreviewPayload<PreviewInfo>(response);
}

export async function getPreview(attemptId: string): Promise<PreviewInfo | null> {
    const response = await authenticatedFetch(
        `${API_PREFIX}/attempts/${attemptId}/preview`
    );
    return parsePreviewPayload<PreviewInfo | null>(response);
}

export async function getPreviewReadiness(attemptId: string): Promise<PreviewReadiness> {
    const response = await authenticatedFetch(
        `${API_PREFIX}/attempts/${attemptId}/preview/readiness`
    );
    return parsePreviewPayload<PreviewReadiness>(response);
}

export async function getPreviewRuntimeStatus(
    attemptId: string
): Promise<PreviewRuntimeStatus> {
    const response = await authenticatedFetch(
        `${API_PREFIX}/attempts/${attemptId}/preview/runtime-status`
    );
    return parsePreviewPayload<PreviewRuntimeStatus>(response);
}

export async function getPreviewRuntimeLogs(
    attemptId: string,
    tail = 200
): Promise<PreviewRuntimeLogs> {
    const response = await authenticatedFetch(
        `${API_PREFIX}/attempts/${attemptId}/preview/runtime-logs?tail=${tail}`
    );
    return parsePreviewPayload<PreviewRuntimeLogs>(response);
}

export async function deletePreview(previewId: string): Promise<void> {
    const response = await authenticatedFetch(`${API_PREFIX}/previews/${previewId}`, {
        method: 'DELETE',
    });
    await parsePreviewPayload<void>(response);
}
