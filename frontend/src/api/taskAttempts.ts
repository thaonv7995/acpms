import { apiGet, apiPost, authenticatedFetch, API_PREFIX } from './client';
import type { FileDiffSummary } from '@/types/timeline-log';

export interface TaskAttempt {
  id: string;
  task_id: string;
  status: AttemptStatus;
  started_at: string | null;
  completed_at: string | null;
  error_message: string | null;
  metadata: Record<string, unknown>;
  created_at: string;
}

export interface AgentLog {
  id: string;
  attempt_id: string;
  log_type: string;
  content: string;
  created_at: string;
}

export interface AgentLogWire {
  id: string;
  attempt_id: string;
  log_type?: string;
  content?: string;
  created_at?: string;
  type?: string;
  message?: string;
  timestamp?: string;
}

export type AttemptStatus = 'QUEUED' | 'RUNNING' | 'SUCCESS' | 'FAILED' | 'CANCELLED';

export async function createTaskAttempt(taskId: string): Promise<TaskAttempt> {
  return apiPost<TaskAttempt>(`${API_PREFIX}/tasks/${taskId}/attempts`, {});
}

/** Create attempt after editing task. Cleans up old worktree and closes open MR first. */
export async function createTaskAttemptFromEdit(taskId: string): Promise<TaskAttempt> {
  return apiPost<TaskAttempt>(`${API_PREFIX}/tasks/${taskId}/attempts/from-edit`, {});
}

export async function getTaskAttempts(taskId: string): Promise<TaskAttempt[]> {
  return apiGet<TaskAttempt[]>(`${API_PREFIX}/tasks/${taskId}/attempts`);
}

export async function getAttempt(attemptId: string): Promise<TaskAttempt> {
  return apiGet<TaskAttempt>(`${API_PREFIX}/attempts/${attemptId}`);
}

export function normalizeAgentLog(log: AgentLogWire): AgentLog {
  return {
    id: log.id,
    attempt_id: log.attempt_id,
    log_type: log.log_type ?? log.type ?? 'stdout',
    content: log.content ?? log.message ?? '',
    created_at: log.created_at ?? log.timestamp ?? new Date().toISOString(),
  };
}

export function normalizeAgentLogs(logs: AgentLogWire[]): AgentLog[] {
  return logs.map(normalizeAgentLog);
}

export async function getAttemptLogs(attemptId: string): Promise<AgentLog[]> {
  const raw = await apiGet<AgentLogWire[] | undefined>(
    `${API_PREFIX}/attempts/${attemptId}/logs`
  );
  const logs: AgentLogWire[] = Array.isArray(raw) ? raw : [];
  return normalizeAgentLogs(logs);
}

export async function sendAttemptInput(attemptId: string, input: string): Promise<void> {
  const res = await authenticatedFetch(`${API_PREFIX}/attempts/${attemptId}/input`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ input }),
  });

  if (!res.ok) {
    throw new Error('Failed to send input');
  }
}

export async function updateAttemptLog(
  attemptId: string,
  logId: string,
  content: string
): Promise<void> {
  const res = await authenticatedFetch(
    `${API_PREFIX}/attempts/${attemptId}/logs/${logId}`,
    {
      method: 'PATCH',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({ content }),
    }
  );

  if (!res.ok) {
    const err = await res.json().catch(() => ({}));
    throw new Error(
      (err as { message?: string })?.message || 'Failed to update message'
    );
  }
}

export type DiffChangeType = 'added' | 'deleted' | 'modified' | 'renamed';

export interface FileDiff {
  change: DiffChangeType;
  old_path: string | null;
  new_path: string | null;
  old_content: string | null;
  new_content: string | null;
  additions: number;
  deletions: number;
}

export interface DiffResponse {
  files: FileDiff[];
  total_files: number;
  total_additions: number;
  total_deletions: number;
}

export async function getAttemptDiff(attemptId: string): Promise<DiffResponse> {
  return apiGet<DiffResponse>(`${API_PREFIX}/attempts/${attemptId}/diff`);
}

export interface DiffSummaryResponse {
  files: FileDiffSummary[];
}

/** Lightweight endpoint for file diff metadata only (no log processing). Use for timeline. */
export async function getAttemptDiffSummary(attemptId: string): Promise<DiffSummaryResponse> {
  return apiGet<DiffSummaryResponse>(`${API_PREFIX}/attempts/${attemptId}/diff-summary`);
}

export interface AttemptArtifact {
  id: string;
  artifact_key: string;
  artifact_type: string;
  size_bytes: number | null;
  file_count: number | null;
  download_url: string | null;
  created_at: string;
}

export async function getAttemptArtifacts(attemptId: string): Promise<AttemptArtifact[]> {
  return apiGet<AttemptArtifact[]>(`${API_PREFIX}/attempts/${attemptId}/artifacts`);
}

export async function approveAttempt(attemptId: string, commitMessage?: string): Promise<void> {
  return apiPost<void>(`${API_PREFIX}/attempts/${attemptId}/approve`, {
    commit_message: commitMessage,
  });
}

export async function cancelAttempt(attemptId: string): Promise<void> {
  return apiPost<void>(`${API_PREFIX}/attempts/${attemptId}/cancel`, {});
}

export interface RetryInfo {
  retry_count: number;
  max_retries: number;
  remaining_retries: number;
  can_retry: boolean;
  auto_retry_enabled: boolean;
  previous_attempt_id: string | null;
  previous_error: string | null;
  next_retry_attempt_id: string | null;
  next_backoff_seconds: number | null;
}

export async function getRetryInfo(attemptId: string): Promise<RetryInfo> {
  return apiGet<RetryInfo>(`${API_PREFIX}/attempts/${attemptId}/retry-info`);
}

export async function retryAttempt(attemptId: string): Promise<TaskAttempt> {
  return apiPost<TaskAttempt>(`${API_PREFIX}/attempts/${attemptId}/retry`, {});
}
