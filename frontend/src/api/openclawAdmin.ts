import { apiGet, apiPost } from './client';

export interface OpenClawClientSummary {
    client_id: string;
    display_name: string;
    status: 'active' | 'disabled' | 'revoked' | string;
    enrolled_at: string;
    last_seen_at: string | null;
    last_seen_ip: string | null;
    last_seen_user_agent: string | null;
    key_fingerprints: string[];
}

export interface OpenClawClientsResponse {
    clients: OpenClawClientSummary[];
}

export interface CreateOpenClawBootstrapTokenRequest {
    label: string;
    expires_in_minutes?: number;
    suggested_display_name?: string;
    metadata?: Record<string, unknown>;
}

export interface OpenClawBootstrapPromptResponse {
    bootstrap_token_id: string;
    expires_at: string;
    prompt_text: string;
    token_preview: string;
}

export interface OpenClawClientMutationResponse {
    client: OpenClawClientSummary;
}

export async function listOpenClawClients(): Promise<OpenClawClientsResponse> {
    return apiGet<OpenClawClientsResponse>('/api/v1/admin/openclaw/clients');
}

export async function createOpenClawBootstrapToken(
    payload: CreateOpenClawBootstrapTokenRequest
): Promise<OpenClawBootstrapPromptResponse> {
    return apiPost<OpenClawBootstrapPromptResponse>(
        '/api/v1/admin/openclaw/bootstrap-tokens',
        payload
    );
}

export async function disableOpenClawClient(
    clientId: string
): Promise<OpenClawClientMutationResponse> {
    return apiPost<OpenClawClientMutationResponse>(
        `/api/v1/admin/openclaw/clients/${encodeURIComponent(clientId)}/disable`,
        {}
    );
}

export async function enableOpenClawClient(
    clientId: string
): Promise<OpenClawClientMutationResponse> {
    return apiPost<OpenClawClientMutationResponse>(
        `/api/v1/admin/openclaw/clients/${encodeURIComponent(clientId)}/enable`,
        {}
    );
}

export async function revokeOpenClawClient(
    clientId: string
): Promise<OpenClawClientMutationResponse> {
    return apiPost<OpenClawClientMutationResponse>(
        `/api/v1/admin/openclaw/clients/${encodeURIComponent(clientId)}/revoke`,
        {}
    );
}
