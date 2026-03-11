// Project Settings API - Per-project configuration
import { apiGet, apiPut, apiPatch } from './client';

/**
 * Project Settings structure as defined in Phase 1
 * Stored in projects.settings JSONB column
 */
export interface ProjectSettings {
    // Agent Flow Settings
    require_review: boolean;
    auto_execute: boolean;
    auto_execute_types: string[];
    auto_execute_priority: 'low' | 'normal' | 'high';
    auto_retry: boolean;
    retry_backoff: 'fixed' | 'exponential';
    max_retries: number;
    max_concurrent: number;
    timeout_mins: number;

    // Deployment Settings
    auto_deploy: boolean; // Preview delivery when task completes
    preview_enabled: boolean; // Legacy alias for preview delivery
    production_deploy_on_merge?: boolean; // Production deploy when MR merged
    preview_ttl_days: number;

    // GitOps Settings
    gitops_enabled: boolean;
    auto_merge: boolean;
    deploy_branch: string;

    // Notification Settings
    notify_on_success: boolean;
    notify_on_failure: boolean;
    notify_on_review: boolean;
    notify_channels: string[];
}

/**
 * Default settings values
 */
export const DEFAULT_PROJECT_SETTINGS: ProjectSettings = {
    // Agent Flow
    require_review: true,
    auto_execute: false,
    auto_execute_types: [],
    auto_execute_priority: 'normal',
    auto_retry: false,
    retry_backoff: 'exponential',
    max_retries: 3,
    max_concurrent: 3,
    timeout_mins: 30,

    // Deployment
    auto_deploy: false,
    preview_enabled: false,
    production_deploy_on_merge: false,
    preview_ttl_days: 7,

    // GitOps
    gitops_enabled: true,
    auto_merge: false,
    deploy_branch: 'main',

    // Notifications
    notify_on_success: false,
    notify_on_failure: true,
    notify_on_review: true,
    notify_channels: [],
};

/**
 * API Response for project settings
 */
export interface ProjectSettingsResponse {
    settings: ProjectSettings;
    defaults: ProjectSettings;
}

/**
 * Request type for updating settings
 */
export type UpdateProjectSettingsRequest = Partial<ProjectSettings>;

/**
 * Request type for patching a single setting
 */
export interface PatchProjectSettingRequest {
    value: unknown;
}

/**
 * Get project settings
 */
export async function getProjectSettings(projectId: string): Promise<ProjectSettingsResponse> {
    return apiGet<ProjectSettingsResponse>(`/api/v1/projects/${projectId}/settings`);
}

/**
 * Update all project settings (PUT - replace)
 */
export async function updateProjectSettings(
    projectId: string,
    settings: UpdateProjectSettingsRequest
): Promise<ProjectSettingsResponse> {
    return apiPut<ProjectSettingsResponse>(`/api/v1/projects/${projectId}/settings`, settings);
}

/**
 * Patch a single project setting
 */
export async function patchProjectSetting(
    projectId: string,
    key: keyof ProjectSettings,
    value: unknown
): Promise<ProjectSettingsResponse> {
    return apiPatch<ProjectSettingsResponse>(
        `/api/v1/projects/${projectId}/settings/${key}`,
        { value }
    );
}
