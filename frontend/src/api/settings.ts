// Settings API - Real Implementation
import { apiGet, apiPost, apiPut } from './client';

// Response type from backend (safe - no encrypted values)
export interface SystemSettingsResponse {
    gitlab_url: string;
    gitlab_pat_configured: boolean;
    gitlab_auto_sync: boolean;
    agent_cli_provider: string;
    cloudflare_account_id: string | null;
    cloudflare_api_token_configured: boolean;
    cloudflare_zone_id: string | null;
    cloudflare_base_domain: string | null;
    notifications_email_enabled: boolean;
    notifications_slack_enabled: boolean;
    notifications_slack_webhook_url: string | null;
    /** Path where agent worktrees (cloned source code) are stored. From env WORKTREES_PATH. */
    worktrees_path: string;
    /** Preferred language for agent conversation: en or vi. */
    preferred_agent_language: string;
    updated_at: string;
}

// Request type for updates
export interface UpdateSystemSettingsRequest {
    gitlab_url?: string;
    gitlab_pat?: string; // Plain text - backend encrypts (used for GitLab or GitHub)
    gitlab_auto_sync?: boolean;
    agent_cli_provider?: string;
    cloudflare_account_id?: string;
    cloudflare_api_token?: string; // Plain text - backend encrypts
    cloudflare_zone_id?: string;
    cloudflare_base_domain?: string;
    notifications_email_enabled?: boolean;
    notifications_slack_enabled?: boolean;
    notifications_slack_webhook_url?: string;
    worktrees_path?: string;
    preferred_agent_language?: string;
}

// Legacy Settings type for backward compatibility with UI
export interface Settings {
    gitlab: {
        url: string;
        token: string; // Masked or empty for display
        autoSync: boolean;
        configured: boolean;
    };
    agent: {
        provider: string;
    };
    cloudflare: {
        accountId: string;
        token: string; // Masked or empty for display
        zoneId: string;
        baseDomain: string;
        configured: boolean;
    };
    notifications: {
        email: boolean;
        slack: boolean;
        slackWebhookUrl: string;
    };
    /** Path where agent worktrees (cloned source code) are stored. Read-only from env. */
    worktreesPath: string;
    /** Preferred language for agent conversation: en | vi. */
    preferredAgentLanguage: string;
}

// Convert backend response to frontend Settings type
function toSettings(response: SystemSettingsResponse): Settings {
    return {
        gitlab: {
            url: response.gitlab_url,
            token: response.gitlab_pat_configured ? '••••••••••••••••••••' : '',
            autoSync: response.gitlab_auto_sync,
            configured: response.gitlab_pat_configured,
        },
        agent: {
            provider: response.agent_cli_provider || 'claude-code',
        },
        cloudflare: {
            accountId: response.cloudflare_account_id || '',
            token: response.cloudflare_api_token_configured ? '••••••••••••••••••••' : '',
            zoneId: response.cloudflare_zone_id || '',
            baseDomain: response.cloudflare_base_domain || '',
            configured: response.cloudflare_api_token_configured,
        },
        notifications: {
            email: response.notifications_email_enabled,
            slack: response.notifications_slack_enabled,
            slackWebhookUrl: response.notifications_slack_webhook_url || '',
        },
        worktreesPath: response.worktrees_path || './worktrees',
        preferredAgentLanguage: response.preferred_agent_language || 'en',
    };
}

// Convert frontend Settings to backend request
function toUpdateRequest(settings: Settings, original: Settings): UpdateSystemSettingsRequest {
    const req: UpdateSystemSettingsRequest = {};

    // Only send changed values
    if (settings.gitlab.url !== original.gitlab.url) {
        req.gitlab_url = settings.gitlab.url;
    }
    // Only send token if it's not masked (user entered new value)
    if (settings.gitlab.token && !settings.gitlab.token.startsWith('••')) {
        req.gitlab_pat = settings.gitlab.token;
    }
    if (settings.gitlab.autoSync !== original.gitlab.autoSync) {
        req.gitlab_auto_sync = settings.gitlab.autoSync;
    }

    if (settings.agent.provider !== original.agent.provider) {
        req.agent_cli_provider = settings.agent.provider;
    }

    if (settings.cloudflare.accountId !== original.cloudflare.accountId) {
        req.cloudflare_account_id = settings.cloudflare.accountId;
    }
    if (settings.cloudflare.token && !settings.cloudflare.token.startsWith('••')) {
        req.cloudflare_api_token = settings.cloudflare.token;
    }
    if (settings.cloudflare.zoneId !== original.cloudflare.zoneId) {
        req.cloudflare_zone_id = settings.cloudflare.zoneId;
    }
    if (settings.cloudflare.baseDomain !== original.cloudflare.baseDomain) {
        req.cloudflare_base_domain = settings.cloudflare.baseDomain;
    }

    if (settings.notifications.email !== original.notifications.email) {
        req.notifications_email_enabled = settings.notifications.email;
    }
    if (settings.notifications.slack !== original.notifications.slack) {
        req.notifications_slack_enabled = settings.notifications.slack;
    }
    if (settings.notifications.slackWebhookUrl !== original.notifications.slackWebhookUrl) {
        req.notifications_slack_webhook_url = settings.notifications.slackWebhookUrl;
    }

    if (settings.worktreesPath !== original.worktreesPath) {
        req.worktrees_path = settings.worktreesPath;
    }
    if (settings.preferredAgentLanguage !== original.preferredAgentLanguage) {
        req.preferred_agent_language = settings.preferredAgentLanguage;
    }

    return req;
}

// Store original settings for comparison during update
let originalSettings: Settings | null = null;

export async function getSettings(): Promise<Settings> {
    const response = await apiGet<SystemSettingsResponse>('/api/v1/settings');
    originalSettings = toSettings(response);
    return originalSettings;
}

export async function updateSettings(settings: Settings): Promise<Settings> {
    if (!originalSettings) {
        // If no original, fetch first
        await getSettings();
    }

    const request = toUpdateRequest(settings, originalSettings!);

    // Only make API call if there are actual changes
    if (Object.keys(request).length === 0) {
        return settings;
    }

    const response = await apiPut<SystemSettingsResponse>('/api/v1/settings', request);
    originalSettings = toSettings(response);
    return originalSettings;
}

export interface ConnectionTestResult {
    success: boolean;
    message: string;
}

export interface CloudflareConnectionCheckRequest {
    cloudflare_account_id?: string;
    cloudflare_api_token?: string;
    cloudflare_zone_id?: string;
    cloudflare_base_domain?: string;
}

export interface CloudflareConnectionCheckResponse {
    status: 'success' | 'warning' | 'error';
    ok: boolean;
    config_complete: boolean;
    connection_ok: boolean;
    tunnel_create_ok: boolean;
    dns_record_ok: boolean | null;
    cleanup_ok: boolean;
    missing_fields: string[];
    message: string;
    details: string[];
    checked_at: string;
    preview_url_example: string | null;
}

export async function testClaudeConnection(): Promise<ConnectionTestResult> {
    // Claude connection is tested via /api/v1/agent/status endpoint
    // This is already handled in SettingsPage via apiGet('/agent/status')
    return { success: true, message: 'Use agent status endpoint' };
}

export async function testGitLabConnection(): Promise<ConnectionTestResult> {
    return apiGet<ConnectionTestResult>('/api/v1/settings/test-gitlab');
}

export async function checkCloudflareConnection(
    payload: CloudflareConnectionCheckRequest
): Promise<CloudflareConnectionCheckResponse> {
    return apiPost<CloudflareConnectionCheckResponse>(
        '/api/v1/settings/cloudflare/check',
        payload
    );
}

export type ProviderAuthState =
    | 'authenticated'
    | 'unauthenticated'
    | 'expired'
    | 'unknown';

export type ProviderAvailabilityReason =
    | 'ok'
    | 'cli_missing'
    | 'not_authenticated'
    | 'auth_expired'
    | 'auth_check_failed';

export interface AgentProviderStatus {
    provider: string;
    installed: boolean;
    auth_state: ProviderAuthState;
    available: boolean;
    reason: ProviderAvailabilityReason;
    message: string;
    checked_at: string;
}

export interface AgentProvidersStatusResponse {
    default_provider: string;
    providers: AgentProviderStatus[];
}

export interface AgentAuthSession {
    session_id: string;
    provider: string;
    flow_type: 'device_flow' | 'oob_code' | 'loopback_proxy' | 'unknown';
    status:
        | 'initiated'
        | 'waiting_user_action'
        | 'verifying'
        | 'succeeded'
        | 'failed'
        | 'cancelled'
        | 'timed_out';
    created_at: string;
    updated_at: string;
    expires_at: string;
    process_pid: number | null;
    allowed_loopback_port: number | null;
    last_seq: number;
    last_error: string | null;
    result: string | null;
    action_url: string | null;
    action_code: string | null;
    action_hint: string | null;
}

export interface SubmitAgentAuthCodeResponse {
    session_id: string;
    status:
        | 'initiated'
        | 'waiting_user_action'
        | 'verifying'
        | 'succeeded'
        | 'failed'
        | 'cancelled'
        | 'timed_out';
    accepted: boolean;
    message: string;
}

export async function getAgentProvidersStatus(): Promise<AgentProvidersStatusResponse> {
    return apiGet<AgentProvidersStatusResponse>('/api/v1/agent/providers/status');
}

export async function initiateAgentAuth(
    provider: string,
    forceReauth?: boolean
): Promise<AgentAuthSession> {
    return apiPost<AgentAuthSession>('/api/v1/agent/auth/initiate', {
        provider,
        force_reauth: forceReauth ?? false,
    });
}

export async function submitAgentAuthCode(
    sessionId: string,
    code: string
): Promise<SubmitAgentAuthCodeResponse> {
    return apiPost<SubmitAgentAuthCodeResponse>('/api/v1/agent/auth/submit-code', {
        session_id: sessionId,
        code,
    });
}

export async function cancelAgentAuth(sessionId: string): Promise<AgentAuthSession> {
    return apiPost<AgentAuthSession>('/api/v1/agent/auth/cancel', {
        session_id: sessionId,
    });
}

export async function getAgentAuthSession(sessionId: string): Promise<AgentAuthSession> {
    return apiGet<AgentAuthSession>(`/api/v1/agent/auth/sessions/${sessionId}`);
}
