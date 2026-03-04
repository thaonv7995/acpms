// Agent Logs API - Real backend integration
import { apiGet } from './client';
import { logger } from '@/lib/logger';

// Types matching backend DTOs
export type AgentLogType = 'stdout' | 'stderr' | 'system';

export interface AgentStatus {
    id: string;
    name: string;
    task_title: string;
    project_name: string;
    status: 'queued' | 'running' | 'success' | 'failed' | 'cancelled';
    started_at: string | null;
    created_at: string;
}

export interface AgentLogEntry {
    id: string;
    attempt_id: string;
    task_id: string;
    task_title: string;
    project_name: string;
    log_type: string;
    content: string;
    created_at: string;
}

// API Functions
export async function getAgentStatuses(): Promise<AgentStatus[]> {
    return apiGet<AgentStatus[]>('/api/v1/agent-activity/status');
}

export async function getAgentLogs(filters?: {
    attemptId?: string;
    projectId?: string;
    limit?: number;
}): Promise<AgentLogEntry[]> {
    const params = new URLSearchParams();
    if (filters?.attemptId) params.append('attempt_id', filters.attemptId);
    if (filters?.projectId) params.append('project_id', filters.projectId);
    if (filters?.limit) params.append('limit', filters.limit.toString());

    const queryString = params.toString();
    const url = `/api/v1/agent-activity/logs${queryString ? `?${queryString}` : ''}`;

    return apiGet<AgentLogEntry[]>(url);
}

// Permission handling (stub - not yet implemented in backend)
export async function approvePermission(_logId: string): Promise<void> {
    // TODO: Implement when permission handling is added to backend
    logger.warn('approvePermission not yet implemented');
}

export async function denyPermission(_logId: string): Promise<void> {
    // TODO: Implement when permission handling is added to backend
    logger.warn('denyPermission not yet implemented');
}
