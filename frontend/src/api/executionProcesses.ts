import { apiGet, apiPost, API_PREFIX } from './client';
import {
  normalizeAgentLogs,
  type AgentLog,
  type AgentLogWire,
  type TaskAttempt,
} from './taskAttempts';

export interface ExecutionProcess {
  id: string;
  attempt_id: string;
  process_id: number | null;
  worktree_path: string | null;
  branch_name: string | null;
  created_at: string;
}

export interface ResetExecutionProcessRequest {
  perform_git_reset?: boolean;
  force_when_dirty?: boolean;
}

export interface ResetExecutionProcessResponse {
  process_id: string;
  worktree_path: string | null;
  git_reset_applied: boolean;
  worktree_was_dirty: boolean;
  force_when_dirty: boolean;
  requested_by_user_id: string;
  requested_at: string;
}

export async function getExecutionProcesses(attemptId: string): Promise<ExecutionProcess[]> {
  const query = encodeURIComponent(attemptId);
  return apiGet<ExecutionProcess[]>(`${API_PREFIX}/execution-processes?attempt_id=${query}`);
}

export async function getExecutionProcess(processId: string): Promise<ExecutionProcess> {
  return apiGet<ExecutionProcess>(`${API_PREFIX}/execution-processes/${processId}`);
}

export async function followUpExecutionProcess(
  processId: string,
  prompt: string
): Promise<TaskAttempt> {
  return apiPost<TaskAttempt>(`${API_PREFIX}/execution-processes/${processId}/follow-up`, {
    prompt,
  });
}

export async function resetExecutionProcess(
  processId: string,
  request: ResetExecutionProcessRequest
): Promise<ResetExecutionProcessResponse> {
  return apiPost<ResetExecutionProcessResponse>(
    `${API_PREFIX}/execution-processes/${processId}/reset`,
    {
      perform_git_reset: request.perform_git_reset ?? false,
      force_when_dirty: request.force_when_dirty ?? false,
    }
  );
}

export async function getExecutionProcessRawLogs(processId: string): Promise<AgentLog[]> {
  const logs = await apiGet<AgentLogWire[]>(
    `${API_PREFIX}/execution-processes/${processId}/raw-logs`
  );
  return normalizeAgentLogs(logs);
}

export async function getExecutionProcessNormalizedLogs(processId: string): Promise<AgentLog[]> {
  const logs = await apiGet<AgentLogWire[]>(
    `${API_PREFIX}/execution-processes/${processId}/normalized-logs`
  );
  return normalizeAgentLogs(logs);
}
